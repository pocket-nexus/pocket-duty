//! The ground a match is played on, as the pack holds it: the collision
//! triangles and their tree, the grid the soldiers walk, the places they take
//! cover behind, and where anyone can be put.

use crate::bvh::{Bvh, Hit};
use crate::math::*;

/// The walk grid's header in the pack's `NAVG` section. A place has a node every `cell` metres,
/// `nx × nz` of them, and `over` more where a floor stands over a floor: a node is one height at
/// one cell, the lowest open one of a cell first and the rest after the grid's own, in the
/// order of their cells. After the header:
///
/// - `nx × nz + over` flags, padded to 4 bytes: 0 closed, 1 a soldier stands, 2 a soldier
///   crouches; beside it [`STAIR`], [`OVER`] and [`LADDER`];
/// - `nx × nz + over` floor heights in centimetres (`i16`), padded to 4 bytes;
/// - `cover` [`Cover`] records and `spawns` [`Spawn`] records;
/// - `over` cells (`u16`), the cell each further node stands at, ascending, padded to 4 bytes;
/// - `links` [`Link`] records, the ladders.
///
/// A place of one level has no further node and no ladder, and its section ends after the starts.
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
    pub over: u16,
    pub links: u16,
}

/// A node's flag beside 1 or 2: a stair's node, where a step may be twice a knee.
pub const STAIR: u8 = 4;
/// A grid node's flag: further nodes stand at its cell, over it.
pub const OVER: u8 = 8;
/// A node's flag: a ladder a soldier climbs begins or ends here.
pub const LADDER: u8 = 16;
/// No node: a ladder's end the walk grid does not reach.
pub const NONE: u32 = u32::MAX;

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

/// A ladder: where its foot stands, where a climber steps off at its top (0.6 m in from the
/// ladder's own line, which rises from the foot), and the nodes at the two for a soldier, or
/// [`NONE`] twice where the walk grid reaches one of them only: a player climbs that one alone.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct Link {
    pub foot: [f32; 3],
    pub top: [f32; 3],
    pub nodes: [u32; 2],
}

impl Link {
    #[inline]
    pub fn foot(&self) -> V3 {
        v3(self.foot[0], self.foot[1], self.foot[2])
    }

    #[inline]
    pub fn top(&self) -> V3 {
        v3(self.top[0], self.top[1], self.top[2])
    }
}

#[derive(Clone, Copy)]
pub struct Level<'a> {
    pub solid: Bvh<'a>,
    pub nav: NavHeader,
    /// A flag and a floor a node: the grid's `nx × nz`, then the further nodes.
    pub flags: &'a [u8],
    pub floor: &'a [i16],
    pub cover: &'a [Cover],
    pub spawns: &'a [Spawn],
    /// The cell of each further node, ascending.
    pub over: &'a [u16],
    pub links: &'a [Link],
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
        let nodes = (nav.nx * nav.nz) as usize + nav.over as usize;
        let mut at = core::mem::size_of::<NavHeader>();
        let flags = bytes.get(at..at + nodes)?;
        at = (at + nodes + 3) & !3;
        let floor = core::slice::from_raw_parts(
            bytes.get(at..at + nodes * 2)?.as_ptr() as *const i16,
            nodes,
        );
        at = (at + nodes * 2 + 3) & !3;
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
        at += nav.spawns as usize * core::mem::size_of::<Spawn>();
        let over = core::slice::from_raw_parts(
            bytes.get(at..at + nav.over as usize * 2)?.as_ptr() as *const u16,
            nav.over as usize,
        );
        at = (at + nav.over as usize * 2 + 3) & !3;
        let links = core::slice::from_raw_parts(
            bytes
                .get(at..at + nav.links as usize * core::mem::size_of::<Link>())?
                .as_ptr() as *const Link,
            nav.links as usize,
        );
        Some(Level {
            solid,
            nav,
            flags,
            floor,
            cover,
            spawns,
            over,
            links,
        })
    }

    /// The grid's cell nearest a point, if it is on the grid: the first node there.
    #[inline]
    pub fn node(&self, x: f32, z: f32) -> Option<u32> {
        let (i, j) = (
            floor((x - self.nav.min_x) / self.nav.cell + 0.5) as i32,
            floor((z - self.nav.min_z) / self.nav.cell + 0.5) as i32,
        );
        (i >= 0 && j >= 0 && i < self.nav.nx as i32 && j < self.nav.nz as i32)
            .then_some(j as u32 * self.nav.nx + i as u32)
    }

    /// The node at a point's cell whose floor is nearest a height: where floors stand over one
    /// another, the one someone at that height is on.
    #[inline]
    pub fn node_at(&self, x: f32, z: f32, y: f32) -> Option<u32> {
        let cell = self.node(x, z)?;
        let mut best = cell;
        for node in self.over(cell) {
            if abs(self.floor[node as usize] as f32 * 0.01 - y)
                < abs(self.floor[best as usize] as f32 * 0.01 - y)
            {
                best = node;
            }
        }
        Some(best)
    }

    /// The cell a node stands at.
    #[inline]
    pub fn cell_of(&self, node: u32) -> u32 {
        let cells = self.nav.nx * self.nav.nz;
        if node < cells {
            node
        } else {
            self.over[(node - cells) as usize] as u32
        }
    }

    /// The further nodes at a cell, over its first: none on a place of one level.
    #[inline]
    pub fn over(&self, cell: u32) -> core::ops::Range<u32> {
        if self.flags[cell as usize] & OVER == 0 {
            0..0
        } else {
            self.over_of(cell)
        }
    }

    #[inline(never)]
    fn over_of(&self, cell: u32) -> core::ops::Range<u32> {
        let first = self.over.partition_point(|c| (*c as u32) < cell);
        let mut end = first;
        while end < self.over.len() && self.over[end] as u32 == cell {
            end += 1;
        }
        let cells = self.nav.nx * self.nav.nz;
        cells + first as u32..cells + end as u32
    }

    #[inline]
    pub fn node_pos(&self, node: u32) -> V3 {
        let cell = self.cell_of(node);
        let (i, j) = (cell % self.nav.nx, cell / self.nav.nx);
        v3(
            self.nav.min_x + i as f32 * self.nav.cell,
            self.floor[node as usize] as f32 * 0.01,
            self.nav.min_z + j as f32 * self.nav.cell,
        )
    }

    #[inline]
    pub fn open(&self, node: u32) -> bool {
        self.flags[node as usize] & 3 != 0
    }

    /// Whether a soldier can walk from one node to one at a neighbouring cell: both open, and no step
    /// taller than a knee, or than two on a stair.
    #[inline(always)]
    pub fn passable(&self, from: u32, to: u32) -> bool {
        // (a stair's nodes carry 4 beside their flag: a step to or from one may be twice a knee)
        let stair = (self.flags[from as usize] | self.flags[to as usize]) & STAIR != 0;
        self.open(to) && (self.floor[from as usize] as i32 - self.floor[to as usize] as i32).abs() <= if stair { 90 } else { 45 }
    }

    /// The node at a cell a soldier steps onto from a node beside it: of the cell's nodes the one
    /// within a step of his.
    #[inline(always)]
    pub fn toward(&self, from: u32, cell: u32) -> Option<u32> {
        if self.passable(from, cell) {
            Some(cell)
        } else if self.flags[cell as usize] & OVER == 0 {
            None
        } else {
            self.over_of(cell).find(|node| self.passable(from, *node))
        }
    }

    /// The other end of the ladder a soldier climbs from a node, and the ladder.
    #[inline]
    pub fn ladder(&self, node: u32) -> Option<(u32, &'a Link)> {
        if self.flags[node as usize] & LADDER == 0 {
            return None;
        }
        self.links.iter().find_map(|link| {
            if link.nodes[0] == node {
                Some((link.nodes[1], link))
            } else if link.nodes[1] == node {
                Some((link.nodes[0], link))
            } else {
                None
            }
        })
    }

    /// The open node nearest a point and within 1.4 m of its height, looking outward ring by ring.
    pub fn nearest_open(&self, x: f32, z: f32, y: f32) -> Option<u32> {
        self.nearest(x, z, y, 1.4, 8)
    }

    /// The open node nearest a point over the ground, whatever its height: where a soldier goes
    /// for someone who stands where the grid does not reach, on a roof the walk grid has no way to.
    pub fn nearest_below(&self, x: f32, z: f32) -> Option<u32> {
        self.nearest(x, z, 0.0, f32::MAX, 20)
    }

    fn nearest(&self, x: f32, z: f32, y: f32, within: f32, rings: i32) -> Option<u32> {
        let (ci, cj) = (
            floor((x - self.nav.min_x) / self.nav.cell + 0.5) as i32,
            floor((z - self.nav.min_z) / self.nav.cell + 0.5) as i32,
        );
        for ring in 0..rings {
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
                    let cell = j as u32 * self.nav.nx + i as u32;
                    for node in core::iter::once(cell).chain(self.over(cell)) {
                        if !self.open(node)
                            || abs(self.floor[node as usize] as f32 * 0.01 - y) > within
                        {
                            continue;
                        }
                        let p = self.node_pos(node);
                        let d = (p.x - x) * (p.x - x) + (p.z - z) * (p.z - z);
                        if best.is_none_or(|b| d < b.0) {
                            best = Some((d, node));
                        }
                    }
                }
            }
            if let Some((_, node)) = best {
                return Some(node);
            }
        }
        None
    }

    /// Whether a soldier can walk straight from one node to another: every node along the way is
    /// open, no step between two of them is too tall, and the walk ends on that node and not on
    /// one over or under it.
    pub fn walkable(&self, from: u32, to: u32) -> bool {
        let (from, goal) = (self.node_pos(from), (from, to));
        let d = (self.node_pos(to) - from).flat();
        let length = d.len();
        // Fine enough that no node the line clips at a corner goes unasked: a soldier takes the
        // line a few centimetres at a time and stops at a closed node however little of it he meets.
        let steps = (length / (self.nav.cell * 0.2)) as i32 + 1;
        let (mut last, mut under) = (goal.0, self.cell_of(goal.0));
        for s in 1..=steps {
            let p = from + d * (s as f32 / steps as f32);
            let Some(cell) = self.node(p.x, p.z) else {
                return false;
            };
            if cell != under {
                let Some(node) = self.toward(last, cell) else {
                    return false;
                };
                (last, under) = (node, cell);
            }
        }
        last == goal.1
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
