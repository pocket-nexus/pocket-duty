//! The ground a match is played on, as the pack holds it: the collision
//! triangles and their tree, the grid the soldiers walk, the places they take
//! cover behind, and where anyone can be put.

use crate::bvh::{Bvh, Hit};
use crate::math::*;

/// The walk grid's header in the pack's `NAVG` section. After it: `nx × nz` flags (0 closed, 1 a
/// soldier stands, 2 a soldier crouches), padded to 4 bytes; `nx × nz` floor heights in
/// centimetres (`i16`), padded to 4 bytes; `cover` [`Cover`] records; `spawns` [`Spawn`] records.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct NavHeader {
    pub cell: f32,
    pub min_x: f32,
    pub min_z: f32,
    pub nx: u32,
    pub nz: u32,
    pub cover: u32,
    pub spawns: u32,
    pub pad: u32,
}

/// A place behind something: where to stand, which way the thing is, and whether it hides a
/// standing soldier.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct Cover {
    pub pos: [f32; 3],
    pub dir: [f32; 2],
    pub high: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct Spawn {
    pub pos: [f32; 3],
    pub yaw: f32,
}

#[derive(Clone, Copy)]
pub struct Level<'a> {
    pub solid: Bvh<'a>,
    pub nav: NavHeader,
    pub flags: &'a [u8],
    pub floor: &'a [i16],
    pub cover: &'a [Cover],
    pub spawns: &'a [Spawn],
}

impl<'a> Level<'a> {
    /// Reads the `NAVG` section in place; `bytes` is 4-byte aligned.
    ///
    /// # Safety
    /// `bytes` is a `NAVG` section the compiler wrote, and outlives the level.
    pub unsafe fn parse_nav(solid: Bvh<'a>, bytes: &'a [u8]) -> Option<Level<'a>> {
        if bytes.len() < core::mem::size_of::<NavHeader>() || bytes.as_ptr() as usize & 3 != 0 {
            return None;
        }
        let nav = (bytes.as_ptr() as *const NavHeader).read();
        let cells = (nav.nx * nav.nz) as usize;
        let mut at = core::mem::size_of::<NavHeader>();
        let flags = bytes.get(at..at + cells)?;
        at = (at + cells + 3) & !3;
        let floor = core::slice::from_raw_parts(
            bytes.get(at..at + cells * 2)?.as_ptr() as *const i16,
            cells,
        );
        at = (at + cells * 2 + 3) & !3;
        let cover = core::slice::from_raw_parts(
            bytes
                .get(at..at + nav.cover as usize * core::mem::size_of::<Cover>())?
                .as_ptr() as *const Cover,
            nav.cover as usize,
        );
        at += nav.cover as usize * core::mem::size_of::<Cover>();
        let spawns = core::slice::from_raw_parts(
            bytes
                .get(at..at + nav.spawns as usize * core::mem::size_of::<Spawn>())?
                .as_ptr() as *const Spawn,
            nav.spawns as usize,
        );
        Some(Level {
            solid,
            nav,
            flags,
            floor,
            cover,
            spawns,
        })
    }

    /// The grid node nearest a point, if it is on the grid.
    #[inline]
    pub fn node(&self, x: f32, z: f32) -> Option<u32> {
        let (i, j) = (
            floor((x - self.nav.min_x) / self.nav.cell + 0.5) as i32,
            floor((z - self.nav.min_z) / self.nav.cell + 0.5) as i32,
        );
        (i >= 0 && j >= 0 && i < self.nav.nx as i32 && j < self.nav.nz as i32)
            .then_some(j as u32 * self.nav.nx + i as u32)
    }

    #[inline]
    pub fn node_pos(&self, node: u32) -> V3 {
        let (i, j) = (node % self.nav.nx, node / self.nav.nx);
        v3(
            self.nav.min_x + i as f32 * self.nav.cell,
            self.floor[node as usize] as f32 * 0.01,
            self.nav.min_z + j as f32 * self.nav.cell,
        )
    }

    #[inline]
    pub fn open(&self, node: u32) -> bool {
        self.flags[node as usize] != 0
    }

    /// Whether a soldier can walk from one node to a neighbour: both open, and no step taller than a knee.
    #[inline]
    pub fn passable(&self, from: u32, to: u32) -> bool {
        self.open(to)
            && (self.floor[from as usize] as i32 - self.floor[to as usize] as i32).abs() <= 45
    }

    /// The open node nearest a point, looking outward ring by ring.
    pub fn nearest_open(&self, x: f32, z: f32, y: f32) -> Option<u32> {
        let (ci, cj) = (
            floor((x - self.nav.min_x) / self.nav.cell + 0.5) as i32,
            floor((z - self.nav.min_z) / self.nav.cell + 0.5) as i32,
        );
        for ring in 0..8i32 {
            let mut best: Option<(f32, u32)> = None;
            for dj in -ring..=ring {
                for di in -ring..=ring {
                    if di.abs().max(dj.abs()) != ring {
                        continue;
                    }
                    let (i, j) = (ci + di, cj + dj);
                    if i < 0 || j < 0 || i >= self.nav.nx as i32 || j >= self.nav.nz as i32 {
                        continue;
                    }
                    let node = j as u32 * self.nav.nx + i as u32;
                    if !self.open(node) || abs(self.floor[node as usize] as f32 * 0.01 - y) > 1.4 {
                        continue;
                    }
                    let p = self.node_pos(node);
                    let d = (p.x - x) * (p.x - x) + (p.z - z) * (p.z - z);
                    if best.is_none_or(|b| d < b.0) {
                        best = Some((d, node));
                    }
                }
            }
            if let Some((_, node)) = best {
                return Some(node);
            }
        }
        None
    }

    /// Whether a soldier can walk straight from one point to another: every node along the way is
    /// open and no step between two of them is too tall.
    pub fn walkable(&self, from: V3, to: V3) -> bool {
        let d = (to - from).flat();
        let length = d.len();
        let steps = (length / (self.nav.cell * 0.6)) as i32 + 1;
        let Some(mut last) = self.node(from.x, from.z) else {
            return false;
        };
        for s in 1..=steps {
            let p = from + d * (s as f32 / steps as f32);
            let Some(node) = self.node(p.x, p.z) else {
                return false;
            };
            if node != last {
                if !self.passable(last, node) {
                    return false;
                }
                last = node;
            }
        }
        true
    }

    /// What a ray meets in the solid world.
    #[inline]
    pub fn ray(&self, from: V3, dir: V3, far: f32) -> Option<Hit> {
        self.solid.raycast(from, dir, far, 0)
    }

    /// Whether nothing solid stands between two points.
    pub fn clear(&self, from: V3, to: V3) -> bool {
        let d = to - from;
        let length = d.len();
        length < 1e-3 || !self.solid.occluded(from, d * (1.0 / length), length, 0)
    }
}
