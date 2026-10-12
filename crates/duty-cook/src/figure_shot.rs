//! Pictures of the cooked figures, and the rasterizer that draws them. The figures' compiler asks
//! the same rasterizer which triangles a view shows.
//!
//!   duty-cook figures --in IR --out DIR --profile P
//!
//! cooks the figures alone, writes the section to `DIR/figures.bin` and draws into `DIR/figures/`,
//! all from the section's own bytes read back (flat light from the upper left on the vertices'
//! colours, faces seen from behind left out):
//!
//!   <soldier>.png        bind pose, WALK_AIM at its start and a quarter in, RUN the same, RELOAD
//!                        half way; from ahead on the rifle's side, and from that side
//!   <soldier>_more.png   AIM, IDLE, PATROL, CROUCH_AIM, CROUCH_WALK, HIT
//!   <soldier>_close.png  the bind pose, large
//!   <weapon>.png         from the eye, 60° from bottom to top, 480 × 272: at rest and at three
//!                        moments of RELOAD_TAC
//!   <weapon>_more.png    aimed, and moments of RELOAD_EMPTY, DRAW and HOLSTER
//!   <weapon>_close.png   at rest at twice the size
//!   <name>_source.png    the export's own mesh, drawn the same way: what the compiler started from
//!
//! A held weapon's colours are near black, so its pictures multiply them by 2.2.

use std::path::Path;

use duty_pack::figure::{self, clip, model, part, Figures, Pose};
use serde_json::Value;

use crate::tex::Image;

pub type V3 = [f32; 3];

pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
pub fn unit(a: V3) -> V3 {
    let l = dot(a, a).sqrt();
    if l > 1e-20 {
        [a[0] / l, a[1] / l, a[2] / l]
    } else {
        [0.0, 0.0, 1.0]
    }
}

/// Where a picture is taken from. `axes` are the view's right, up and back (toward the viewer) in
/// the space of what it draws.
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub axes: [V3; 3],
    pub eye: V3,
    /// With perspective: pixels a unit covers at one unit ahead. Without: pixels a unit covers.
    pub scale: f32,
    pub perspective: bool,
    /// The nearest a point may be with perspective.
    pub near: f32,
    pub width: usize,
    pub height: usize,
}

impl View {
    /// The eye of a held weapon's space: at the origin, looking down −Z, `fov` degrees from the
    /// bottom of the picture to its top.
    pub fn eye(width: usize, height: usize, fov: f32) -> View {
        View { axes: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], eye: [0.0; 3], scale: height as f32 * 0.5 / (fov.to_radians() * 0.5).tan(), perspective: true, near: 0.02, width, height }
    }

    /// The same eye turned by `yaw` and `pitch` (radians).
    pub fn turned(mut self, yaw: f32, pitch: f32) -> View {
        let (sy, cy, sp, cp) = (yaw.sin(), yaw.cos(), pitch.sin(), pitch.cos());
        let back = [sy * cp, -sp, cy * cp];
        let right = unit(cross([0.0, 1.0, 0.0], back));
        self.axes = [right, cross(back, right), back];
        self
    }

    /// Without perspective, from the direction `toward` (from the subject to the viewer), `centre`
    /// in the middle of the picture, `span` units from its bottom to its top.
    pub fn flat(width: usize, height: usize, toward: V3, centre: V3, span: f32) -> View {
        let back = unit(toward);
        let right = unit(cross(if back[1].abs() > 0.99 { [0.0, 0.0, 1.0] } else { [0.0, 1.0, 0.0] }, back));
        View { axes: [right, cross(back, right), back], eye: centre, scale: height as f32 / span, perspective: false, near: 0.0, width, height }
    }

    fn local(&self, p: V3) -> V3 {
        let d = sub(p, self.eye);
        [dot(self.axes[0], d), dot(self.axes[1], d), dot(self.axes[2], d)]
    }

    /// A point of the view's own space on the picture: column, row, and how near it is (greater is nearer).
    fn project(&self, v: V3) -> V3 {
        if self.perspective {
            let w = 1.0 / -v[2];
            [self.width as f32 * 0.5 + v[0] * self.scale * w, self.height as f32 * 0.5 - v[1] * self.scale * w, w]
        } else {
            [self.width as f32 * 0.5 + v[0] * self.scale, self.height as f32 * 0.5 - v[1] * self.scale, v[2]]
        }
    }
}

/// How near the nearest surface is at each pixel of a picture.
pub struct Depth {
    pub width: usize,
    pub height: usize,
    pub near: Vec<f32>,
}

impl Depth {
    pub fn new(width: usize, height: usize) -> Depth {
        Depth { width, height, near: vec![f32::NEG_INFINITY; width * height] }
    }

    /// Draws one triangle. `hit(pixel, shares, front)` is called for every pixel where it is the
    /// nearest surface so far, with the three corners' shares there and the side it shows. A
    /// triangle seen from behind is drawn only when `two_sided`.
    pub fn fill(&mut self, view: &View, corners: [V3; 3], two_sided: bool, mut hit: impl FnMut(usize, V3, bool)) {
        let local = corners.map(|p| view.local(p));
        // The part in front of the near plane: up to four corners, each with its shares of the three.
        let mut poly: [(V3, V3); 4] = [([0.0; 3], [0.0; 3]); 4];
        let mut n = 0;
        if view.perspective {
            let ahead = |v: V3| -v[2] - view.near;
            for i in 0..3 {
                let (a, b) = (local[i], local[(i + 1) % 3]);
                let (da, db) = (ahead(a), ahead(b));
                let share = |k: usize| core::array::from_fn(|j| if j == k { 1.0 } else { 0.0 });
                if da >= 0.0 {
                    poly[n] = (a, share(i));
                    n += 1;
                }
                if (da >= 0.0) != (db >= 0.0) {
                    let t = da / (da - db);
                    let mut s = [0.0; 3];
                    s[i] = 1.0 - t;
                    s[(i + 1) % 3] = t;
                    poly[n] = ([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t], s);
                    n += 1;
                }
            }
        } else {
            for i in 0..3 {
                let mut s = [0.0; 3];
                s[i] = 1.0;
                poly[i] = (local[i], s);
            }
            n = 3;
        }
        for k in 1..n.saturating_sub(1) {
            let (p, s) = ([view.project(poly[0].0), view.project(poly[k].0), view.project(poly[k + 1].0)], [poly[0].1, poly[k].1, poly[k + 1].1]);
            // Rows run down the picture, so a face turned to the viewer winds clockwise here.
            let area = (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[1][1] - p[0][1]) * (p[2][0] - p[0][0]);
            let front = area < 0.0;
            if area == 0.0 || (!front && !two_sided) {
                continue;
            }
            let x0 = p.iter().map(|q| q[0]).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
            let x1 = (p.iter().map(|q| q[0]).fold(f32::MIN, f32::max).ceil().max(0.0) as usize).min(self.width);
            let y0 = p.iter().map(|q| q[1]).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
            let y1 = (p.iter().map(|q| q[1]).fold(f32::MIN, f32::max).ceil().max(0.0) as usize).min(self.height);
            let inv = 1.0 / area;
            for y in y0..y1 {
                for x in x0..x1 {
                    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                    let w0 = ((p[1][0] - px) * (p[2][1] - py) - (p[1][1] - py) * (p[2][0] - px)) * inv;
                    let w1 = ((p[2][0] - px) * (p[0][1] - py) - (p[2][1] - py) * (p[0][0] - px)) * inv;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }
                    let near = w0 * p[0][2] + w1 * p[1][2] + w2 * p[2][2];
                    let at = y * self.width + x;
                    if near > self.near[at] {
                        self.near[at] = near;
                        hit(at, core::array::from_fn(|j| w0 * s[0][j] + w1 * s[1][j] + w2 * s[2][j]), front);
                    }
                }
            }
        }
    }
}

/// A picture being drawn: flat light from the upper left on the colours the vertices carry.
pub struct Canvas {
    pub image: Image,
    depth: Depth,
    /// What the colours are multiplied by before the light (the weapons are near black).
    pub gain: f32,
}

impl Canvas {
    pub fn new(width: usize, height: usize, ground: [f32; 3], gain: f32) -> Canvas {
        let mut image = Image::new(width, height);
        image.data.fill([ground[0], ground[1], ground[2], 1.0]);
        Canvas { image, depth: Depth::new(width, height), gain }
    }

    /// One triangle with a colour (0…1, as the display shows it) at each corner. A face seen from
    /// behind is left out.
    pub fn triangle(&mut self, view: &View, corners: [V3; 3], colours: [V3; 3]) {
        let n = unit(cross(sub(corners[1], corners[0]), sub(corners[2], corners[0])));
        let seen = [dot(view.axes[0], n), dot(view.axes[1], n), dot(view.axes[2], n)];
        let light = 0.38 + 0.8 * dot(seen, unit([-0.55, 0.7, 0.45])).max(0.0);
        let (image, gain) = (&mut self.image, self.gain * light);
        self.depth.fill(view, corners, false, |at, s, _| {
            let c: [f32; 3] = core::array::from_fn(|k| (s[0] * colours[0][k] + s[1] * colours[1][k] + s[2] * colours[2][k]) * gain);
            image.data[at] = [c[0], c[1], c[2], 1.0];
        });
    }

    /// A model of a section with its bones at `poses`.
    pub fn model(&mut self, view: &View, model: &figure::ModelRef, poses: &[Pose]) {
        for batch in model.batches() {
            let placed: Vec<(V3, V3)> = (0..batch.batch.vertices as usize)
                .map(|i| {
                    let v = batch.vertex(i).unwrap();
                    (batch.place(&v, poses), [(v.colour & 255) as f32 / 255.0, (v.colour >> 8 & 255) as f32 / 255.0, (v.colour >> 16 & 255) as f32 / 255.0])
                })
                .collect();
            for t in 0..batch.batch.indices as usize / 3 {
                let v: [(V3, V3); 3] = core::array::from_fn(|k| placed[batch.index(t * 3 + k).unwrap() as usize]);
                self.triangle(view, [v[0].0, v[1].0, v[2].0], [v[0].1, v[1].1, v[2].1]);
            }
        }
    }
}

/// Pictures side by side and one under another.
struct Sheet {
    image: Image,
    cell: (usize, usize),
}

impl Sheet {
    fn new(columns: usize, rows: usize, cell: (usize, usize)) -> Sheet {
        Sheet { image: Image::new(columns * cell.0, rows * cell.1), cell }
    }

    fn put(&mut self, column: usize, row: usize, picture: &Image) {
        for y in 0..self.cell.1 {
            for x in 0..self.cell.0 {
                self.image.data[(row * self.cell.1 + y) * self.image.width + column * self.cell.0 + x] = picture.data[y * picture.width + x];
            }
        }
    }
}

/// Every bone's pose in one frame of a clip.
pub fn frame(figures: &Figures, id: u32, frame: u32) -> Option<Vec<Pose>> {
    let clip = figures.clip(id)?;
    Some((0..clip.clip.bones).map(|b| clip.pose(frame, b)).collect())
}

/// The turn of Euler angles as three.js composes them in the order XYZ, and the move `offset`.
pub fn carried(turn: V3, offset: V3) -> Pose {
    let (a, b, c) = (turn[0], turn[1], turn[2]);
    let (sa, ca, sb, cb, sc, cc) = (a.sin(), a.cos(), b.sin(), b.cos(), c.sin(), c.cos());
    [
        [cb * cc, -cb * sc, sb, offset[0]],
        [ca * sc + sa * sb * cc, ca * cc - sa * sb * sc, -sa * cb, offset[1]],
        [sa * sc - ca * sb * cc, sa * cc + ca * sb * sc, ca * cb, offset[2]],
    ]
}

const SOLDIERS: [(&str, u32); 3] = [("vanguard", model::VANGUARD), ("irregular", model::IRREGULAR), ("breacher", model::BREACHER)];
const HELD: [(&str, u32); 3] = [("rifle", model::RIFLE), ("smg", model::SMG), ("pistol", model::PISTOL)];

/// Two sheets of a soldier: its bind pose and moments of walking, running and reloading; then the
/// other clips. Each moment from ahead on the rifle's side, and from that side.
fn soldier_sheets(figures: &Figures, id: u32) -> Result<[Image; 2], String> {
    let model = figures.model(id).ok_or("the section lacks a soldier")?;
    let rest = vec![figure::UNMOVED; figure::bone::SOLDIER];
    let at = |c: u32, share: f32| -> Vec<Pose> {
        let frames = figures.clip(c).map_or(1, |r| r.clip.frames);
        frame(figures, c, (frames as f32 * share) as u32).unwrap_or(rest.clone())
    };
    let first = [rest.clone(), at(clip::WALK_AIM, 0.0), at(clip::WALK_AIM, 0.25), at(clip::RUN, 0.0), at(clip::RUN, 0.25), at(clip::RELOAD, 0.5)];
    let second = [at(clip::AIM, 0.0), at(clip::IDLE, 0.0), at(clip::PATROL, 0.25), at(clip::CROUCH_AIM, 0.0), at(clip::CROUCH_WALK, 0.25), at(clip::HIT, 0.3)];
    let cell = (230, 400);
    let views = [unit([-0.6, 0.18, 0.8]), [-1.0, 0.0, 0.0]];
    let draw = |moments: &[Vec<Pose>; 6]| {
        let mut sheet = Sheet::new(moments.len(), 2, cell);
        for (column, poses) in moments.iter().enumerate() {
            for (row, toward) in views.iter().enumerate() {
                let mut canvas = Canvas::new(cell.0, cell.1, [0.42, 0.44, 0.46], 1.0);
                canvas.model(&View::flat(cell.0, cell.1, *toward, [0.0, 0.92, 0.1], 2.1), &model, poses);
                sheet.put(column, row, &canvas.image);
            }
        }
        sheet.image
    };
    Ok([draw(&first), draw(&second)])
}

/// The soldier in its bind pose from ahead and from behind, large enough to tell its faces apart.
fn soldier_close(figures: &Figures, id: u32) -> Result<Image, String> {
    let model = figures.model(id).ok_or("the section lacks a soldier")?;
    let rest = vec![figure::UNMOVED; figure::bone::SOLDIER];
    let cell = (520, 940);
    let mut sheet = Sheet::new(3, 1, cell);
    for (column, toward) in [unit([-0.6, 0.18, 0.8]), unit([0.7, 0.25, -0.7]), [-1.0, 0.0, 0.0]].into_iter().enumerate() {
        let mut canvas = Canvas::new(cell.0, cell.1, [0.42, 0.44, 0.46], 1.0);
        canvas.model(&View::flat(cell.0, cell.1, toward, [0.0, 0.92, 0.1], 2.0), &model, &rest);
        sheet.put(column, 0, &canvas.image);
    }
    Ok(sheet.image)
}

fn held_sheets(figures: &Figures, weapon: u32, id: u32) -> Result<[Image; 2], String> {
    let model = figures.model(id).ok_or("the section lacks a held weapon")?;
    let info = figures.info(id).ok_or("the section lacks a held weapon's points")?;
    let rest = vec![figure::UNMOVED; part::WEAPON];
    let at = |which: u32, share: f32| -> Vec<Pose> {
        let c = clip::held(weapon, which);
        let frames = figures.clip(c).map_or(1, |r| r.clip.frames);
        frame(figures, c, ((frames - 1) as f32 * share).round() as u32).unwrap_or(rest.clone())
    };
    let aimed = vec![carried(info.ads_turn, info.ads_offset); part::WEAPON];
    let first = [rest.clone(), at(clip::RELOAD_TAC, 0.25), at(clip::RELOAD_TAC, 0.5), at(clip::RELOAD_TAC, 0.78)];
    let second = [aimed, at(clip::RELOAD_EMPTY, 0.88), at(clip::DRAW, 0.3), at(clip::HOLSTER, 0.6)];
    let cell = (480, 272);
    let draw = |moments: &[Vec<Pose>; 4]| {
        let mut sheet = Sheet::new(2, 2, cell);
        for (i, poses) in moments.iter().enumerate() {
            let mut canvas = Canvas::new(cell.0, cell.1, [0.62, 0.64, 0.66], 2.2);
            canvas.model(&View::eye(cell.0, cell.1, 60.0), &model, poses);
            sheet.put(i % 2, i / 2, &canvas.image);
        }
        sheet.image
    };
    Ok([draw(&first), draw(&second)])
}

/// The held weapon at rest from the eye at twice the handheld's size, to tell its faces apart.
fn held_close(figures: &Figures, id: u32) -> Result<Image, String> {
    let model = figures.model(id).ok_or("the section lacks a held weapon")?;
    let mut canvas = Canvas::new(960, 544, [0.62, 0.64, 0.66], 2.2);
    canvas.model(&View::eye(960, 544, 60.0), &model, &vec![figure::UNMOVED; part::WEAPON]);
    Ok(canvas.image)
}

pub fn run(ir: &Path, out: &Path, profile: &str) -> Result<(), String> {
    let profile: Value = serde_json::from_slice(&std::fs::read(profile).map_err(|e| format!("profile: {e}"))?).map_err(|e| format!("profile: {e}"))?;
    let bytes = crate::figures::cook(ir, &profile)?;
    if bytes.is_empty() {
        return Err("the export holds no figures (run `bun tools/duty.ts export --only figures` first)".into());
    }
    let dir = out.join("figures");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(out.join("figures.bin"), &bytes).map_err(|e| e.to_string())?;
    let figures = Figures::parse(&bytes).ok_or("the section does not read back")?;
    for (name, id) in SOLDIERS {
        let [first, second] = soldier_sheets(&figures, id)?;
        first.save(&dir.join(format!("{name}.png")))?;
        second.save(&dir.join(format!("{name}_more.png")))?;
        soldier_close(&figures, id)?.save(&dir.join(format!("{name}_close.png")))?;
    }
    for (weapon, (name, id)) in HELD.iter().enumerate() {
        let [first, second] = held_sheets(&figures, weapon as u32, *id)?;
        first.save(&dir.join(format!("{name}.png")))?;
        second.save(&dir.join(format!("{name}_more.png")))?;
        held_close(&figures, *id)?.save(&dir.join(format!("{name}_close.png")))?;
    }
    crate::figures::source_pictures(ir, &dir)?;
    println!("figures: {} KiB in {}, pictures in {}", bytes.len() / 1024, out.join("figures.bin").display(), dir.display());
    Ok(())
}
