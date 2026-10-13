//! The soldiers and the weapon in the player's hands: the pack's figures
//! (`duty_pack::figure`), posed from their clips as the PSP poses them
//! (`psp/src/figures.rs`).
//!
//! A batch is one draw of up to four bones. A bone's matrix is the clip's
//! pose for it between two frames; the draw's own matrix carries the figure
//! to where it stands. A figure is lit as the GE lights it: the sky's share
//! on every side, the sun's on the sides turned to it, and the sun's only
//! where the ground's shadow picture has sun at the figure's feet. The GE
//! blends and lights in its vertex stage; here the vertex program does
//! (`shaders/scene.wgsl`, `skin_vertex`), from one record a batch.

use duty_pack::figure::{bone, clip, model, part, Batch, Clip, FigureHeader, Info, Model, Pose, SkinVertex};
use duty_pack::{self as pack, Scene};
use duty_sim::game::Game;
use duty_sim::mat::{self, Mat4};
use duty_sim::math::*;
use duty_sim::soldier::{Soldier, State};
use duty_sim::tune::soldier as rules;
use duty_sim::weapon::Action;

/// Floats in a batch's record: the matrix to the screen, four bones of three rows, and the light
/// (toward the sun, the sun's colour, up, the sky's colour), each of four.
pub const RECORD: usize = 16 + 48 + 16;
/// Metres from the eye at which a soldier is drawn from his plainer model.
const FAR_FROM: f32 = 16.0;
/// The held weapon's own frustum: degrees top to bottom at the hip, and its near and far planes.
const HELD_FOV: f32 = 60.0;
const HELD_NEAR: f32 = 0.02;
const HELD_FAR: f32 = 6.0;

const IDENTITY: Pose = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]];

/// One draw of a model: which of the model's bones its four weights move, and where its
/// triangles are in the buffers.
#[derive(Clone, Copy, Debug)]
pub struct BatchAt {
    pub bones: [u8; 4],
    pub count: u32,
    /// The batch's first vertex among all the figures' vertices, and its first index among all their indices.
    pub vertex: u32,
    pub first: u32,
    pub indices: u32,
}

struct ModelAt {
    id: u32,
    batches: Vec<BatchAt>,
}

struct ClipAt {
    head: Clip,
    poses: Vec<Pose>,
}

/// A batch to draw this frame: its record's number, and its triangles.
#[derive(Clone, Copy, Debug)]
pub struct Drawn {
    pub record: u32,
    pub vertex: u32,
    pub first: u32,
    pub indices: u32,
}

pub struct Figures {
    models: Vec<ModelAt>,
    clips: Vec<ClipAt>,
    infos: Vec<Info>,
    /// Every batch's vertices (`SkinVertex`, 24 bytes each) and indices, in the pack's order.
    pub vertices: Vec<u8>,
    pub indices: Vec<u16>,
}

fn blend(a: &Pose, b: &Pose, t: f32) -> Pose {
    core::array::from_fn(|r| core::array::from_fn(|c| a[r][c] + (b[r][c] - a[r][c]) * t))
}

/// `a` after `b`.
fn after(a: &Pose, b: &Pose) -> Pose {
    core::array::from_fn(|r| core::array::from_fn(|c| a[r][0] * b[0][c] + a[r][1] * b[1][c] + a[r][2] * b[2][c] + if c == 3 { a[r][3] } else { 0.0 }))
}

fn apply(m: &Pose, p: [f32; 3]) -> V3 {
    v3(m[0][0] * p[0] + m[0][1] * p[1] + m[0][2] * p[2] + m[0][3], m[1][0] * p[0] + m[1][1] * p[1] + m[1][2] * p[2] + m[1][3], m[2][0] * p[0] + m[2][1] * p[1] + m[2][2] * p[2] + m[2][3])
}

/// A turn of `angle` about the axis `axis` through the point `at`, then a move by `by`.
fn turn_about(axis: V3, angle: f32, at: V3, by: V3) -> Pose {
    let (s, c) = (sin(angle), cos(angle));
    let (x, y, z) = (axis.x, axis.y, axis.z);
    let r = [[c + x * x * (1.0 - c), x * y * (1.0 - c) - z * s, x * z * (1.0 - c) + y * s], [y * x * (1.0 - c) + z * s, c + y * y * (1.0 - c), y * z * (1.0 - c) - x * s], [z * x * (1.0 - c) - y * s, z * y * (1.0 - c) + x * s, c + z * z * (1.0 - c)]];
    let moved: [f32; 3] = core::array::from_fn(|i| [at.x, at.y, at.z][i] - (r[i][0] * at.x + r[i][1] * at.y + r[i][2] * at.z) + [by.x, by.y, by.z][i]);
    core::array::from_fn(|i| [r[i][0], r[i][1], r[i][2], moved[i]])
}

/// Three turns about the eye's axes, the one about z first, then the one about y, then x (three.js
/// writes this order as XYZ), then a move: the reference's own way of writing where the hands hold
/// a weapon.
fn euler(turn: [f32; 3], by: [f32; 3]) -> Pose {
    let x = turn_about(v3(1.0, 0.0, 0.0), turn[0], V3::ZERO, v3(by[0], by[1], by[2]));
    let y = turn_about(v3(0.0, 1.0, 0.0), turn[1], V3::ZERO, V3::ZERO);
    let z = turn_about(v3(0.0, 0.0, 1.0), turn[2], V3::ZERO, V3::ZERO);
    after(&x, &after(&y, &z))
}

/// The sun's colour where `lit` of it reaches the ground. The GE takes a light's colour as 8 bits a
/// part: more than all of a part is all of it.
fn sun_light(scene: &Scene, lit: f32) -> [f32; 4] {
    [saturate(scene.sun_colour[0] * lit), saturate(scene.sun_colour[1] * lit), saturate(scene.sun_colour[2] * lit), 0.0]
}

impl Figures {
    /// The figures of the pack's `FIGS` section.
    pub fn parse(bytes: &[u8]) -> Result<Figures, String> {
        let short = || String::from("the pack's figures are cut short");
        let head: FigureHeader = pack::read(bytes, 0).ok_or_else(short)?;
        let mut at = core::mem::size_of::<FigureHeader>();
        let (mut models, mut vertices, mut indices) = (Vec::new(), Vec::new(), Vec::new());
        for _ in 0..head.models {
            let m: Model = pack::read(bytes, at).ok_or_else(short)?;
            at += core::mem::size_of::<Model>();
            let mut batches = Vec::new();
            for _ in 0..m.batches {
                let b: Batch = pack::read(bytes, at).ok_or_else(short)?;
                at += core::mem::size_of::<Batch>();
                let stride = core::mem::size_of::<SkinVertex>();
                let (from, then) = (at, at + b.vertices as usize * stride);
                at = (then + b.indices as usize * 2 + 3) & !3;
                if at > bytes.len() {
                    return Err(short());
                }
                batches.push(BatchAt { bones: [b.bones[0], b.bones[1], b.bones[2], b.bones[3]], count: b.bone_count.min(4), vertex: (vertices.len() / stride) as u32, first: indices.len() as u32, indices: b.indices });
                vertices.extend_from_slice(&bytes[from..then]);
                indices.extend(bytes[then..then + b.indices as usize * 2].chunks_exact(2).map(|i| u16::from_le_bytes([i[0], i[1]])));
            }
            models.push(ModelAt { id: m.id, batches });
        }
        let mut clips = Vec::new();
        for _ in 0..head.clips {
            let c: Clip = pack::read(bytes, at).ok_or_else(short)?;
            at += core::mem::size_of::<Clip>();
            let count = (c.frames * c.bones) as usize;
            let poses = (0..count).map(|i| pack::read::<Pose>(bytes, at + i * core::mem::size_of::<Pose>()).ok_or_else(short)).collect::<Result<Vec<_>, _>>()?;
            at += count * core::mem::size_of::<Pose>();
            clips.push(ClipAt { head: c, poses });
        }
        let infos = (0..head.infos as usize).filter_map(|i| pack::read::<Info>(bytes, at + i * core::mem::size_of::<Info>())).collect();
        Ok(Figures { models, clips, infos, vertices, indices })
    }

    fn info(&self, model: u32) -> Option<&Info> {
        self.infos.iter().find(|i| i.model == model)
    }

    fn model(&self, id: u32) -> Option<&ModelAt> {
        self.models.iter().find(|m| m.id == id)
    }

    /// A bone's pose in a clip at `at`: turns of the clip when it loops, seconds when it does not.
    fn pose(&self, id: u32, bone: usize, at: f32) -> Pose {
        let Some(c) = self.clips.iter().find(|c| c.head.id == id) else { return IDENTITY };
        let (frames, bones) = (c.head.frames as usize, c.head.bones as usize);
        if frames == 0 || bone >= bones {
            return IDENTITY;
        }
        let f = if c.head.looped != 0 { (at - floor(at)) * frames as f32 } else { clamp(at * c.head.rate, 0.0, frames as f32 - 1.0) };
        let i = (f as usize).min(frames - 1);
        let next = if c.head.looped != 0 { (i + 1) % frames } else { (i + 1).min(frames - 1) };
        blend(&c.poses[i * bones + bone], &c.poses[next * bones + bone], f - i as f32)
    }

    /// A model's batches as records and draws: `clip` carries the figure's own space to the screen, and
    /// the light is said in that space. Returns the triangles.
    #[allow(clippy::too_many_arguments)]
    fn batches(&self, id: u32, poses: &[Pose], clip: &Mat4, light: &[[f32; 4]; 4], records: &mut Vec<f32>, drawn: &mut Vec<Drawn>) -> u32 {
        let Some(m) = self.model(id) else { return 0 };
        let mut triangles = 0;
        for b in &m.batches {
            let record = (records.len() / RECORD) as u32;
            records.extend_from_slice(clip);
            for slot in 0..4 {
                // (a slot the batch does not use moves nothing: no vertex has a weight for it)
                let pose = if slot < b.count as usize { &poses[(b.bones[slot] as usize).min(poses.len() - 1)] } else { &IDENTITY };
                for row in pose {
                    records.extend_from_slice(row);
                }
            }
            for part in light {
                records.extend_from_slice(part);
            }
            drawn.push(Drawn { record, vertex: b.vertex, first: b.first, indices: b.indices });
            triangles += b.indices / 3;
        }
        triangles
    }

    /// The soldiers, standing and fallen, as the eye at `eye` looking along `look` sees them through
    /// `vp`. `sunlit` says how much of the sun reaches the ground at a place. Returns the triangles.
    #[allow(clippy::too_many_arguments)]
    pub fn soldiers(&self, scene: &Scene, vp: &Mat4, eye: V3, look: V3, game: &Game, sunlit: impl Fn(f32, f32) -> f32, records: &mut Vec<f32>, drawn: &mut Vec<Drawn>) -> u32 {
        let mut triangles = 0;
        let sun = v3(scene.sun[0], scene.sun[1], scene.sun[2]);
        for s in game.soldiers.iter().filter(|s| s.used) {
            let to = s.pos + v3(0.0, 0.9, 0.0) - eye;
            let distance = to.len();
            // Behind the eye, or too far to be more than a pixel.
            if distance > 110.0 || to.dot(look) < -1.5 {
                continue;
            }
            let mut poses = [IDENTITY; bone::SOLDIER];
            self.pose_soldier(s, &mut poses);
            // The model faces +Z; a soldier's heading at yaw 0 is -Z.
            let (sn, cs) = (sin(s.yaw + PI), cos(s.yaw + PI));
            let root: Mat4 = [cs, 0.0, sn, s.pos.x, 0.0, 1.0, 0.0, s.pos.y, -sn, 0.0, cs, s.pos.z, 0.0, 0.0, 0.0, 1.0];
            // The light, in the soldier's own space: the turn that stood him there, undone.
            let own = |d: V3| [cs * d.x - sn * d.z, d.y, sn * d.x + cs * d.z, 0.0];
            let lit = sunlit(s.pos.x, s.pos.z);
            let light = [own(sun), sun_light(scene, lit), own(V3::UP), [scene.sky_colour[0], scene.sky_colour[1], scene.sky_colour[2], 0.0]];
            // Far off, his plainer drawing when the pack has one: at 16 m he is 24 pixels tall.
            let id = model::VANGUARD + s.variant as u32 % 3;
            let plain = id + model::FAR;
            let id = if distance > FAR_FROM && self.model(plain).is_some() { plain } else { id };
            triangles += self.batches(id, &poses, &mat::mul(vp, &root), &light, records, drawn);
        }
        triangles
    }

    /// A soldier's bones: his clip, the one he leaves, what plays over them, his aim, his fall.
    fn pose_soldier(&self, s: &Soldier, poses: &mut [Pose; bone::SOLDIER]) {
        let still = matches!(s.clip, clip::AIM | clip::CROUCH_AIM | clip::IDLE);
        // What plays instead of standing still: a flinch, a change of magazine, a round's kick.
        let over = if s.state == State::Dead || !still {
            None
        } else if s.hit_ago < 0.55 {
            Some((clip::HIT, s.hit_ago))
        } else if s.reload > 0.0 {
            Some((clip::RELOAD, rules::RELOAD[(s.variant == 1) as usize] - s.reload))
        } else if s.fired_ago < 0.3 && s.clip == clip::AIM {
            Some((clip::FIRE, s.fired_ago))
        } else {
            None
        };
        for (b, pose) in poses.iter_mut().enumerate() {
            *pose = match over {
                Some((id, at)) => self.pose(id, b, at),
                None => {
                    let now = self.pose(s.clip, b, s.phase);
                    if s.blend < 1.0 {
                        blend(&self.pose(s.clip_before, b, s.phase_before), &now, s.blend)
                    } else {
                        now
                    }
                }
            };
        }
        let Some(info) = self.info(model::VANGUARD) else { return };
        if abs(s.aim_pitch) > 0.01 && s.state != State::Dead {
            // The chest, the arms and the head turn up or down about the small of the back. The
            // model faces +Z with its right at -X: a turn about +X lowers what is ahead.
            let pivot = apply(&poses[bone::SPINE1], info.joints[bone::SPINE1]);
            let lift = turn_about(v3(1.0, 0.0, 0.0), -s.aim_pitch, pivot, V3::ZERO);
            for pose in poses.iter_mut().take(bone::FINGERS_L + 1).skip(bone::SPINE1) {
                *pose = after(&lift, pose);
            }
        }
        if s.state == State::Dead {
            // He goes over backwards from the round that felled him, faster as he falls, and lies.
            let t = min(s.dead_time / 0.62, 1.0);
            let away = wrap_angle(s.fall_yaw - s.yaw);
            // In his own space the way he falls: -Z is his front.
            let axis = v3(cos(away), 0.0, sin(away));
            let fall = turn_about(axis, t * t * 1.5, V3::ZERO, v3(0.0, 0.08 * t, 0.0));
            for pose in poses.iter_mut() {
                *pose = after(&fall, pose);
            }
        }
    }

    /// The weapon in the player's hands, in a frustum of its own. `view` is the eye's view matrix.
    /// Returns the frustum's matrix, where the muzzle is in the eye's own axes, and the triangles.
    #[allow(clippy::too_many_arguments)]
    pub fn held(&self, scene: &Scene, view: &Mat4, aspect: f32, game: &Game, sunlit: impl Fn(f32, f32) -> f32, records: &mut Vec<f32>, drawn: &mut Vec<Drawn>) -> Option<(Mat4, V3, u32)> {
        let arms = &game.arms;
        let id = model::RIFLE + arms.active as u32;
        if game.player.dead || self.model(id).is_none() {
            return None;
        }
        let info = *self.info(id)?;
        let w = arms.active as u32;
        let (which, at) = match arms.action {
            Action::None => (clip::REST, 0.0),
            Action::Reload { empty, at, .. } => (if empty { clip::RELOAD_EMPTY } else { clip::RELOAD_TAC }, at),
            Action::Holster { at, .. } => (clip::HOLSTER, at),
            Action::Draw { at } => (clip::DRAW, at),
        };
        // Where the hands carry everything: up to the sights, back with a round, down in a sprint,
        // and with the walk.
        let aim = smoothstep(0.0, 1.0, arms.ads);
        let player = &game.player;
        let stride = sin(game.time * 9.0) * min(player.speed() / 4.57, 1.4) * lerp(1.0, 0.25, aim);
        // The sprint's turn and place are the weapon's own, from the pack, as the sights' are
        // (`duty_sim::pose::Posed::held`, which the consoles draw from, reads the same).
        let sprint = arms.sprint;
        let (st, so) = (info.sprint_turn, info.sprint_offset);
        let carry = euler(
            [info.ads_turn[0] * aim + st[0] * sprint - arms.back * 1.6, info.ads_turn[1] * aim + st[1] * sprint, info.ads_turn[2] * aim + st[2] * sprint + arms.roll],
            [info.ads_offset[0] * aim + so[0] * sprint + stride * 0.006, info.ads_offset[1] * aim + so[1] * sprint - abs(stride) * 0.005, info.ads_offset[2] * aim + so[2] * sprint + arms.back],
        );
        let mut poses = [IDENTITY; part::WEAPON];
        for (b, pose) in poses.iter_mut().enumerate() {
            *pose = after(&carry, &self.pose(clip::held(w, which), b, at));
        }
        // The bolt goes back along the weapon's own length, which at the hip is not the eye's: it
        // is the eye's when the sights are up, so the turn that raises them says where it lies. A
        // change of an empty magazine moves the bolt itself.
        if arms.bolt > 0.0 && !matches!(arms.action, Action::Reload { empty: true, .. }) {
            let (body, raise) = (&poses[part::BODY], euler(info.ads_turn, [0.0; 3]));
            let along = raise[2];
            let back = v3(
                body[0][0] * along[0] + body[0][1] * along[1] + body[0][2] * along[2],
                body[1][0] * along[0] + body[1][1] * along[1] + body[1][2] * along[2],
                body[2][0] * along[0] + body[2][1] * along[1] + body[2][2] * along[2],
            ) * (info.bolt_travel * arms.bolt);
            let slide = turn_about(V3::UP, 0.0, V3::ZERO, back);
            poses[part::BOLT] = after(&slide, &poses[part::BOLT]);
        }
        let frustum = mat::perspective(HELD_FOV * lerp(1.0, 0.86, aim), aspect, HELD_NEAR, HELD_FAR);
        // The sun's direction and the world's up as the eye sees them: the weapon's space is the eye's.
        let seen = |d: V3| [view[0] * d.x + view[1] * d.y + view[2] * d.z, view[4] * d.x + view[5] * d.y + view[6] * d.z, view[8] * d.x + view[9] * d.y + view[10] * d.z, 0.0];
        let lit = sunlit(player.body.pos.x, player.body.pos.z);
        let light = [seen(v3(scene.sun[0], scene.sun[1], scene.sun[2])), sun_light(scene, lit), seen(V3::UP), [scene.sky_colour[0], scene.sky_colour[1], scene.sky_colour[2], 0.0]];
        let triangles = self.batches(id, &poses, &frustum, &light, records, drawn);
        Some((frustum, apply(&poses[part::BODY], info.muzzle), triangles))
    }
}
