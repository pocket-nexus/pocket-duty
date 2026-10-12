//! From the reference's triangles to what a handheld draws: the world cut into cells, each cell's
//! share of each material simplified to three levels of detail that meet their neighbours without
//! a gap.

use std::collections::HashMap;

use meshopt::{SimplifyOptions, VertexDataAdapter};
use rayon::prelude::*;

use crate::world::{Material, Soup, World};

pub const LODS: usize = 3;

#[derive(Clone, Copy, Debug)]
pub struct Grid {
    pub min: [f32; 2],
    pub cell: f32,
    pub nx: usize,
    pub nz: usize,
}

impl Grid {
    pub fn cell_of(&self, x: f32, z: f32) -> usize {
        let i = (((x - self.min[0]) / self.cell).floor() as i64).clamp(0, self.nx as i64 - 1) as usize;
        let j = (((z - self.min[1]) / self.cell).floor() as i64).clamp(0, self.nz as i64 - 1) as usize;
        j * self.nx + i
    }
}

/// A triangle's corner: one triangle's own, so two faces that meet at a hard edge each keep their light.
#[derive(Clone, Copy, Debug, Default)]
pub struct Corner {
    pub pos: [f32; 3],
    pub nrm: [f32; 3],
    /// The surface's colour before light, gamma 2.2.
    pub col: [f32; 3],
    /// The light it sends back with the sun off, linear.
    pub amb: [f32; 3],
    pub uv: [f32; 2],
}

pub type Tri = [Corner; 3];

/// One material's triangles in one cell.
pub struct Piece {
    pub cell: usize,
    pub material: usize,
    pub detail: bool,
    /// Flat ground: its sun shadow is the shadow picture's, not its vertices'.
    pub ground: bool,
    pub lods: [Vec<Tri>; LODS],
}

pub struct Settings {
    /// Metres a level of detail may move a surface.
    pub error: [f32; LODS],
    /// Flat faces of these families below this height are ground.
    pub ground_families: Vec<String>,
    pub ground_below: f32,
    /// Triangles smaller than this area (m²) are left out of the second and third level.
    pub min_area: [f32; LODS],
}

pub fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
pub fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub fn len(a: [f32; 3]) -> f32 {
    dot(a, a).sqrt()
}
pub fn face_normal(t: &Tri) -> ([f32; 3], f32) {
    let n = cross(sub(t[1].pos, t[0].pos), sub(t[2].pos, t[0].pos));
    let l = len(n);
    if l > 1e-12 {
        ([n[0] / l, n[1] / l, n[2] / l], l * 0.5)
    } else {
        ([0.0, 1.0, 0.0], 0.0)
    }
}

/// A position as a key: tenths of a millimetre.
fn key(p: [f32; 3]) -> [i32; 3] {
    [(p[0] * 10000.0).round() as i32, (p[1] * 10000.0).round() as i32, (p[2] * 10000.0).round() as i32]
}

const SHARED: u32 = u32::MAX;

/// One cell's triangles of a soup, at each level of detail.
fn simplify_cell(soup: &Soup, material: &Material, tris: &[u32], shared: &HashMap<[i32; 3], u32>, set: &Settings) -> [Vec<Tri>; LODS] {
    // The cell's own vertex list.
    let mut local: HashMap<u32, u32> = HashMap::new();
    let mut verts: Vec<u32> = Vec::new();
    let mut idx: Vec<u32> = Vec::with_capacity(tris.len() * 3);
    for &t in tris {
        for k in 0..3 {
            let v = soup.idx[t as usize * 3 + k];
            let at = *local.entry(v).or_insert_with(|| {
                verts.push(v);
                verts.len() as u32 - 1
            });
            idx.push(at);
        }
    }
    let positions: Vec<u8> = verts.iter().flat_map(|&v| soup.verts[v as usize].pos.iter().flat_map(|c| c.to_le_bytes()).collect::<Vec<u8>>()).collect();
    // A point other cells use stays where it is at every level: that is what makes the levels meet.
    let locks: Vec<bool> = verts.iter().map(|&v| shared.get(&key(soup.verts[v as usize].pos)).copied() == Some(SHARED)).collect();
    let adapter = VertexDataAdapter::new(&positions, 12, 0).expect("positions");
    core::array::from_fn(|lod| {
        if soup.detail && lod > 0 {
            return Vec::new();
        }
        // A cut-out's shape is its picture: its few faces stay as they are, and are left out far away.
        let kept: Vec<u32> = if material.cutout {
            if lod == LODS - 1 {
                Vec::new()
            } else {
                idx.clone()
            }
        } else {
            let mut options = SimplifyOptions::ErrorAbsolute | SimplifyOptions::Permissive;
            if lod > 0 {
                options |= SimplifyOptions::Prune;
            }
            meshopt::simplify_with_locks(&idx, &adapter, &locks, 0, set.error[lod], options, None)
        };
        let mut out: Vec<Tri> = Vec::with_capacity(kept.len() / 3);
        for t in kept.chunks_exact(3) {
            let v = [&soup.verts[verts[t[0] as usize] as usize], &soup.verts[verts[t[1] as usize] as usize], &soup.verts[verts[t[2] as usize] as usize]];
            let mut tri: Tri = core::array::from_fn(|k| Corner { pos: v[k].pos, nrm: v[k].nrm, col: v[k].col, amb: v[k].amb, uv: v[k].uv });
            let (n, area) = face_normal(&tri);
            if area < 1e-7 || area < set.min_area[lod] {
                continue;
            }
            // A corner keeps its vertex's normal where the face is part of a rounded surface, and takes
            // the face's own where the vertex came from another face (a collapse across a hard edge).
            for c in &mut tri {
                if dot(c.nrm, n) < 0.8 {
                    c.nrm = n;
                }
            }
            out.push(tri);
        }
        out
    })
}

/// Cuts the world into cells and gives every cell's share of every material its levels of detail.
pub fn pieces(world: &World, grid: &Grid, set: &Settings) -> Vec<Piece> {
    let mut all = Vec::new();
    for soup in &world.soups {
        let material = &world.materials[soup.material];
        let count = soup.idx.len() / 3;
        let cell_of: Vec<u32> = (0..count)
            .map(|t| {
                let p = |k: usize| soup.verts[soup.idx[t * 3 + k] as usize].pos;
                let (a, b, c) = (p(0), p(1), p(2));
                grid.cell_of((a[0] + b[0] + c[0]) / 3.0, (a[2] + b[2] + c[2]) / 3.0) as u32
            })
            .collect();
        // Which points are used from more than one cell.
        let mut shared: HashMap<[i32; 3], u32> = HashMap::with_capacity(soup.verts.len() / 2);
        for t in 0..count {
            for k in 0..3 {
                let at = shared.entry(key(soup.verts[soup.idx[t * 3 + k] as usize].pos)).or_insert(cell_of[t]);
                if *at != cell_of[t] {
                    *at = SHARED;
                }
            }
        }
        let mut by_cell: HashMap<u32, Vec<u32>> = HashMap::new();
        for t in 0..count {
            by_cell.entry(cell_of[t]).or_default().push(t as u32);
        }
        let mut cells: Vec<(u32, Vec<u32>)> = by_cell.into_iter().collect();
        cells.sort_by_key(|c| c.0);
        let is_ground_family = set.ground_families.iter().any(|f| *f == material.family);
        let made: Vec<Vec<Piece>> = cells
            .par_iter()
            .map(|(cell, tris)| {
                let lods = simplify_cell(soup, material, tris, &shared, set);
                // The flat faces at ground level are a piece of their own.
                let mut ground: [Vec<Tri>; LODS] = Default::default();
                let mut rest: [Vec<Tri>; LODS] = Default::default();
                for (lod, tris) in lods.into_iter().enumerate() {
                    for t in tris {
                        let flat = is_ground_family && face_normal(&t).0[1] > 0.94 && t.iter().all(|c| c.pos[1] < set.ground_below);
                        if flat {
                            ground[lod].push(t);
                        } else {
                            rest[lod].push(t);
                        }
                    }
                }
                let mut out = Vec::new();
                for (is_ground, lods) in [(true, ground), (false, rest)] {
                    if lods.iter().any(|l| !l.is_empty()) {
                        out.push(Piece { cell: *cell as usize, material: soup.material, detail: soup.detail, ground: is_ground, lods });
                    }
                }
                out
            })
            .collect();
        all.extend(made.into_iter().flatten());
    }
    all
}
