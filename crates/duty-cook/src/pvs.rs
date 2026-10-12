//! What can be seen from where.
//!
//! The street is a canyon: from any place in it most of the map is behind a wall. For every cell
//! of the grid the compiler finds the places an eye can be (above every floor of the collision
//! world with room for a head), sends rays from them in every direction through the world's own
//! triangles, and lists the chunks the rays reach. The device draws from the list of the cell its
//! eye is in.

use duty_pack::{chunk, Chunk, Cluster, Scene, ViewCell, WorldVertex};
use duty_sim::bvh::{self, Bvh, Tri};
use duty_sim::math::{v3, V3};
use rayon::prelude::*;

use crate::collision::Collision;

pub struct Settings {
    /// Rays from each cell.
    pub rays: usize,
    /// A chunk this near a cell's box is listed whatever the rays say, metres.
    pub near: f32,
}

/// Light and sight pass: glass, and the holes of a cut-out.
const CLEAR: u32 = 1 << 31;

struct Random(u64);

impl Random {
    fn next(&mut self) -> f32 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        ((self.0.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 40) as f32) / (1u64 << 24) as f32
    }
}

/// Where an eye can be in a cell: points above each floor with room for a crouched head.
fn places(collision: &Bvh, scene: &Scene, cell: u32) -> Vec<V3> {
    let centre = scene.cell_centre(cell);
    let mut out = Vec::new();
    let steps = scene.cell.ceil() as i32;
    for j in 0..steps {
        for i in 0..steps {
            let (x, z) = (centre[0] - scene.cell * 0.5 + (i as f32 + 0.5) * scene.cell / steps as f32, centre[2] - scene.cell * 0.5 + (j as f32 + 0.5) * scene.cell / steps as f32);
            // Every floor of the column, from the top down.
            let mut y = scene.max[1] + 1.0;
            while let Some(hit) = collision.raycast(v3(x, y, z), v3(0.0, -1.0, 0.0), y - scene.min[1] + 2.0, 0) {
                let floor = y - hit.t;
                if hit.n.y > 0.6 && !collision.occluded(v3(x, floor + 0.2, z), V3::UP, 1.0, 0) {
                    out.push(v3(x, floor, z));
                }
                y = floor - 0.05;
            }
        }
    }
    out
}

pub struct Sight {
    pub cells: Vec<ViewCell>,
    pub lists: Vec<u16>,
    pub bits: Vec<u8>,
    /// Cells an eye can be in, and the chunks and clusters their lists hold on average.
    pub open: usize,
    pub chunks: usize,
    pub clusters: usize,
}

pub fn compute(scene: &Scene, chunks: &[Chunk], clusters: &[Cluster], vertices: &[WorldVertex], indices: &[u16], collision: &Collision, set: &Settings) -> Sight {
    // The world as the device draws it at each level of detail, each triangle tagged with its cluster.
    let mut worlds = Vec::new();
    for level in 0..chunk::LODS {
        let mut verts: Vec<[f32; 3]> = Vec::new();
        let mut tris: Vec<Tri> = Vec::new();
        for c in chunks {
            let lod = c.lod(level);
            if lod.clusters == 0 {
                continue;
            }
            let centre = scene.cell_centre(c.cell as u32);
            let base = verts.len() as u32;
            // The level's indices end where the last of its clusters' runs does.
            let end = clusters[lod.cluster as usize..lod.cluster as usize + lod.clusters as usize].iter().map(|cl| cl.first + cl.count as u32).max().unwrap_or(0);
            let count = indices[lod.index as usize..(lod.index + end) as usize].iter().map(|v| *v as usize + 1).max().unwrap_or(0);
            verts.extend(vertices[lod.vertex as usize..lod.vertex as usize + count].iter().map(|v| core::array::from_fn(|a| centre[a] + v.pos[a] as f32 * scene.quantum)));
            let clear = if c.flags & (chunk::BLEND | chunk::CUTOUT) != 0 { CLEAR } else { 0 };
            for id in lod.cluster..lod.cluster + lod.clusters as u32 {
                let cl = &clusters[id as usize];
                let run = &indices[(lod.index + cl.first) as usize..(lod.index + cl.first) as usize + cl.count as usize];
                tris.extend(run.chunks_exact(3).map(|t| Tri { v: [base + t[0] as u32, base + t[1] as u32, base + t[2] as u32], tag: id | clear }));
            }
        }
        let nodes = bvh::build(&verts, &mut tris);
        worlds.push((verts, tris, nodes));
    }
    let walk = collision.bvh();
    let cells = (scene.cells_x * scene.cells_z) as usize;
    let bytes = clusters.len().div_ceil(8);
    // The chunk each cluster belongs to.
    let mut owner = vec![0u16; clusters.len()];
    for (i, c) in chunks.iter().enumerate() {
        for lod in c.levels() {
            owner[lod.cluster as usize..lod.cluster as usize + lod.clusters as usize].fill(i as u16);
        }
    }
    let seen: Vec<Option<Vec<u8>>> = (0..cells)
        .into_par_iter()
        .map(|cell| {
            let places = places(&walk, scene, cell as u32);
            if places.is_empty() {
                return None;
            }
            let mut bits = vec![0u8; bytes];
            let levels: Vec<Bvh> = worlds.iter().map(|(verts, tris, nodes)| Bvh { nodes, tris, verts }).collect();
            let mut random = Random(0x9e37_79b9_7f4a_7c15 ^ (cell as u64 + 1).wrapping_mul(0xbf58_476d_1ce4_e5b9));
            let jitter = scene.cell / scene.cell.ceil();
            for _ in 0..set.rays {
                let place = places[(random.next() * places.len() as f32) as usize % places.len()];
                // Anywhere a head can be over that floor: crouched, standing, at the top of a jump.
                let from = place + v3((random.next() - 0.5) * jitter, 0.9 + random.next() * 1.5, (random.next() - 0.5) * jitter);
                let (u, phi) = (random.next() * 2.0 - 1.0, random.next() * core::f32::consts::TAU);
                let r = (1.0 - u * u).max(0.0).sqrt();
                let d = v3(r * phi.cos(), u, r * phi.sin());
                // A frame mixes the levels: what is near is drawn fine, what is far coarse, and the
                // device moves the line between them with the frame's load. So what hides a thing
                // at one level may not be there to hide it: along a ray, every level's clusters
                // count as seen as far as the farthest of the three levels' first walls.
                let mut reach = 0.0f32;
                for world in &levels {
                    let (mut at, mut gone) = (from, 0.0f32);
                    let mut wall = 400.0;
                    for _ in 0..6 {
                        let Some(hit) = world.raycast(at, d, 400.0 - gone, 0) else { break };
                        gone += hit.t + 0.01;
                        if hit.tag & CLEAR == 0 {
                            wall = gone;
                            break;
                        }
                        at = from + d * gone;
                    }
                    reach = reach.max(wall);
                }
                for world in &levels {
                    let (mut at, mut gone) = (from, 0.0f32);
                    for _ in 0..12 {
                        let Some(hit) = world.raycast(at, d, (reach + 0.05 - gone).max(0.0), 0) else { break };
                        let id = (hit.tag & !CLEAR) as usize;
                        bits[id >> 3] |= 1 << (id & 7);
                        gone += hit.t + 0.01;
                        if gone >= reach {
                            break;
                        }
                        at = from + d * gone;
                    }
                }
            }
            // What stands right beside the cell is seen whatever the rays met.
            let centre = scene.cell_centre(cell as u32);
            let half = scene.cell * 0.5 + set.near;
            for c in chunks {
                if c.min[0] < centre[0] + half && c.max[0] > centre[0] - half && c.min[2] < centre[2] + half && c.max[2] > centre[2] - half {
                    for lod in c.levels() {
                        for id in lod.cluster as usize..lod.cluster as usize + lod.clusters as usize {
                            bits[id >> 3] |= 1 << (id & 7);
                        }
                    }
                }
            }
            Some(bits)
        })
        .collect();
    let mut sight = Sight { cells: Vec::with_capacity(cells), lists: Vec::new(), bits: Vec::new(), open: 0, chunks: 0, clusters: 0 };
    for bits in &seen {
        let Some(bits) = bits else {
            sight.cells.push(ViewCell::default());
            continue;
        };
        let mut listed = vec![false; chunks.len()];
        for id in 0..clusters.len() {
            if bits[id >> 3] >> (id & 7) & 1 == 1 {
                listed[owner[id] as usize] = true;
                sight.clusters += 1;
            }
        }
        let first = sight.lists.len();
        sight.lists.extend(listed.iter().enumerate().filter(|l| *l.1).map(|l| l.0 as u16));
        sight.cells.push(ViewCell { first: first as u32, count: (sight.lists.len() - first) as u32, bits: sight.bits.len() as u32 });
        sight.bits.extend_from_slice(bits);
        sight.open += 1;
    }
    sight.chunks = sight.lists.len() / sight.open.max(1);
    sight.clusters /= sight.open.max(1);
    sight
}
