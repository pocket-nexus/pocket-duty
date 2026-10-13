//! Ways across the walk grid.
//!
//! The grid is the reference's (`web/src/ai/nav.js`): a node every 0.8 m,
//! open where a soldier can stand, with the floor's height there. A way is
//! found by A* over the eight neighbours, as the reference does, and pulled
//! straight where the grid allows a straight walk.
//!
//! This game adds two things to it. Where a floor stands over a floor a cell has a node for each
//! (`Level::over`), and a step goes to whichever node of the neighbouring cell is within a step's
//! height: a stair's head and a bridge's end join the two floors with no record of their own. And
//! a ladder (`Level::links`) is a step between the nodes at its foot and its top.
//!
//! A search can visit thousands of nodes, and a step of the game has room for
//! a few hundred: one search runs at a time, a slice a step, and the soldiers
//! wait their turn for a moment. A search opens a node once and has no limit of
//! its own: on a grid whose open nodes hang together it ends at its goal. The
//! way it finds is pulled straight in the steps after, a few points a step.

use alloc::vec;
use alloc::vec::Vec;

use crate::level::Level;
use crate::math::*;

/// Points a way holds at most.
pub const POINTS: usize = 32;
/// Sixteenths of the distance left that a search's estimate is (`estimate`): 16 is the distance
/// itself, and more leans the search toward the goal.
const LEAN: u32 = 32;
/// Nodes of a way a point may pass over: a straight walk is looked for this far ahead.
const AHEAD: usize = 24;
/// A step that pulls a way straight makes a point for this many nodes it might have opened.
const PULL: u32 = 24;
/// Nodes of a found way that are kept for the pull: what its points can pass over.
const HELD: usize = POINTS * AHEAD + 1;

#[derive(Clone, Copy, Debug)]
pub struct Path {
    pub points: [V3; POINTS],
    pub len: u8,
    /// A bit a point: he is carried to it from the point before it, along a ladder when the
    /// point is a ladder's end, and along the floor from that end to the node by it.
    pub climbs: u32,
}

impl Default for Path {
    fn default() -> Self {
        Path {
            points: [V3::ZERO; POINTS],
            len: 0,
            climbs: 0,
        }
    }
}

/// A step out of a node toward one of the eight cells about it: what it costs to get that far,
/// the two cells a diagonal passes between, what a crouch adds, and that cell and the goal's.
#[derive(Clone, Copy)]
struct Step {
    from: u32,
    here: u32,
    sides: Option<(u32, u32)>,
    crouch: u32,
    at: (i32, i32),
    goal: (i32, i32),
    costs: Costs,
}

struct Search {
    owner: u8,
    from: u32,
    to: u32,
    opened: u32,
    /// Once the goal is reached: the node of `Pathfinder::way` the way's last point stands at,
    /// and the points so far (`pull`); and whether `way` stops short of the goal.
    pulled: Option<(usize, Path)>,
    short: bool,
}

/// A step straight to the next cell and one to a cell across a corner, in eighths of a metre.
#[derive(Clone, Copy)]
struct Costs {
    straight: u32,
    across: u32,
}

impl Costs {
    fn of(level: &Level) -> Costs {
        let straight = (level.nav.cell * 8.0) as u32;
        Costs { straight, across: straight * 181 / 128 }
    }
}

pub struct Pathfinder {
    /// Per node: the cost to reach it in eighths of a metre, the node before it, and the search
    /// that last touched it: `epoch` for a node a way has reached, `epoch + 1` once the search has
    /// opened it (its cost is then settled and no later way to it is taken).
    cost: Vec<u16>,
    before: Vec<u16>,
    mark: Vec<u16>,
    epoch: u16,
    /// The reached nodes, the cheapest estimate first: (estimate, node). A node a cheaper way
    /// reaches again has an entry for each; the dearer ones are passed over when they come up.
    heap: Vec<(u32, u16)>,
    search: Option<Search>,
    /// The nodes of the way the search in hand found, from its start on: to its goal, or `HELD`
    /// of them.
    way: Vec<u16>,
    /// Empty on a grid whose open nodes hang together, as a cooked pack's do. After a search
    /// that opened every node it could reach and did not come to its goal: a number a node for
    /// the part of the grid it stands in, 0 where no such search has been. A way between two
    /// parts is then not there, and no node is opened again to learn it.
    part: Vec<u8>,
    parts: u8,
    /// Who waits: (owner, from, to).
    waiting: Vec<(u8, u32, u32)>,
    /// Ways asked for, found, not there, and given up (none is: a search opens a node once, so it
    /// ends by itself).
    pub counts: [u32; 4],
    /// Nodes the searches opened in all, and the most one opened.
    pub opened: (u64, u32),
    /// Ways with more corners than a way has points: they end at the last corner they hold.
    pub cut: u32,
    /// Sixteenths of the distance left that the estimate is (`LEAN`).
    pub lean: u32,
}

impl Pathfinder {
    pub fn new(nodes: usize) -> Pathfinder {
        Pathfinder {
            cost: vec![0; nodes],
            before: vec![0; nodes],
            mark: vec![0; nodes],
            epoch: 0,
            heap: Vec::with_capacity(1024),
            way: Vec::with_capacity(HELD),
            part: Vec::new(),
            parts: 0,
            search: None,
            counts: [0; 4],
            waiting: Vec::with_capacity(16),
            opened: (0, 0),
            cut: 0,
            lean: LEAN,
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

    /// The distance left from a cell to the goal's as the steps' own costs count it at best
    /// (across corners while both columns and rows are left, then straight), in eighths of a
    /// metre. It never runs over what the way costs, so a node the search opens is reached by its
    /// cheapest way and is opened once. (The reference's runs 6% over a metre's length and this
    /// game's steps were counted shorter than a metre's: 13% over a straight step and 20% over
    /// one across, so a search opened a node again for every cheaper way found to it later.)
    #[inline(always)]
    fn estimate(&self, costs: Costs, at: (i32, i32), goal: (i32, i32)) -> u32 {
        let (dx, dz) = ((at.0 - goal.0).unsigned_abs(), (at.1 - goal.1).unsigned_abs());
        let (long, short) = (dx.max(dz), dx.min(dz));
        ((long - short) * costs.straight + short * costs.across) * self.lean / 16
    }

    /// A reached node's place in the heap: by its cost and the estimate from it, and of two alike
    /// the one farther along its way first.
    #[inline(always)]
    fn key(cost: u32, estimate: u32) -> u32 {
        (cost + estimate) << 15 | (0x7fff - (cost >> 1))
    }

    /// A node's column and row on the grid.
    #[inline]
    fn place(level: &Level, node: u32) -> (i32, i32) {
        let (cell, nx) = (level.cell_of(node) as i32, level.nav.nx as i32);
        (cell % nx, cell / nx)
    }

    /// Works on the search in hand for `budget` nodes. Returns a way when one is done: the owner
    /// and the way (empty when there is none).
    pub fn step(&mut self, level: &Level, budget: u32) -> Option<(u8, Path)> {
        // A way that is found is pulled straight a few points a step, and no node is opened in
        // those steps: the step that ends a search is no longer than another.
        if self.search.as_ref().is_some_and(|s| s.pulled.is_some()) {
            return self.pull(level, (budget / PULL).max(1));
        }
        // A place of one level, with no ladder, is searched by a loop that asks neither of a node.
        if level.over.is_empty() && level.links.is_empty() {
            self.search::<false>(level, budget)
        } else {
            self.search::<true>(level, budget)
        }
    }

    fn search<const STOREYS: bool>(&mut self, level: &Level, budget: u32) -> Option<(u8, Path)> {
        if self.search.is_none() {
            if self.waiting.is_empty() {
                return None;
            }
            let (owner, from, to) = self.waiting.remove(0);
            // (two marks a search: reached, and opened)
            self.epoch = if self.epoch >= u16::MAX - 3 { 0 } else { self.epoch + 2 };
            if self.epoch == 0 {
                self.mark.fill(0);
                self.epoch = 2;
            }
            self.heap.clear();
            // A way from or to a closed node is not there, and one between two parts of the grid
            // that an earlier search found apart: no node is opened to learn either.
            let apart = !self.part.is_empty() && {
                let (a, b) = (self.part[from as usize], self.part[to as usize]);
                a != b && (a != 0 || b != 0)
            };
            if level.open(from) && level.open(to) && !apart {
                self.mark[from as usize] = self.epoch;
                self.cost[from as usize] = 0;
                self.before[from as usize] = from as u16;
                self.push(Self::key(0, self.estimate(Costs::of(level), Self::place(level, from), Self::place(level, to))), from as u16);
            }
            self.search = Some(Search {
                owner,
                from,
                to,
                opened: 0,
                pulled: None,
                short: false,
            });
        }
        let (owner, from, to) = {
            let s = self.search.as_ref().unwrap();
            (s.owner, s.from, s.to)
        };
        let nx = level.nav.nx as i32;
        let costs = Costs::of(level);
        let step_cost = costs.straight;
        let goal = Self::place(level, to);
        let settled = self.epoch + 1;
        // A step opens `budget` nodes. An entry passed over is an eighth of one: it is taken off
        // the heap and nothing is asked of the grid.
        let mut left = budget * 8;
        while left > 0 {
            let Some((_, node)) = self.pop() else {
                // Every node the start reaches is opened and the goal is none of them: they are
                // a part of the grid by themselves.
                if self.search.as_ref().is_some_and(|s| s.opened > 0) && self.parts < u8::MAX {
                    self.parts += 1;
                    if self.part.is_empty() {
                        self.part = vec![0; self.mark.len()];
                    }
                    for (part, mark) in self.part.iter_mut().zip(&self.mark) {
                        if *mark == settled {
                            *part = self.parts;
                        }
                    }
                }
                self.done();
                self.counts[2] += 1;
                return Some((owner, Path::default()));
            };
            let node = node as u32;
            // (an entry of a node a cheaper way reached since: that way's entry came up before)
            if self.mark[node as usize] == settled {
                left -= 1;
                continue;
            }
            left = left.saturating_sub(8);
            if node == to {
                // The way's nodes, back from the goal: the ones from its start on, as many as a
                // way's points can pass over. The steps after this one pull them straight.
                let (mut length, mut at) = (1, to);
                while at != from && length < self.before.len() {
                    length += 1;
                    at = self.before[at as usize] as u32;
                }
                let held = length.min(HELD);
                self.way.clear();
                self.way.resize(held, 0);
                let (mut k, mut at) = (length, to);
                while k > 0 {
                    k -= 1;
                    if k < held {
                        self.way[k] = at as u16;
                    }
                    at = self.before[at as usize] as u32;
                }
                let search = self.search.as_mut().unwrap();
                search.short = held < length;
                search.pulled = Some((0, Path::default()));
                return None;
            }
            self.mark[node as usize] = settled;
            self.search.as_mut().unwrap().opened += 1;
            let (i, j) = if STOREYS { Self::place(level, node) } else { (node as i32 % nx, node as i32 / nx) };
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
                // The cell's first node, then the ones over it where a floor stands over a floor.
                let beside = (nj * nx + ni) as u32;
                let step = Step {
                    from: node,
                    here: here + if di != 0 && dj != 0 { costs.across } else { step_cost },
                    sides: (di != 0 && dj != 0).then_some(((j * nx + ni) as u32, (nj * nx + i) as u32)),
                    crouch: step_cost * 8 / 5,
                    at: (ni, nj),
                    goal,
                    costs,
                };
                self.visit::<STOREYS>(level, step, beside);
                if STOREYS && level.flags[beside as usize] & crate::level::OVER != 0 {
                    self.visit_over(level, step, beside);
                }
            }
            // A ladder from here: a metre of it costs five of a walk.
            if let Some((next, _)) = level.ladder(node).filter(|_| STOREYS) {
                let rise = (level.floor[node as usize] as i32 - level.floor[next as usize] as i32)
                    .unsigned_abs();
                self.reach(costs, node, next, here + step_cost + rise * 2 / 5, Self::place(level, next), goal);
            }
        }
        None
    }

    /// A step from a node to one at a cell beside it.
    #[inline(always)]
    fn visit<const STOREYS: bool>(&mut self, level: &Level, step: Step, next: u32) {
        if !level.passable(step.from, next) {
            return;
        }
        // No cutting a corner: a diagonal needs both of its sides open.
        if let Some((a, b)) = step.sides {
            if if STOREYS {
                level.toward(step.from, a).is_none() || level.toward(step.from, b).is_none()
            } else {
                !level.passable(step.from, a) || !level.passable(step.from, b)
            } {
                return;
            }
        }
        let rise = (level.floor[step.from as usize] as i32 - level.floor[next as usize] as i32).unsigned_abs();
        let mut cost = step.here + rise * 22 / 125;
        if level.flags[next as usize] & 3 == 2 {
            cost += step.crouch;
        }
        self.reach(step.costs, step.from, next, cost, step.at, step.goal);
    }

    /// The same to the nodes over a cell's first.
    #[inline(never)]
    fn visit_over(&mut self, level: &Level, step: Step, beside: u32) {
        for next in level.over(beside) {
            self.visit::<true>(level, step, next);
        }
    }

    /// Notes a cheaper way to a node, which stands at the cell `at`, and puts it among the reached.
    #[inline(always)]
    fn reach(&mut self, costs: Costs, from: u32, next: u32, cost: u32, at: (i32, i32), goal: (i32, i32)) {
        if cost > u16::MAX as u32 {
            return;
        }
        let mark = self.mark[next as usize];
        // (an opened node's cost is settled)
        if mark == self.epoch + 1 {
            return;
        }
        if mark != self.epoch || cost < self.cost[next as usize] as u32 {
            self.mark[next as usize] = self.epoch;
            self.cost[next as usize] = cost as u16;
            self.before[next as usize] = from as u16;
            self.push(Self::key(cost, self.estimate(costs, at, goal)), next as u16);
        }
    }

    /// Ends the search in hand and counts what it opened.
    fn done(&mut self) {
        if let Some(search) = self.search.take() {
            self.opened.0 += search.opened as u64;
            self.opened.1 = self.opened.1.max(search.opened);
        }
    }

    /// Pulls the way in hand straight, `points` of it a step: from each point the farthest later
    /// node a soldier can walk straight to. Returns the way when its goal or its last point is
    /// reached.
    fn pull(&mut self, level: &Level, points: u32) -> Option<(u8, Path)> {
        let (owner, to, short, (mut i, mut path)) = {
            let s = self.search.as_ref().unwrap();
            (s.owner, s.to, s.short, s.pulled.unwrap())
        };
        let nodes = &self.way;
        let node = |k: usize| nodes[k] as u32;
        let mut left = points;
        while i + 1 < nodes.len() && (path.len as usize) < POINTS {
            if left == 0 {
                self.search.as_mut().unwrap().pulled = Some((i, path));
                return None;
            }
            left -= 1;
            // A ladder is two points after the node at its near end: the ladder's own far end,
            // and the node there. He is carried to both, the second along the floor: a ladder's
            // own ends stand by a wall, where the grid may be closed.
            if let Some((other, link)) = level
                .ladder(node(i))
                .filter(|(other, _)| *other == node(i + 1) && !level.passable(node(i), *other))
            {
                if path.len as usize + 2 > POINTS {
                    break;
                }
                path.points[path.len as usize] = if link.nodes[0] == node(i) { link.top() } else { link.foot() };
                path.points[path.len as usize + 1] = level.node_pos(other);
                path.climbs |= 0b11 << path.len;
                path.len += 2;
                i += 1;
                continue;
            }
            // The farthest of the next nodes in a straight walk. Along a way the straight walks
            // from a node are open as far as some node and closed past it, so that node is looked
            // for by halves, between the next one and the one `AHEAD`.
            let reach = (i + AHEAD).min(nodes.len() - 1);
            let mut next = i + 1;
            if reach > next {
                if level.walkable(node(i), node(reach)) {
                    next = reach;
                } else {
                    let mut past = reach;
                    while past - next > 1 {
                        let middle = (next + past) / 2;
                        if level.walkable(node(i), node(middle)) {
                            next = middle;
                        } else {
                            past = middle;
                        }
                    }
                }
            }
            path.points[path.len as usize] = level.node_pos(node(next));
            path.len += 1;
            i = next;
        }
        // A way with more corners than it has points ends at the last corner it holds: whoever
        // walks it asks again from there.
        let whole = i + 1 >= nodes.len() && !short;
        if whole && path.len > 0 {
            path.points[path.len as usize - 1] = level.node_pos(to);
        }
        self.done();
        self.counts[1] += 1;
        self.cut += !whole as u32;
        Some((owner, path))
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
            over: 0,
            links: 0,
        },
        flags,
        vec![0; (nx * nz) as usize],
    )
}

/// A place for tests, with floors over floors: `rows` of text, a node every 0.8 m, `#` closed and
/// a letter a floor (`a` at 0 m, each letter after it 0.4 m higher); `over` the same rows with a
/// letter where a further node stands over the first.
#[cfg(test)]
pub(crate) struct Plan {
    pub nav: crate::level::NavHeader,
    pub flags: Vec<u8>,
    pub floor: Vec<i16>,
    pub over: Vec<u16>,
    pub links: Vec<crate::level::Link>,
}

#[cfg(test)]
impl Plan {
    pub fn new(rows: &[&str], over: &[&str]) -> Plan {
        let (mut nav, mut flags, mut floor) = grid(rows, 0.8);
        let height = |b: u8| (b.saturating_sub(b'a')) as i16 * 40;
        for (at, b) in rows.iter().flat_map(|r| r.bytes()).enumerate() {
            floor[at] = height(b);
        }
        let mut cells = Vec::new();
        for (at, b) in over.iter().flat_map(|r| r.bytes()).enumerate() {
            if b.is_ascii_lowercase() {
                flags[at] |= crate::level::OVER;
                cells.push(at as u16);
                flags.push(1);
                floor.push(height(b));
            }
        }
        nav.over = cells.len() as u16;
        Plan { nav, flags, floor, over: cells, links: Vec::new() }
    }

    /// The node at a column and a row nearest a height.
    pub fn at(&self, column: u32, row: u32, y: f32) -> u32 {
        self.level().node_at(column as f32 * 0.8, row as f32 * 0.8, y).unwrap()
    }

    /// A ladder between two nodes, a soldier's too.
    pub fn ladder(&mut self, foot: u32, top: u32) {
        let (a, b) = (self.level().node_pos(foot), self.level().node_pos(top));
        self.links.push(crate::level::Link { foot: [a.x, a.y, a.z], top: [b.x, b.y, b.z], nodes: [foot, top] });
        self.flags[foot as usize] |= crate::level::LADDER;
        self.flags[top as usize] |= crate::level::LADDER;
        self.nav.links = self.links.len() as u16;
    }

    pub fn level(&self) -> Level<'_> {
        Level {
            solid: crate::bvh::Bvh { nodes: &[], tris: &[], verts: &[] },
            nav: self.nav,
            flags: &self.flags,
            floor: &self.floor,
            cover: &[],
            spawns: &[],
            over: &self.over,
            links: &self.links,
        }
    }

    /// The way the pathfinder finds between two nodes.
    pub fn way(&self, from: u32, to: u32) -> Path {
        let level = self.level();
        let mut finder = Pathfinder::new(self.flags.len());
        finder.ask(0, from, to);
        (0..200).find_map(|_| finder.step(&level, 100)).expect("an answer").1
    }
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
            over: &[],
            links: &[],
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
        let mut at = 4;
        for p in &path.points[..path.len as usize] {
            let next = level.node(p.x, p.z).unwrap();
            assert!(level.walkable(at, next), "{at:?} to {p:?}");
            at = next;
        }
    }

    /// A level of one floor over a test grid.
    fn flat<'a>(nav: crate::level::NavHeader, flags: &'a [u8], floor: &'a [i16]) -> Level<'a> {
        Level { solid: Bvh { nodes: &[], tris: &[], verts: &[] }, nav, flags, floor, cover: &[], spawns: &[], over: &[], links: &[] }
    }

    #[test]
    fn a_way_that_is_not_there_is_searched_for_once() {
        // Two rooms of twelve nodes with no door between them.
        let (nav, flags, floor) = grid(&["....#....", "....#....", "....#...."], 0.8);
        let level = flat(nav, &flags, &floor);
        let mut finder = Pathfinder::new(flags.len());
        finder.ask(0, 0, 8);
        let done = (0..50).find_map(|_| finder.step(&level, 5)).expect("an answer");
        assert_eq!(done.1.len, 0);
        // The search opened every node of the first room, and none of them twice.
        assert_eq!((finder.opened, finder.counts), ((12, 12), [1, 0, 1, 0]));
        // Asked again, from either room: the answer comes in one step and no node is opened.
        for (from, to) in [(0, 8), (10, 17), (8, 0), (26, 9)] {
            finder.ask(0, from, to);
            assert_eq!(finder.step(&level, 5).expect("an answer at once").1.len, 0);
        }
        assert_eq!((finder.opened, finder.counts), ((12, 12), [5, 0, 5, 0]));
        // A way inside either room is found as before.
        for (from, to) in [(0, 21), (5, 26)] {
            finder.ask(0, from, to);
            assert!((0..50).find_map(|_| finder.step(&level, 5)).expect("an answer").1.len > 0);
        }
        // A closed node has no way to it, and none is looked for.
        let opened = finder.opened.0;
        finder.ask(0, 0, 4);
        assert_eq!(finder.step(&level, 5).expect("an answer at once").1.len, 0);
        assert_eq!(finder.opened.0, opened);
    }

    #[test]
    fn a_way_across_a_large_place_is_found_in_steps_of_the_budget() {
        // 61 corridors of 120 nodes, each joined to the next at one end: the way from the first to the
        // last passes every one of them, 7 380 nodes, more than a search once opened before it gave up.
        let rows: Vec<String> = (0..121)
            .map(|r| match r % 4 {
                1 => format!("{}.", "#".repeat(119)),
                3 => format!(".{}", "#".repeat(119)),
                _ => ".".repeat(120),
            })
            .collect();
        let rows: Vec<&str> = rows.iter().map(|r| r.as_str()).collect();
        let (nav, flags, floor) = grid(&rows, 0.8);
        let level = flat(nav, &flags, &floor);
        let open = flags.iter().filter(|f| **f != 0).count() as u32;
        let (from, to) = (0u32, 120 * 120);
        let mut finder = Pathfinder::new(flags.len());
        finder.ask(0, from, to);
        let mut steps = 0;
        let way = loop {
            steps += 1;
            assert!(steps < 200, "no answer in 200 steps");
            if let Some((_, way)) = finder.step(&level, 120) {
                break way;
            }
        };
        assert_eq!(finder.counts, [1, 1, 0, 0]);
        // Every node on the way is opened, each once, and a step opens 120 at most.
        assert!(finder.opened.1 > 7000 && finder.opened.1 <= open, "{} of {open} nodes opened", finder.opened.1);
        assert!(steps * 120 >= finder.opened.1, "{} nodes in {steps} steps", finder.opened.1);
        // The way has 120 corners and a way holds `POINTS`: it ends at the last corner it holds, every
        // leg of it a straight walk, and whoever walks it asks again from there until he is at the goal.
        assert_eq!((way.len as usize, finder.cut), (POINTS, 1));
        let (mut at, mut asked, mut way) = (from, 1, way);
        loop {
            for p in &way.points[..way.len as usize] {
                let next = level.node(p.x, p.z).unwrap();
                assert!(level.walkable(at, next), "{at} to {next}");
                at = next;
            }
            if at == to {
                break;
            }
            asked += 1;
            assert!(asked <= 20, "the goal is not reached in twenty ways");
            finder.ask(0, at, to);
            way = (0..200).find_map(|_| finder.step(&level, 120)).expect("an answer").1;
        }
        assert_eq!(finder.counts, [asked, asked, 0, 0]);
    }

    #[test]
    fn the_estimate_never_runs_over_the_way() {
        // Across an open floor the way is the estimate itself, so the search opens the nodes along it
        // and their neighbours and no others; with an estimate that ran over, it took the first way
        // it met. Twenty columns and five rows away: fifteen steps straight and five across.
        let rows = ["........................"; 12];
        let (nav, flags, floor) = grid(&rows, 0.8);
        let level = flat(nav, &flags, &floor);
        let mut finder = Pathfinder::new(flags.len());
        finder.lean = 16;
        let (from, to) = (2 * 24 + 1, 7 * 24 + 21);
        let costs = Costs::of(&level);
        assert_eq!(finder.estimate(costs, Pathfinder::place(&level, from), Pathfinder::place(&level, to)), 15 * costs.straight + 5 * costs.across);
        finder.ask(0, from, to);
        let way = (0..50).find_map(|_| finder.step(&level, 120)).expect("an answer").1;
        assert_eq!(way.len, 1, "a straight walk");
        assert_eq!(finder.cost[to as usize] as u32, 15 * costs.straight + 5 * costs.across);
        assert!(finder.opened.1 <= 120, "{} nodes opened for a way of 20", finder.opened.1);
    }

    /// Two banks 3.2 m over a way between them, a bridge from bank to bank over the way (row 2),
    /// and a slope from the left bank down into the way (column 2, rows 4 to 12).
    fn bridge() -> Plan {
        Plan::new(
            &[
                "iiiaaaiii", "iiiaaaiii", "iiiaaaiii", "iiiaaaiii", "iiiaaaiii", "iihaaaiii", "iigaaaiii",
                "iifaaaiii", "iieaaaiii", "iidaaaiii", "iicaaaiii", "iibaaaiii", "iiaaaaiii", "iiaaaaiii",
            ],
            &["", "", "   iii   "].map(|row| if row.is_empty() { "         " } else { row }),
        )
    }

    /// The metres of a way from a node, and whether every point of it lies between two heights.
    fn length(plan: &Plan, from: u32, way: &Path, low: f32, high: f32) -> (f32, bool) {
        let (mut at, mut metres, mut within) = (plan.level().node_pos(from), 0.0, true);
        for p in &way.points[..way.len as usize] {
            metres += (*p - at).len();
            within &= p.y >= low && p.y <= high;
            at = *p;
        }
        (metres, within)
    }

    #[test]
    fn a_cell_has_a_node_a_floor() {
        let plan = bridge();
        let level = plan.level();
        let (on, under) = (plan.at(4, 2, 3.0), plan.at(4, 2, 0.2));
        assert!(on != under && level.cell_of(on) == level.cell_of(under));
        assert_eq!((level.floor[on as usize], level.floor[under as usize]), (320, 0));
        assert_eq!(level.node_pos(on).flat(), level.node_pos(under).flat());
        // A step from the bank goes onto the bridge, and a step along the way goes under it.
        assert_eq!(level.toward(plan.at(2, 2, 3.2), level.cell_of(plan.at(3, 2, 0.0))), Some(plan.at(3, 2, 3.2)));
        assert_eq!(level.toward(plan.at(3, 1, 0.0), level.cell_of(plan.at(3, 2, 0.0))), Some(plan.at(3, 2, 0.0)));
        // Beside the bridge there is the way's floor only, and no step down to it.
        assert_eq!(level.toward(plan.at(4, 2, 3.2), level.cell_of(plan.at(4, 3, 0.0))), None);
        // The nearest node to someone on the bridge is the bridge's, and to someone under it the way's.
        assert_eq!(level.nearest_open(3.2, 1.6, 3.1), Some(on));
        assert_eq!(level.nearest_open(3.2, 1.6, 0.1), Some(under));
        assert!(level.walkable(plan.at(0, 2, 3.2), plan.at(8, 2, 3.2)));
        assert!(!level.walkable(plan.at(0, 2, 3.2), plan.at(4, 2, 0.0)));
        assert!(!level.walkable(plan.at(0, 3, 3.2), plan.at(8, 3, 3.2)));
    }

    #[test]
    fn a_way_goes_over_a_bridge_and_another_under_it() {
        let plan = bridge();
        // Bank to bank: straight over the bridge, 6.4 m, never down.
        let (from, to) = (plan.at(0, 2, 3.2), plan.at(8, 2, 3.2));
        let over = plan.way(from, to);
        let (metres, high) = length(&plan, from, &over, 3.1, 3.3);
        assert!(over.len > 0 && high && abs(metres - 6.4) < 0.1, "{metres} m in {} points", over.len);
        // Along the way: straight under the bridge, 3.2 m, never up.
        let (from, to) = (plan.at(4, 0, 0.0), plan.at(4, 4, 0.0));
        let under = plan.way(from, to);
        let (metres, low) = length(&plan, from, &under, -0.1, 0.1);
        assert!(under.len > 0 && low && abs(metres - 3.2) < 0.1, "{metres} m in {} points", under.len);
        // From the bridge to the ground under it: round by the bank and down the slope.
        let (from, to) = (plan.at(4, 2, 3.2), plan.at(4, 2, 0.0));
        let round = plan.way(from, to);
        let (metres, _) = length(&plan, from, &round, 0.0, 0.0);
        assert!(round.len > 2 && metres > 16.0, "{metres} m in {} points", round.len);
        assert_eq!(round.points[round.len as usize - 1], plan.level().node_pos(to));
        assert_eq!(over.climbs | under.climbs | round.climbs, 0);
    }

    #[test]
    fn a_ladder_is_a_way_up_and_down() {
        // A floor 3.2 m over another with no slope between them.
        let mut plan = Plan::new(&["aaaaiiii", "aaaaiiii", "aaaaiiii"], &["        "; 3]);
        let (low, high) = (plan.at(0, 1, 0.0), plan.at(7, 1, 3.2));
        assert_eq!(plan.way(low, high).len, 0);
        plan.ladder(plan.at(3, 1, 0.0), plan.at(4, 1, 3.2));
        let level = plan.level();
        let (foot, top) = (level.node_pos(plan.at(3, 1, 0.0)), level.node_pos(plan.at(4, 1, 3.2)));
        let up = plan.way(low, high);
        // (the foot's node, the ladder's top, the top's node, the goal)
        assert_eq!((up.len, up.climbs), (4, 0b0110), "{:?}", &up.points[..up.len as usize]);
        assert_eq!((up.points[0], up.points[1], up.points[2], up.points[3]), (foot, top, top, level.node_pos(high)));
        let down = plan.way(high, low);
        assert_eq!((down.len, down.climbs), (4, 0b0110), "{:?}", &down.points[..down.len as usize]);
        assert_eq!((down.points[0], down.points[1], down.points[2], down.points[3]), (top, foot, foot, level.node_pos(low)));
        // A way from one end of a ladder to the other is the ladder alone.
        let short = plan.way(plan.at(3, 1, 0.0), plan.at(4, 1, 3.2));
        assert_eq!((short.len, short.climbs, short.points[0], short.points[1]), (2, 0b11, top, top));
    }
}
