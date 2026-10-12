//! The collision world: the triangles the reference's physics system holds, each with its surface,
//! and the tree the device walks through them.

use std::collections::HashMap;
use std::path::Path;

use duty_sim::bvh::{self, Bvh, Node, Tri};

use crate::ir::Dir;

pub struct Collision {
    pub verts: Vec<[f32; 3]>,
    pub tris: Vec<Tri>,
    pub nodes: Vec<Node>,
    pub surfaces: Vec<String>,
}

impl Collision {
    pub fn open(ir: &Path) -> Result<Collision, String> {
        let dir = Dir::open(&ir.join("collision.dir"))?;
        let pos = dir.f32s("position");
        let surface = dir.u8s("surface");
        // The physics system keeps three corners a triangle; shared corners become one vertex here.
        let mut index: HashMap<[u32; 3], u32> = HashMap::new();
        let mut verts: Vec<[f32; 3]> = Vec::new();
        let mut tris = Vec::with_capacity(surface.len());
        for (t, s) in surface.iter().enumerate() {
            let v: [u32; 3] = core::array::from_fn(|k| {
                let p = [pos[t * 9 + k * 3], pos[t * 9 + k * 3 + 1], pos[t * 9 + k * 3 + 2]];
                *index.entry(p.map(f32::to_bits)).or_insert_with(|| {
                    verts.push(p);
                    verts.len() as u32 - 1
                })
            });
            if v[0] != v[1] && v[1] != v[2] && v[2] != v[0] {
                tris.push(Tri { v, tag: *s as u32 });
            }
        }
        let nodes = bvh::build(&verts, &mut tris);
        let surfaces = dir.meta["surfaces"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
        Ok(Collision { verts, tris, nodes, surfaces })
    }

    pub fn bvh(&self) -> Bvh<'_> {
        Bvh { nodes: &self.nodes, tris: &self.tris, verts: &self.verts }
    }
}
