//! Ways across the walk grid.
//!
//! The grid is the reference's (`web/src/ai/nav.js`): a node every 0.8 m,
//! open where a soldier can stand, with the floor's height there. A way is
//! found by A* over the eight neighbours, as the reference does, and pulled
//! straight where the grid allows a straight walk.
//!
//! A search can visit thousands of nodes, and a step of the game has room for
//! a few hundred: one search runs at a time, a slice a step, and the soldiers
//! wait their turn for a moment.

use alloc::vec;
use alloc::vec::Vec;

use crate::level::Level;
use crate::math::*;

/// Points a way holds at most.
pub const POINTS: usize = 16;
/// Nodes a search may open before it gives up (`nav.js:233`).
const LIMIT: u32 = 6000;

#[derive(Clone, Copy, Debug)]
pub struct Path {
    pub points: [V3; POINTS],
    pub len: u8,
}

impl Default for Path {
    fn default() -> Self {
        Path {
            points: [V3::ZERO; POINTS],
            len: 0,
        }
    }
}

struct Search {
    owner: u8,
    from: u32,
    to: u32,
    opened: u32,
}

pub struct Pathfinder {
    /// Per node: the cost to reach it in eighths of a metre, the node before it, and the search
    /// that last touched it.
    cost: Vec<u16>,
    before: Vec<u16>,
    mark: Vec<u16>,
    epoch: u16,
    /// The open nodes, the cheapest estimate first: (estimate, node).
    heap: Vec<(u32, u16)>,
    search: Option<Search>,
    /// Who waits: (owner, from, to).
    waiting: Vec<(u8, u32, u32)>,
    /// Ways asked for, found, not there, and given up after `LIMIT` nodes.
    pub counts: [u32; 4],
}

impl Pathfinder {
    pub fn new(nodes: usize) -> Pathfinder {
        Pathfinder {
            cost: vec![0; nodes],
            before: vec![0; nodes],
            mark: vec![0; nodes],
            epoch: 0,
            heap: Vec::with_capacity(1024),
            search: None,
            counts: [0; 4],
            waiting: Vec::with_capacity(16),
        }
    }

    /// Asks for a way; a later ask by the same owner takes the earlier one's place.
    pub fn ask(&mut self, owner: u8, from: u32, to: u32) {
        self.waiting.retain(|w| w.0 != owner);
        if self.search.as_ref().is_some_and(|s| s.owner == owner) {
            self.search = None;
        }
        if self.waiting.len() < 16 {
            self.waiting.push((owner, from, to));
            self.counts[0] += 1;
        }
    }

    pub fn forget(&mut self, owner: u8) {
        self.waiting.retain(|w| w.0 != owner);
        if self.search.as_ref().is_some_and(|s| s.owner == owner) {
            self.search = None;
        }
    }

    fn push(&mut self, key: u32, node: u16) {
        let h = &mut self.heap;
        h.push((key, node));
        let mut i = h.len() - 1;
        while i > 0 {
            let parent = (i - 1) / 2;
            if h[parent].0 <= h[i].0 {
                break;
            }
            h.swap(parent, i);
            i = parent;
        }
    }

    fn pop(&mut self) -> Option<(u32, u16)> {
        let h = &mut self.heap;
        if h.is_empty() {
            return None;
        }
        let top = h.swap_remove(0);
        let mut i = 0;
        loop {
            let (l, r) = (i * 2 + 1, i * 2 + 2);
            let mut least = i;
            if l < h.len() && h[l].0 < h[least].0 {
                least = l;
            }
            if r < h.len() && h[r].0 < h[least].0 {
                least = r;
            }
            if least == i {
                break;
            }
            h.swap(i, least);
            i = least;
        }
        Some(top)
    }

    /// The distance left as the grid allows it at best, in eighths of a metre, a little long so
    /// the search leans toward the goal (`nav.js:260`).
    fn estimate(level: &Level, a: u32, b: u32) -> u32 {
        let nx = level.nav.nx;
        let (dx, dz) = (
            ((a % nx) as i32 - (b % nx) as i32).unsigned_abs(),
            ((a / nx) as i32 - (b / nx) as i32).unsigned_abs(),
        );
        let (long, short) = (dx.max(dz), dx.min(dz));
        ((long as f32 + short as f32 * 0.414) * level.nav.cell * 8.0 * 1.06) as u32
    }

    /// Works on the search in hand for `budget` nodes. Returns a way when one is done: the owner
    /// and the way (empty when there is none).
    pub fn step(&mut self, level: &Level, budget: u32) -> Option<(u8, Path)> {
        if self.search.is_none() {
            if self.waiting.is_empty() {
                return None;
            }
            let (owner, from, to) = self.waiting.remove(0);
            self.epoch = self.epoch.wrapping_add(1);
            if self.epoch == 0 {
                self.mark.fill(0);
                self.epoch = 1;
            }
            self.heap.clear();
            self.mark[from as usize] = self.epoch;
            self.cost[from as usize] = 0;
            self.before[from as usize] = from as u16;
            self.push(Self::estimate(level, from, to), from as u16);
            self.search = Some(Search {
                owner,
                from,
                to,
                opened: 0,
            });
        }
        let (owner, from, to) = {
            let s = self.search.as_ref().unwrap();
            (s.owner, s.from, s.to)
        };
        let nx = level.nav.nx as i32;
        let step_cost = (level.nav.cell * 8.0) as u32;
        for _ in 0..budget {
            let Some((_, node)) = self.pop() else {
                self.search = None;
                self.counts[2] += 1;
                return Some((owner, Path::default()));
            };
            let node = node as u32;
            if node == to {
                let path = self.trace(level, from, to);
                self.search = None;
                self.counts[1] += 1;
                return Some((owner, path));
            }
            let opened = {
                let s = self.search.as_mut().unwrap();
                s.opened += 1;
                s.opened
            };
            if opened > LIMIT {
                self.search = None;
                self.counts[3] += 1;
                return Some((owner, Path::default()));
            }
            let (i, j) = (node as i32 % nx, node as i32 / nx);
            let here = self.cost[node as usize] as u32;
            for (di, dj) in [
                (1, 0),
                (-1, 0),
                (0, 1),
                (0, -1),
                (1, 1),
                (1, -1),
                (-1, 1),
                (-1, -1),
            ] {
                let (ni, nj) = (i + di, j + dj);
                if ni < 0 || nj < 0 || ni >= nx || nj >= level.nav.nz as i32 {
                    continue;
                }
                let next = (nj * nx + ni) as u32;
                if !level.passable(node, next) {
                    continue;
                }
                // No cutting a corner: a diagonal needs both of its sides open.
                if di != 0
                    && dj != 0
                    && (!level.passable(node, (j * nx + ni) as u32)
                        || !level.passable(node, (nj * nx + i) as u32))
                {
                    continue;
                }
                let rise = (level.floor[node as usize] as i32 - level.floor[next as usize] as i32)
                    .unsigned_abs();
                let mut cost =
                    here + if di != 0 && dj != 0 {
                        step_cost * 181 / 128
                    } else {
                        step_cost
                    } + rise * 22 / 125;
                if level.flags[next as usize] & 3 == 2 {
                    cost += step_cost * 8 / 5;
                }
                if cost > u16::MAX as u32 {
                    continue;
                }
                if self.mark[next as usize] != self.epoch || cost < self.cost[next as usize] as u32
                {
                    self.mark[next as usize] = self.epoch;
                    self.cost[next as usize] = cost as u16;
                    self.before[next as usize] = node as u16;
                    self.push(cost + Self::estimate(level, next, to), next as u16);
                }
            }
        }
        None
    }

    /// The way back from the goal, pulled straight: from each point the farthest later one a
    /// soldier can walk straight to.
    fn trace(&self, level: &Level, from: u32, to: u32) -> Path {
        let mut nodes: Vec<u32> = Vec::with_capacity(128);
        let mut at = to;
        while at != from && nodes.len() < 4096 {
            nodes.push(at);
            at = self.before[at as usize] as u32;
        }
        nodes.push(from);
        nodes.reverse();
        let mut path = Path::default();
        let mut i = 0;
        while i + 1 < nodes.len() && (path.len as usize) < POINTS {
            let here = level.node_pos(nodes[i]);
            // The farthest of the next nodes in a straight walk, looking a few at a time.
            let mut next = i + 1;
            let mut probe = (i + 24).min(nodes.len() - 1);
            while probe > i + 1 {
                if level.walkable(here, level.node_pos(nodes[probe])) {
                    next = probe;
                    break;
                }
                probe = i + 1 + (probe - i - 1) / 2;
            }
            path.points[path.len as usize] = level.node_pos(nodes[next]);
            path.len += 1;
            i = next;
        }
        // A way longer than the points hold ends at its goal all the same.
        if path.len > 0 {
            path.points[path.len as usize - 1] = if i + 1 >= nodes.len() {
                level.node_pos(to)
            } else {
                path.points[path.len as usize - 1]
            };
        }
        path
    }
}

/// A grid for tests: `rows` of text, `#` closed, anything else open, a node every `cell` metres.
#[cfg(test)]
pub(crate) fn grid(rows: &[&str], cell: f32) -> (crate::level::NavHeader, Vec<u8>, Vec<i16>) {
    let (nx, nz) = (rows[0].len() as u32, rows.len() as u32);
    let flags: Vec<u8> = rows
        .iter()
        .flat_map(|r| r.bytes().map(|b| (b != b'#') as u8))
        .collect();
    (
        crate::level::NavHeader {
            cell,
            min_x: 0.0,
            min_z: 0.0,
            nx,
            nz,
            cover: 0,
            spawns: 0,
            pad: 0,
        },
        flags,
        vec![0; (nx * nz) as usize],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bvh::Bvh;

    #[test]
    fn a_way_goes_round_a_wall() {
        let (nav, flags, floor) = grid(
            &[
                "..........",
                "..........",
                "..######..",
                "..........",
                "..........",
            ],
            0.8,
        );
        let level = Level {
            solid: Bvh {
                nodes: &[],
                tris: &[],
                verts: &[],
            },
            nav,
            flags: &flags,
            floor: &floor,
            cover: &[],
            spawns: &[],
        };
        let mut finder = Pathfinder::new(flags.len());
        // From over the wall's middle to under it.
        finder.ask(3, 4, 44);
        let mut found = None;
        for _ in 0..50 {
            if let Some(done) = finder.step(&level, 20) {
                found = Some(done);
                break;
            }
        }
        let (owner, path) = found.expect("a way");
        assert_eq!(owner, 3);
        assert!(path.len >= 2 && path.len <= 5, "{} points", path.len);
        let end = path.points[path.len as usize - 1];
        assert!(abs(end.x - 4.0 * 0.8) < 1e-3 && abs(end.z - 4.0 * 0.8) < 1e-3);
        // Every leg of it can be walked straight.
        let mut at = level.node_pos(4);
        for p in &path.points[..path.len as usize] {
            assert!(level.walkable(at, *p), "{at:?} to {p:?}");
            at = *p;
        }
    }

    #[test]
    fn a_closed_goal_has_no_way() {
        let (nav, flags, floor) = grid(&["....#....", "....#....", "....#...."], 0.8);
        let level = Level {
            solid: Bvh {
                nodes: &[],
                tris: &[],
                verts: &[],
            },
            nav,
            flags: &flags,
            floor: &floor,
            cover: &[],
            spawns: &[],
        };
        let mut finder = Pathfinder::new(flags.len());
        finder.ask(0, 0, 8);
        let done = (0..50)
            .find_map(|_| finder.step(&level, 50))
            .expect("an answer");
        assert_eq!(done.1.len, 0);
    }
}
