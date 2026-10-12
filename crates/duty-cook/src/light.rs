//! The light the pack bakes: the sun through everything the reference draws, the sky as the
//! reference renders it, and one bounce of both off what the sky's rays meet.
//!
//! The devices have no lighting stage that could do this: a vertex's colour is the surface's colour
//! under this light, as the display shows it.

use std::path::Path;

use duty_sim::bvh::{self, Bvh, Node};
use duty_sim::math::{v3, V3};

use crate::ir::Dir;
use crate::world::World;

/// The sky as the reference's dome program drew it: six square faces of linear radiance.
pub struct Sky {
    side: usize,
    faces: Vec<Face>,
    /// The same at 16 texels a side: what a surface's rays read, so a few dozen of them see the whole sky.
    coarse: Vec<Vec<[f32; 3]>>,
    pub sun: V3,
    /// The sun's irradiance on a face turned to it.
    pub sun_colour: [f32; 3],
    pub hour: f32,
}

struct Face {
    right: V3,
    up: V3,
    forward: V3,
    data: Vec<f32>,
}

const COARSE: usize = 16;

impl Sky {
    pub fn open(ir: &Path) -> Result<Sky, String> {
        let dir = Dir::open(&ir.join("sky.dir"))?;
        let side = dir.meta["side"].as_u64().ok_or("sky.dir: no side")? as usize;
        let vector = |v: &serde_json::Value| v3(v[0].as_f64().unwrap_or(0.0) as f32, v[1].as_f64().unwrap_or(0.0) as f32, v[2].as_f64().unwrap_or(0.0) as f32);
        let mut faces = Vec::new();
        for f in dir.meta["faces"].as_array().ok_or("sky.dir: no faces")? {
            faces.push(Face { right: vector(&f["right"]), up: vector(&f["up"]), forward: vector(&f["forward"]), data: dir.f32s(f["name"].as_str().unwrap_or("")) });
        }
        let block = side / COARSE;
        let coarse = faces
            .iter()
            .map(|f| {
                let mut out = vec![[0.0f32; 3]; COARSE * COARSE];
                for y in 0..side {
                    for x in 0..side {
                        let o = &mut out[(y / block) * COARSE + x / block];
                        for k in 0..3 {
                            // The sun's disc is the sun's light, which the bake adds by itself.
                            o[k] += f.data[(y * side + x) * 3 + k].min(40.0) / (block * block) as f32;
                        }
                    }
                }
                out
            })
            .collect();
        let sun = &dir.meta["sun"];
        let intensity = sun["intensity"].as_f64().unwrap_or(1.0) as f32;
        let colour = vector(&sun["colour"]);
        Ok(Sky { side, faces, coarse, sun: vector(&sun["direction"]).norm(), sun_colour: [colour.x * intensity, colour.y * intensity, colour.z * intensity], hour: dir.meta["hour"].as_f64().unwrap_or(12.0) as f32 })
    }

    fn texel(&self, d: V3, side: usize) -> (usize, usize) {
        let (mut best, mut at) = (f32::MIN, 0);
        for (i, f) in self.faces.iter().enumerate() {
            let along = d.dot(f.forward);
            if along > best {
                best = along;
                at = i;
            }
        }
        let f = &self.faces[at];
        let (x, y) = (d.dot(f.right) / best, d.dot(f.up) / best);
        let px = (((x + 1.0) * 0.5 * side as f32) as usize).min(side - 1);
        let py = (((1.0 - y) * 0.5 * side as f32) as usize).min(side - 1);
        (at, py * side + px)
    }

    /// The sky's radiance in a direction, at full detail.
    pub fn radiance(&self, d: V3) -> [f32; 3] {
        let (face, at) = self.texel(d, self.side);
        let data = &self.faces[face].data;
        [data[at * 3], data[at * 3 + 1], data[at * 3 + 2]]
    }

    /// The same, averaged over a patch of sky.
    pub fn soft(&self, d: V3) -> [f32; 3] {
        let (face, at) = self.texel(d, COARSE);
        self.coarse[face][at]
    }
}

/// Bits of an occluder's tag above its material number.
pub mod tag {
    /// Light passes: glass.
    pub const CLEAR: u32 = 1 << 30;
    pub const MATERIAL: u32 = 0xffff;
}

pub struct Lighting {
    verts: Vec<[f32; 3]>,
    tris: Vec<bvh::Tri>,
    nodes: Vec<Node>,
    pub sky: Sky,
    rays: Vec<[f32; 3]>,
}

/// Directions over the half space about +Y, their density following the cosine.
fn hemisphere(n: usize) -> Vec<[f32; 3]> {
    (0..n)
        .map(|i| {
            let u = (i as f32 + 0.5) / n as f32;
            // The golden angle turns each ray from the last.
            let phi = i as f32 * 2.399_963_2;
            let r = u.sqrt();
            [r * phi.cos(), (1.0 - u).max(0.0).sqrt(), r * phi.sin()]
        })
        .collect()
}

impl Lighting {
    /// Everything the reference draws stands in the light's way.
    pub fn new(world: &World, sky: Sky, rays: usize) -> Lighting {
        let mut verts = Vec::new();
        let mut tris = Vec::new();
        for soup in &world.soups {
            let material = &world.materials[soup.material];
            let base = verts.len() as u32;
            verts.extend(soup.verts.iter().map(|v| v.pos));
            let tag = soup.material as u32 | if material.transparent { tag::CLEAR } else { 0 };
            // A cut-out's faces are mostly holes: leaves let the light through.
            if material.cutout {
                continue;
            }
            tris.extend(soup.idx.chunks_exact(3).map(|t| bvh::Tri { v: [base + t[0], base + t[1], base + t[2]], tag }));
        }
        let nodes = bvh::build(&verts, &mut tris);
        Lighting { verts, tris, nodes, sky, rays: hemisphere(rays) }
    }

    fn bvh(&self) -> Bvh<'_> {
        Bvh { nodes: &self.nodes, tris: &self.tris, verts: &self.verts }
    }

    pub fn triangles(&self) -> usize {
        self.tris.len()
    }

    /// What a ray meets first: how far along it, and the material's number.
    pub fn first(&self, from: V3, d: V3) -> Option<(f32, usize)> {
        self.bvh().raycast(from, d, 400.0, tag::CLEAR).map(|hit| (hit.t, (hit.tag & tag::MATERIAL) as usize))
    }

    /// The height of the ground (or of what lies on it) at a point of the map.
    pub fn ground(&self, x: f32, z: f32) -> f32 {
        self.bvh().raycast(v3(x, 0.9, z), v3(0.0, -1.0, 0.0), 4.0, tag::CLEAR).map_or(0.0, |hit| 0.9 - hit.t)
    }

    /// How much of the sun a point sees, 0…1: rays to points across the sun's disc, widened to `spread` radians.
    pub fn sun_seen(&self, p: V3, spread: f32, samples: usize) -> f32 {
        let l = self.sky.sun;
        let side = l.cross(V3::UP).norm_or(v3(1.0, 0.0, 0.0));
        let up = side.cross(l);
        let bvh = self.bvh();
        let mut seen = 0.0;
        for i in 0..samples {
            let (a, r) = (i as f32 * 2.399_963_2, ((i as f32 + 0.5) / samples as f32).sqrt() * spread);
            let d = (l + side * (a.cos() * r) + up * (a.sin() * r)).norm();
            if !bvh.occluded(p, d, 400.0, tag::CLEAR) {
                seen += 1.0;
            }
        }
        seen / samples as f32
    }

    /// How much of the half space in front of a surface is open, 0…1, weighted by the cosine: what is
    /// left of the sky's fill under a balcony or in a room.
    pub fn open(&self, p: V3, n: V3) -> f32 {
        let bvh = self.bvh();
        // Off the surface, clear of the layers the reference lays a few millimetres over a wall.
        let from = p + n * 0.03;
        let t = if n.y.abs() < 0.9 { n.cross(V3::UP).norm() } else { n.cross(v3(1.0, 0.0, 0.0)).norm() };
        let b = n.cross(t);
        let mut open = 0.0;
        for r in &self.rays {
            let d = t * r[0] + n * r[1] + b * r[2];
            if !bvh.occluded(from, d, 60.0, tag::CLEAR) {
                open += 1.0;
            }
        }
        open / self.rays.len() as f32
    }
}

/// The reference's display transform as the export recorded it: its exposure, tone curve and grade
/// run on a table of colours whose channels step evenly in stops.
pub struct Tone {
    side: usize,
    low: f32,
    high: f32,
    table: Vec<u8>,
}

impl Tone {
    pub fn open(ir: &Path) -> Result<Tone, String> {
        let dir = Dir::open(&ir.join("tone.dir"))?;
        Ok(Tone { side: dir.meta["side"].as_u64().ok_or("tone.dir: no side")? as usize, low: dir.meta["low"].as_f64().unwrap_or(-9.0) as f32, high: dir.meta["high"].as_f64().unwrap_or(5.0) as f32, table: dir.u8s("table").to_vec() })
    }

    /// From linear radiance to the display's code values, 0…1.
    pub fn display(&self, linear: [f32; 3]) -> [f32; 3] {
        let n = self.side;
        // Where each channel stands between the table's steps.
        let at = linear.map(|c| ((c.max(1e-6).log2() - self.low) / (self.high - self.low)).clamp(0.0, 1.0) * (n - 1) as f32);
        let base = at.map(|a| (a as usize).min(n - 2));
        let f: [f32; 3] = core::array::from_fn(|k| at[k] - base[k] as f32);
        let mut out = [0.0f32; 3];
        for corner in 0..8 {
            let (i, j, k) = (corner & 1, (corner >> 1) & 1, corner >> 2);
            let w = (if i == 1 { f[0] } else { 1.0 - f[0] }) * (if j == 1 { f[1] } else { 1.0 - f[1] }) * (if k == 1 { f[2] } else { 1.0 - f[2] });
            let o = (((base[2] + k) * n + base[1] + j) * n + base[0] + i) * 3;
            for c in 0..3 {
                out[c] += self.table[o + c] as f32 / 255.0 * w;
            }
        }
        // Below the table's first step the display goes to black with the light.
        let floor = 2.0f32.powf(self.low);
        core::array::from_fn(|c| if linear[c] < floor { out[c] * (linear[c].max(0.0) / floor) } else { out[c] })
    }
}
