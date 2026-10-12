//! The world as the reference drew it: every mesh and every instance in world space, gathered by
//! material.

use std::collections::HashMap;
use std::path::Path;

use crate::ir::Dir;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Projection {
    /// Along the dominant axis of the face's normal, in world space.
    Planar,
    /// The reference mixes three projections; a handheld takes the dominant one.
    Triplanar,
    /// The mesh's own coordinates.
    Mesh,
}

#[derive(Clone, Debug)]
pub struct Material {
    pub name: String,
    /// The forged tile it samples: `plaster`, `wood`, …
    pub family: String,
    pub projection: Projection,
    /// Metres a tile covers (`Mesh`: how many times the coordinates repeat it).
    pub scale: f32,
    pub offset: [f32; 2],
    /// The tile's mean colour in linear light.
    pub mean: [f32; 3],
    pub double_sided: bool,
    pub transparent: bool,
    pub opacity: f32,
    pub cutout: bool,
    /// Light it gives off, linear.
    pub emissive: [f32; 3],
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub nrm: [f32; 3],
    /// What the reference's surface program gives the vertex before light, at gamma 2.2.
    pub col: [f32; 3],
    /// The light the reference's surface program sends back from the vertex with the sun switched
    /// off, linear: the sky's fill, the ground's, the environment, and what the surface gives off.
    pub amb: [f32; 3],
    pub uv: [f32; 2],
}

/// Triangles of one material. `detail`: small things the reference stops drawing at a distance.
pub struct Soup {
    pub material: usize,
    pub detail: bool,
    pub verts: Vec<Vertex>,
    pub idx: Vec<u32>,
}

pub struct World {
    pub materials: Vec<Material>,
    pub soups: Vec<Soup>,
    /// Triangles the export held, before anything was left out.
    pub source_triangles: usize,
    pub dropped: Vec<(String, usize)>,
}

fn apply(m: &[f32], p: [f32; 3]) -> [f32; 3] {
    [m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12], m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13], m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14]]
}

/// A normal through a matrix: by the cofactors of its 3 × 3, so a stretched instance keeps its faces' directions.
fn apply_normal(m: &[f32], n: [f32; 3]) -> [f32; 3] {
    let c = |a: usize, b: usize, c: usize, d: usize| m[a] * m[b] - m[c] * m[d];
    let co = [
        [c(5, 10, 9, 6), c(9, 2, 1, 10), c(1, 6, 5, 2)],
        [c(8, 6, 4, 10), c(0, 10, 8, 2), c(4, 2, 0, 6)],
        [c(4, 9, 8, 5), c(8, 1, 0, 9), c(0, 5, 4, 1)],
    ];
    // (the cofactor matrix is the inverse transpose times the determinant)
    let v: [f32; 3] = core::array::from_fn(|i| co[i][0] * n[0] + co[i][1] * n[1] + co[i][2] * n[2]);
    let det = m[0] * co[0][0] + m[4] * co[0][1] + m[8] * co[0][2];
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() * det.signum();
    if l.abs() > 1e-12 {
        [v[0] / l, v[1] / l, v[2] / l]
    } else {
        n
    }
}

/// Reads `world.dir`. A mesh whose name is in `drop` is left out.
pub fn load(ir: &Path, drop: &[String]) -> Result<World, String> {
    let dir = Dir::open(&ir.join("world.dir"))?;
    let materials: Vec<Material> = dir.meta["materials"]
        .as_array()
        .ok_or("world.dir lists no materials")?
        .iter()
        .map(|m| {
            let three = |key: &str| -> [f32; 3] { core::array::from_fn(|i| m[key][i].as_f64().unwrap_or(0.0) as f32) };
            Material {
                name: m["name"].as_str().unwrap_or("").to_string(),
                family: m["family"].as_str().unwrap_or("plain").to_string(),
                projection: match m["projection"].as_str() {
                    Some("triplanar") => Projection::Triplanar,
                    Some("mesh") => Projection::Mesh,
                    _ => Projection::Planar,
                },
                scale: m["scale"].as_f64().unwrap_or(1.0) as f32,
                offset: [m["offset"][0].as_f64().unwrap_or(0.0) as f32, m["offset"][1].as_f64().unwrap_or(0.0) as f32],
                mean: three("mean"),
                double_sided: m["doubleSided"].as_bool().unwrap_or(false),
                transparent: m["transparent"].as_bool().unwrap_or(false),
                opacity: m["opacity"].as_f64().unwrap_or(1.0) as f32,
                cutout: m["cutout"].as_bool().unwrap_or(false),
                emissive: three("emissive"),
            }
        })
        .collect();
    let mut soups: HashMap<(usize, bool), Soup> = HashMap::new();
    let mut source_triangles = 0;
    let mut dropped: Vec<(String, usize)> = Vec::new();
    for m in dir.meta["meshes"].as_array().ok_or("world.dir lists no meshes")? {
        let name = m["name"].as_str().unwrap_or("");
        let pos = dir.f32s(m["position"].as_str().ok_or("a mesh without positions")?);
        let nrm = dir.i8s(m["normal"].as_str().ok_or("a mesh without normals")?);
        let col = dir.u8s(m["colour"].as_str().ok_or("a mesh without colours")?);
        let amb = dir.f32s(m["ambient"].as_str().ok_or("a mesh without its sunless light: export again")?);
        let idx = dir.u32s(m["index"].as_str().ok_or("a mesh without indices")?);
        let uv = m["uv"].as_str().map(|n| dir.f32s(n));
        let count = pos.len() / 3;
        let instances = m["instances"].as_u64().unwrap_or(0) as usize;
        let copies = instances.max(1);
        source_triangles += idx.len() / 3 * copies;
        if drop.iter().any(|d| d == name) {
            match dropped.iter_mut().find(|d| d.0 == name) {
                Some(d) => d.1 += idx.len() / 3 * copies,
                None => dropped.push((name.to_string(), idx.len() / 3 * copies)),
            }
            continue;
        }
        let material = m["material"].as_u64().unwrap_or(0) as usize;
        let has_normals = m["hasNormals"].as_bool().unwrap_or(true);
        let soup = soups.entry((material, !m["reach"].is_null())).or_insert_with(|| Soup { material, detail: !m["reach"].is_null(), verts: Vec::new(), idx: Vec::new() });
        let matrices = m["matrices"].as_str().map(|n| dir.f32s(n));
        for k in 0..copies {
            let base = soup.verts.len() as u32;
            let matrix = matrices.as_ref().map(|all| &all[k * 16..k * 16 + 16]);
            for v in 0..count {
                let p = [pos[v * 3], pos[v * 3 + 1], pos[v * 3 + 2]];
                let n = [nrm[v * 3] as f32 / 127.0, nrm[v * 3 + 1] as f32 / 127.0, nrm[v * 3 + 2] as f32 / 127.0];
                let c = (k * count + v) * 3;
                soup.verts.push(Vertex {
                    pos: matrix.map_or(p, |m| apply(m, p)),
                    nrm: if !has_normals { [0.0; 3] } else { matrix.map_or(n, |m| apply_normal(m, n)) },
                    col: [col[c] as f32 / 255.0, col[c + 1] as f32 / 255.0, col[c + 2] as f32 / 255.0],
                    amb: [amb[c], amb[c + 1], amb[c + 2]],
                    uv: uv.as_ref().map_or([0.0; 2], |uv| [uv[v * 2], uv[v * 2 + 1]]),
                });
            }
            // A mirrored instance turns its triangles over: wind them back.
            let flipped = matrix.is_some_and(|m| m[0] * (m[5] * m[10] - m[9] * m[6]) - m[4] * (m[1] * m[10] - m[9] * m[2]) + m[8] * (m[1] * m[6] - m[5] * m[2]) < 0.0);
            for t in idx.chunks_exact(3) {
                if flipped {
                    soup.idx.extend_from_slice(&[base + t[0], base + t[2], base + t[1]]);
                } else {
                    soup.idx.extend_from_slice(&[base + t[0], base + t[1], base + t[2]]);
                }
            }
        }
    }
    let mut soups: Vec<Soup> = soups.into_values().collect();
    soups.sort_by_key(|s| (s.material, s.detail));
    Ok(World { materials, soups, source_triangles, dropped })
}
