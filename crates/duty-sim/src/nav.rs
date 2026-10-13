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
    /// A bit a point: he is carried to it from the point before it, along a ladder when the
    /// point is a ladder's end, and along the floor from that end to the node by it.
    pub climbs: u16,
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

    /// The distance left from a cell to the goal's as the grid allows it at best, in eighths of a
    /// metre, a little long so the search leans toward the goal (`nav.js:260`).
    #[inline]
    fn estimate(level: &Level, at: (i32, i32), goal: (i32, i32)) -> u32 {
        let (dx, dz) = ((at.0 - goal.0).unsigned_abs(), (at.1 - goal.1).unsigned_abs());
        let (long, short) = (dx.max(dz), dx.min(dz));
        ((long as f32 + short as f32 * 0.414) * level.nav.cell * 8.0 * 1.06) as u32
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
            self.epoch = self.epoch.wrapping_add(1);
            if self.epoch == 0 {
                self.mark.fill(0);
                self.epoch = 1;
            }
            self.heap.clear();
            self.mark[from as usize] = self.epoch;
            self.cost[from as usize] = 0;
            self.before[from as usize] = from as u16;
            self.push(Self::estimate(level, Self::place(level, from), Self::place(level, to)), from as u16);
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
        let goal = Self::place(level, to);
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
                    here: here + if di != 0 && dj != 0 { step_cost * 181 / 128 } else { step_cost },
                    sides: (di != 0 && dj != 0).then_some(((j * nx + ni) as u32, (nj * nx + i) as u32)),
                    crouch: step_cost * 8 / 5,
                    at: (ni, nj),
                    goal,
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
                self.reach(level, node, next, here + step_cost + rise * 2 / 5, Self::place(level, next), goal);
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
        self.reach(level, step.from, next, cost, step.at, step.goal);
    }

    /// The same to the nodes over a cell's first.
    #[inline(never)]
    fn visit_over(&mut self, level: &Level, step: Step, beside: u32) {
        for next in level.over(beside) {
            self.visit::<true>(level, step, next);
        }
    }

    /// Notes a cheaper way to a node, which stands at the cell `at`, and opens it.
    #[inline(always)]
    fn reach(&mut self, level: &Level, from: u32, next: u32, cost: u32, at: (i32, i32), goal: (i32, i32)) {
        if cost > u16::MAX as u32 {
            return;
        }
        if self.mark[next as usize] != self.epoch || cost < self.cost[next as usize] as u32 {
            self.mark[next as usize] = self.epoch;
            self.cost[next as usize] = cost as u16;
            self.before[next as usize] = from as u16;
            self.push(cost + Self::estimate(level, at, goal), next as u16);
        }
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
            // A ladder is two points after the node at its near end: the ladder's own far end,
            // and the node there. He is carried to both, the second along the floor: a ladder's
            // own ends stand by a wall, where the grid may be closed.
            if let Some((other, link)) = level
                .ladder(nodes[i])
                .filter(|(other, _)| *other == nodes[i + 1] && !level.passable(nodes[i], *other))
            {
                if path.len as usize + 2 > POINTS {
                    break;
                }
                path.points[path.len as usize] = if link.nodes[0] == nodes[i] { link.top() } else { link.foot() };
                path.points[path.len as usize + 1] = level.node_pos(other);
                path.climbs |= 0b11 << path.len;
                path.len += 2;
                i += 1;
                continue;
            }
            // The farthest of the next nodes in a straight walk, looking a few at a time.
            let mut next = i + 1;
            let mut probe = (i + 24).min(nodes.len() - 1);
            while probe > i + 1 {
                if level.walkable(nodes[i], nodes[probe]) {
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
            over: &[],
            links: &[],
        };
        let mut finder = Pathfinder::new(flags.len());
        finder.ask(0, 0, 8);
        let done = (0..50)
            .find_map(|_| finder.step(&level, 50))
            .expect("an answer");
        assert_eq!(done.1.len, 0);
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
        // (a node on the way to the foot's, the foot's node, the ladder's top, the top's node, the goal)
        assert_eq!((up.len, up.climbs), (5, 0b01100), "{:?}", &up.points[..up.len as usize]);
        assert_eq!((up.points[1], up.points[2], up.points[3], up.points[4]), (foot, top, top, level.node_pos(high)));
        let down = plan.way(high, low);
        assert_eq!((down.len, down.climbs), (5, 0b01100), "{:?}", &down.points[..down.len as usize]);
        assert_eq!((down.points[1], down.points[2], down.points[3], down.points[4]), (top, foot, foot, level.node_pos(low)));
        // A way from one end of a ladder to the other is the ladder alone.
        let short = plan.way(plan.at(3, 1, 0.0), plan.at(4, 1, 3.2));
        assert_eq!((short.len, short.climbs, short.points[0], short.points[1]), (2, 0b11, top, top));
    }
}
