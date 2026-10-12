//! From pieces to what the device draws: each level of detail lit, cut where the sun's shadow
//! crosses a face, and stored as indexed vertices on the pack's position lattice.
//!
//! A vertex carries the light. So a shadow's edge can only lie where there are vertices, and a
//! face is cut along its edges until the sun changes across no edge longer than the level allows.
//! An edge is cut for every triangle that holds it, so no cut leaves a gap.

use std::collections::{HashMap, HashSet};

use duty_pack::{chunk, Chunk, Cluster, Lod, Scene, WorldVertex};
use meshopt::VertexDataAdapter;
use duty_sim::math::{v3, V3};
use rayon::prelude::*;

use crate::light::{Lighting, Tone};
use crate::lod::{dot, face_normal, len, sub, Corner, Piece, Tri, LODS};
use crate::world::{Material, Projection, World};

pub struct Built {
    pub chunks: Vec<Chunk>,
    pub vertices: Vec<WorldVertex>,
    pub indices: Vec<u16>,
    pub clusters: Vec<Cluster>,
    /// Triangles at each level of detail, and how many of them are large.
    pub triangles: [usize; LODS],
    pub large: [usize; LODS],
    pub lit_points: usize,
}

pub struct Settings {
    /// What is left of the sky's fill where no sky is seen at all, 0…1.
    pub open_floor: f32,
    /// The picture each material samples.
    pub texture_of: Vec<u16>,
    pub texture_flags: Vec<u16>,
    /// The longest edge a face that is wide as well as long keeps at each level, metres; 0 keeps any.
    pub largest: [f32; LODS],
    /// The shortest edge the sun's shadow still cuts at each level, metres; 0 cuts none.
    pub shadow_edge: [f32; LODS],
    /// A triangle with an edge longer than this is one the device may have to cut, metres.
    pub large_edge: f32,
    /// Triangles a cluster holds at most, and how much a cluster's faces are kept facing alike
    /// (0 groups by place alone).
    pub cluster_triangles: usize,
    pub cone_weight: f32,
    /// The level's own horizontal axes in the world: most walls face along one of them.
    pub level_axes: [[f32; 3]; 2],
}

/// A point of the position lattice and a direction: what a corner's light depends on.
type LightKey = ([i32; 3], [i8; 3]);

fn light_key(at: [f32; 3], c: &Corner, quantum: f32) -> LightKey {
    (at.map(|p| (p / quantum).round() as i32), c.nrm.map(|n| (n * 24.0).round() as i8))
}

/// Where a corner's light is taken: a few centimetres into its own triangle. At the corner itself
/// two surfaces meet, and a point there is as often inside the other one as in front of it.
fn sample_point(c: &Corner, tri: &Tri) -> [f32; 3] {
    let centre: [f32; 3] = core::array::from_fn(|k| (tri[0].pos[k] + tri[1].pos[k] + tri[2].pos[k]) / 3.0);
    let d = sub(centre, c.pos);
    let t = (0.05 / len(d).max(1e-6)).min(0.12);
    core::array::from_fn(|k| c.pos[k] + d[k] * t)
}

type EdgeKey = ([i32; 3], [i32; 3]);

fn edge_key(a: &Corner, b: &Corner) -> EdgeKey {
    let k = |c: &Corner| c.pos.map(|p| (p * 4096.0).round() as i32);
    let (x, y) = (k(a), k(b));
    if x <= y {
        (x, y)
    } else {
        (y, x)
    }
}

fn midpoint(a: &Corner, b: &Corner) -> Corner {
    let n = [a.nrm[0] + b.nrm[0], a.nrm[1] + b.nrm[1], a.nrm[2] + b.nrm[2]];
    let l = len(n).max(1e-9);
    Corner {
        pos: [(a.pos[0] + b.pos[0]) * 0.5, (a.pos[1] + b.pos[1]) * 0.5, (a.pos[2] + b.pos[2]) * 0.5],
        nrm: [n[0] / l, n[1] / l, n[2] / l],
        col: [(a.col[0] + b.col[0]) * 0.5, (a.col[1] + b.col[1]) * 0.5, (a.col[2] + b.col[2]) * 0.5],
        amb: [(a.amb[0] + b.amb[0]) * 0.5, (a.amb[1] + b.amb[1]) * 0.5, (a.amb[2] + b.amb[2]) * 0.5],
        uv: [(a.uv[0] + b.uv[0]) * 0.5, (a.uv[1] + b.uv[1]) * 0.5],
    }
}

/// How much of the sun each corner sees, kept by lattice point and direction.
struct Sun<'a> {
    lighting: &'a Lighting,
    quantum: f32,
    seen: HashMap<LightKey, f32>,
}

impl Sun<'_> {
    /// How much of the sun the corner sees from `at`.
    fn at(&mut self, c: &Corner, at: [f32; 3]) -> f32 {
        let (lighting, sun) = (self.lighting, self.lighting.sky.sun);
        *self.seen.entry(light_key(at, c, self.quantum)).or_insert_with(|| {
            let n = v3(c.nrm[0], c.nrm[1], c.nrm[2]);
            if n.dot(sun) <= 0.02 {
                0.0
            } else {
                lighting.sun_seen(v3(at[0], at[1], at[2]) + n * 0.03, 0.012, 4)
            }
        })
    }
}

/// Cuts the triangles' edges until no wide face is longer than `largest` and the sun changes
/// across no edge longer than `shadow_edge`.
fn refine(mut tris: Vec<Tri>, sun: &mut Sun, largest: f32, shadow_edge: f32) -> Vec<Tri> {
    let facing = |c: &Corner, l: V3| c.nrm[0] * l.x + c.nrm[1] * l.y + c.nrm[2] * l.z > 0.02;
    let light = sun.lighting.sky.sun;
    for _ in 0..7 {
        let mut cut: HashSet<EdgeKey> = HashSet::new();
        for t in &tris {
            let lengths: [f32; 3] = core::array::from_fn(|k| len(sub(t[k].pos, t[(k + 1) % 3].pos)));
            let longest = (0..3).max_by(|a, b| lengths[*a].partial_cmp(&lengths[*b]).unwrap()).unwrap();
            let area = face_normal(t).1;
            if largest > 0.0 && lengths[longest] > largest && 2.0 * area / lengths[longest] > 0.5 {
                cut.insert(edge_key(&t[longest], &t[(longest + 1) % 3]));
            }
            if shadow_edge <= 0.0 {
                continue;
            }
            for k in 0..3 {
                let (a, b) = (&t[k], &t[(k + 1) % 3]);
                if lengths[k] <= shadow_edge || !(facing(a, light) || facing(b, light)) {
                    continue;
                }
                let (sa, sb) = (sun.at(a, sample_point(a, t)), sun.at(b, sample_point(b, t)));
                let m = midpoint(a, b);
                // The shadow's edge crosses it, or a shadow narrower than the edge lies on it.
                if (sa - sb).abs() > 0.3 || (sun.at(&m, sample_point(&m, t)) - (sa + sb) * 0.5).abs() > 0.3 {
                    cut.insert(edge_key(a, b));
                }
            }
        }
        if cut.is_empty() {
            break;
        }
        let mut next = Vec::with_capacity(tris.len() + cut.len() * 2);
        for t in tris {
            let split: [bool; 3] = core::array::from_fn(|k| cut.contains(&edge_key(&t[k], &t[(k + 1) % 3])));
            let m: [Corner; 3] = core::array::from_fn(|k| midpoint(&t[k], &t[(k + 1) % 3]));
            match split.iter().filter(|s| **s).count() {
                0 => next.push(t),
                1 => {
                    let k = split.iter().position(|s| *s).unwrap();
                    let (a, b, c) = (t[k], t[(k + 1) % 3], t[(k + 2) % 3]);
                    next.push([a, m[k], c]);
                    next.push([m[k], b, c]);
                }
                2 => {
                    // The corner between the two cut edges, and the two others.
                    let k = (0..3).find(|k| !split[*k]).unwrap();
                    let (a, b, c) = (t[k], t[(k + 1) % 3], t[(k + 2) % 3]);
                    let (mbc, mca) = (m[(k + 1) % 3], m[(k + 2) % 3]);
                    next.push([c, mca, mbc]);
                    next.push([a, b, mbc]);
                    next.push([a, mbc, mca]);
                }
                _ => {
                    next.push([t[0], m[0], m[2]]);
                    next.push([m[0], t[1], m[1]]);
                    next.push([m[2], m[1], t[2]]);
                    next.push([m[0], m[1], m[2]]);
                }
            }
        }
        tris = next;
    }
    tris
}

/// A corner's texture coordinates, as the reference's surface program projects them: along the
/// dominant axis of the face's normal, a tile every `scale` metres. The pictures are stored top row
/// first, so the vertical coordinate runs the other way.
fn project(c: &Corner, n: [f32; 3], m: &Material) -> [f32; 2] {
    if m.projection == Projection::Mesh {
        return [c.uv[0] * m.scale + m.offset[0], -(c.uv[1] * m.scale + m.offset[1])];
    }
    let (ax, ay, az) = (n[0].abs(), n[1].abs(), n[2].abs());
    let sign = |v: f32| if v >= 0.0 { 1.0 } else { -1.0 };
    let p = c.pos;
    let uv = if ax > ay {
        if ax > az {
            [-p[2] * sign(n[0]), p[1]]
        } else {
            [p[0] * sign(n[2]), p[1]]
        }
    } else if ay > az {
        [p[0], -p[2] * sign(n[1])]
    } else {
        [p[0] * sign(n[2]), p[1]]
    };
    [uv[0] / m.scale + m.offset[0], -(uv[1] / m.scale + m.offset[1])]
}

fn rgb565(c: [f32; 3]) -> u16 {
    let q = |v: f32, max: f32| (v.clamp(0.0, 1.0) * max + 0.5) as u16;
    q(c[0], 31.0) | q(c[1], 63.0) << 5 | q(c[2], 31.0) << 11
}

/// One level of one piece, ready to be laid into the pack.
struct Level {
    vertices: Vec<WorldVertex>,
    /// Cluster after cluster, each with its large triangles first.
    indices: Vec<u16>,
    /// The clusters, their runs counted from the level's first index.
    clusters: Vec<Cluster>,
    large: usize,
    bounds: ([f32; 3], [f32; 3]),
}

struct Lit {
    levels: [Level; LODS],
    uv_offset: [f32; 2],
    uv_scale: [f32; 2],
    points: usize,
}

fn light_piece(piece: &Piece, material: &Material, scene: &Scene, lighting: &Lighting, tone: &Tone, set: &Settings) -> Lit {
    let centre = scene.cell_centre(piece.cell as u32);
    let mut sun = Sun { lighting, quantum: scene.quantum, seen: HashMap::new() };
    let refined: Vec<Vec<Tri>> = piece
        .lods
        .iter()
        .enumerate()
        .map(|(l, tris)| if material.cutout { tris.clone() } else { refine(tris.clone(), &mut sun, set.largest[l], if piece.ground { 0.0 } else { set.shadow_edge[l] }) })
        .collect();
    // The frame the levels' texture coordinates share.
    let coords: Vec<Vec<[[f32; 2]; 3]>> = refined
        .iter()
        .map(|lod| {
            lod.iter()
                .map(|tri| {
                    let n = face_normal(tri).0;
                    [project(&tri[0], n, material), project(&tri[1], n, material), project(&tri[2], n, material)]
                })
                .collect()
        })
        .collect();
    let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
    for uv in coords.iter().flatten().flatten() {
        for k in 0..2 {
            lo[k] = lo[k].min(uv[k]);
            hi[k] = hi[k].max(uv[k]);
        }
    }
    let uv_offset = [lo[0].floor(), lo[1].floor()];
    let uv_scale = [(hi[0] - uv_offset[0]).max(1e-3) * (32768.0 / 32767.0), (hi[1] - uv_offset[1]).max(1e-3) * (32768.0 / 32767.0)];
    // How open each corner is to the sky.
    let mut open: HashMap<LightKey, f32> = HashMap::new();
    let sun_dir = lighting.sky.sun;
    let levels: [Level; LODS] = core::array::from_fn(|l| {
        let mut level = Level { vertices: Vec::new(), indices: Vec::new(), clusters: Vec::new(), large: 0, bounds: ([f32::MAX; 3], [f32::MIN; 3]) };
        let mut welded: HashMap<WorldVertex, u16> = HashMap::new();
        let mut triangles: Vec<[u16; 3]> = Vec::new();
        for (t, tri) in refined[l].iter().enumerate() {
            let mut corners = [0u16; 3];
            for (k, c) in tri.iter().enumerate() {
                let at = sample_point(c, tri);
                let key = light_key(at, c, scene.quantum);
                let n = v3(c.nrm[0], c.nrm[1], c.nrm[2]);
                let sky = *open.entry(key).or_insert_with(|| lighting.open(v3(at[0], at[1], at[2]), n));
                let fill = set.open_floor + (1.0 - set.open_floor) * sky;
                let facing = n.dot(sun_dir).max(0.0);
                let seen = if piece.ground { 1.0 } else { sun.at(c, at) };
                let albedo = c.col.map(|v| v.powf(2.2));
                // A surface the reference's program could not be asked takes the fill open ground has.
                let ambient = if c.amb[0] < 0.0 { [albedo[0] * 0.13, albedo[1] * 0.21, albedo[2] * 0.41] } else { c.amb };
                let linear: [f32; 3] = core::array::from_fn(|i| ambient[i] * fill + albedo[i] * lighting.sky.sun_colour[i] * facing * seen * core::f32::consts::FRAC_1_PI);
                let uv = coords[l][t][k];
                let v = WorldVertex {
                    uv: [(((uv[0] - uv_offset[0]) / uv_scale[0]) * 32768.0).round().clamp(0.0, 32767.0) as i16, (((uv[1] - uv_offset[1]) / uv_scale[1]) * 32768.0).round().clamp(0.0, 32767.0) as i16],
                    colour: rgb565(tone.display(linear)),
                    pos: core::array::from_fn(|i| ((c.pos[i] - centre[i]) / scene.quantum).round().clamp(-32768.0, 32767.0) as i16),
                };
                for i in 0..3 {
                    level.bounds.0[i] = level.bounds.0[i].min(c.pos[i]);
                    level.bounds.1[i] = level.bounds.1[i].max(c.pos[i]);
                }
                let next = welded.len();
                corners[k] = *welded.entry(v).or_insert_with(|| {
                    assert!(next < 65535, "a chunk of cell {} holds more than 65 535 vertices at level {l}: make the cells smaller", piece.cell);
                    level.vertices.push(v);
                    next as u16
                });
            }
            // Two corners on one lattice point: the triangle has no area there.
            if corners[0] == corners[1] || corners[1] == corners[2] || corners[2] == corners[0] {
                continue;
            }
            triangles.push(corners);
        }
        if triangles.is_empty() {
            return level;
        }
        // Clusters: runs of triangles that lie together and face alike.
        let place = |v: u16| -> [f32; 3] { core::array::from_fn(|i| centre[i] + level.vertices[v as usize].pos[i] as f32 * scene.quantum) };
        let positions: Vec<u8> = (0..level.vertices.len()).flat_map(|v| place(v as u16).into_iter().flat_map(|c| c.to_le_bytes()).collect::<Vec<u8>>()).collect();
        let adapter = VertexDataAdapter::new(&positions, 12, 0).expect("positions");
        // Triangles that face the same way go together: a cluster whose faces all look one way is
        // left out whole when the eye is behind them. The six ways are the level's own: along its
        // street, across it, up and down.
        let two_sided_material = material.double_sided || material.cutout || material.transparent;
        let mut facing: [Vec<u32>; 6] = Default::default();
        for t in &triangles {
            let p = [place(t[0]), place(t[1]), place(t[2])];
            let n = crate::lod::cross(sub(p[1], p[0]), sub(p[2], p[0]));
            let along = [dot(n, set.level_axes[0]), n[1], dot(n, set.level_axes[1])];
            let axis = if two_sided_material { 0 } else { (0..3).max_by(|a, b| along[*a].abs().partial_cmp(&along[*b].abs()).unwrap()).unwrap() * 2 + (along.iter().fold((0.0f32, 0.0f32), |best, v| if v.abs() > best.0 { (v.abs(), *v) } else { best }).1 < 0.0) as usize };
            facing[axis].extend(t.map(|v| v as u32));
        }
        let two_sided = two_sided_material;
        for group in facing.iter().filter(|g| !g.is_empty()) {
          let meshlets = meshopt::build_meshlets(group, &adapter, 96, set.cluster_triangles, set.cone_weight);
          for m in meshlets.iter() {
            let mut run: Vec<(bool, [u16; 3])> = m
                .triangles
                .chunks_exact(3)
                .map(|t| {
                    let c = [m.vertices[t[0] as usize] as u16, m.vertices[t[1] as usize] as u16, m.vertices[t[2] as usize] as u16];
                    let p = [place(c[0]), place(c[1]), place(c[2])];
                    let longest = (0..3).map(|k| len(sub(p[k], p[(k + 1) % 3]))).fold(0.0f32, f32::max);
                    (longest > set.large_edge, c)
                })
                .collect();
            if run.is_empty() {
                continue;
            }
            run.sort_by_key(|t| !t.0);
            let large = run.iter().filter(|t| t.0).count();
            let indices: Vec<u32> = run.iter().flat_map(|t| t.1.map(|v| v as u32)).collect();
            let bounds = meshopt::compute_cluster_bounds(&indices, &adapter);
            level.clusters.push(Cluster {
                centre: bounds.center,
                radius: bounds.radius,
                axis: if two_sided { [0; 3] } else { bounds.cone_axis_s8 },
                cutoff: if two_sided { 127 } else { bounds.cone_cutoff_s8 },
                first: level.indices.len() as u32,
                count: indices.len() as u16,
                large: large as u16,
            });
            level.large += large;
            level.indices.extend(run.iter().flat_map(|t| t.1));
          }
        }
        level
    });
    Lit { levels, uv_offset, uv_scale, points: open.len() }
}

pub fn build(pieces: &[Piece], world: &World, scene: &Scene, lighting: &Lighting, tone: &Tone, set: &Settings) -> Built {
    let lit: Vec<Lit> = pieces.par_iter().map(|piece| light_piece(piece, &world.materials[piece.material], scene, lighting, tone, set)).collect();
    // In the order the device binds its pictures.
    let mut order: Vec<usize> = (0..pieces.len()).collect();
    order.sort_by_key(|&i| {
        let p = &pieces[i];
        (world.materials[p.material].transparent, set.texture_of[p.material], p.ground, p.cell, p.material, p.detail)
    });
    let mut built = Built { chunks: Vec::new(), vertices: Vec::new(), indices: Vec::new(), clusters: Vec::new(), triangles: [0; LODS], large: [0; LODS], lit_points: lit.iter().map(|l| l.points).sum() };
    for i in order {
        let (piece, lit) = (&pieces[i], &lit[i]);
        let material = &world.materials[piece.material];
        let mut flags = set.texture_flags[piece.material];
        for (on, bit) in [(piece.ground, chunk::GROUND), (piece.detail, chunk::DETAIL), (material.double_sided, chunk::TWO_SIDED), (material.transparent, chunk::BLEND)] {
            if on {
                flags |= bit;
            }
        }
        let mut record = Chunk {
            min: [f32::MAX; 3],
            max: [f32::MIN; 3],
            texture: set.texture_of[piece.material],
            flags,
            cell: piece.cell as u16,
            opacity: (material.opacity.clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
            pad: 0,
            uv_scale: lit.uv_scale,
            uv_offset: lit.uv_offset,
            lod: [Lod::default(); LODS],
        };
        for (l, level) in lit.levels.iter().enumerate() {
            if level.indices.is_empty() {
                continue;
            }
            for a in 0..3 {
                record.min[a] = record.min[a].min(level.bounds.0[a]);
                record.max[a] = record.max[a].max(level.bounds.1[a]);
            }
            record.lod[l] = Lod { vertex: built.vertices.len() as u32, index: built.indices.len() as u32, indices: level.indices.len() as u32, cluster: built.clusters.len() as u32, clusters: level.clusters.len() as u32 };
            built.vertices.extend_from_slice(&level.vertices);
            built.indices.extend_from_slice(&level.indices);
            built.clusters.extend_from_slice(&level.clusters);
            built.triangles[l] += level.indices.len() / 3;
            built.large[l] += level.large;
        }
        if record.lod.iter().any(|l| l.indices > 0) {
            built.chunks.push(record);
        }
    }
    built
}
