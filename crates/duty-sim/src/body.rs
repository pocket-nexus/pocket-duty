//! A body that walks the solid world.
//!
//! It is a capsule held a step's height off the ground: what is lower than a
//! step passes under it, and the feet follow the ground by rays cast down
//! through that gap. Walls and ceilings push the capsule out along the
//! shortest way. A step moves it a little at a time, never more than a
//! third of its radius, so nothing thin is passed through.
//!
//! The reference sweeps its capsule through the triangles (`web/src/physics/
//! character.js`), a dozen sweeps a step. A 333 MHz machine has no room for
//! that: here a step is a handful of rays and one pass over the triangles
//! the capsule touches.

use crate::bvh::nearest_segment_triangle;
use crate::level::Level;
use crate::math::*;
use crate::math::min as min_f;
use crate::tune::player::{SLOPE, SNAP};

/// The solid world's triangles about a body, kept from step to step.
///
/// A body moves a hand's width in a step and asks the same of the world each time: what its
/// capsule touches, and what is under its feet. Asked of the tree, that is some three hundred
/// lines of the data cache a step, each a microsecond on a PSP while the GE draws. Here the
/// triangles of a box about the body are read once and the steps that stay inside it ask those.
pub struct Near {
    /// The box the triangles were gathered in; nothing was while `max` is under `min`.
    min: [f32; 3],
    max: [f32; 3],
    /// Triangles held; `HOLDS + 1` when the box has more than that, and the world is asked itself.
    count: usize,
    corners: [[V3; 3]; Near::HOLDS],
    tags: [u32; Near::HOLDS],
    /// Times the triangles were gathered so far, and of them the times the place had too many.
    pub gathered: u32,
    pub crowded: u32,
}

impl Default for Near {
    fn default() -> Near {
        Near::new()
    }
}

impl Near {
    const HOLDS: usize = 160;
    /// Metres the gathered box reaches past what was asked for, to the sides and up and down.
    const AROUND: [f32; 3] = [0.6, 0.35, 0.6];

    pub const fn new() -> Near {
        Near { min: [0.0; 3], max: [-1.0; 3], count: 0, corners: [[V3::ZERO; 3]; Near::HOLDS], tags: [0; Near::HOLDS], gathered: 0, crowded: 0 }
    }

    /// Triangles held now.
    pub fn held(&self) -> usize {
        self.count
    }

    /// Forgets what it holds: the body was put somewhere else, or the world is another.
    pub fn clear(&mut self) {
        self.max = [-1.0; 3];
        self.min = [0.0; 3];
        self.count = 0;
    }

    /// Makes what is held cover the box `lo`…`hi`. False when the place has too many triangles
    /// to hold: the caller asks the world.
    fn covers(&mut self, level: &Level, lo: &[f32; 3], hi: &[f32; 3]) -> bool {
        let inside = (0..3).all(|a| lo[a] >= self.min[a] && hi[a] <= self.max[a]);
        if !inside {
            self.min = core::array::from_fn(|a| lo[a] - Near::AROUND[a]);
            self.max = core::array::from_fn(|a| hi[a] + Near::AROUND[a]);
            let (min, max) = (self.min, self.max);
            let mut count = 0usize;
            let (corners, tags) = (&mut self.corners, &mut self.tags);
            level.solid.within(&min, &max, |tri| {
                if count > Near::HOLDS {
                    return;
                }
                let p = [level.solid.corner(tri, 0), level.solid.corner(tri, 1), level.solid.corner(tri, 2)];
                // Its own box against this one: a leaf holds triangles that reach elsewhere.
                if outside(&p, &min, &max) {
                    return;
                }
                if count < Near::HOLDS {
                    corners[count] = p;
                    tags[count] = level.solid.tris[tri as usize].tag;
                }
                count += 1;
            });
            self.count = count;
            self.gathered += 1;
            self.crowded += (count > Near::HOLDS) as u32;
        }
        self.count <= Near::HOLDS
    }
}

/// Whether a triangle is wholly to one side of a box.
#[inline]
fn outside(p: &[V3; 3], min: &[f32; 3], max: &[f32; 3]) -> bool {
    (p[0].x < min[0] && p[1].x < min[0] && p[2].x < min[0])
        || (p[0].x > max[0] && p[1].x > max[0] && p[2].x > max[0])
        || (p[0].y < min[1] && p[1].y < min[1] && p[2].y < min[1])
        || (p[0].y > max[1] && p[1].y > max[1] && p[2].y > max[1])
        || (p[0].z < min[2] && p[1].z < min[2] && p[2].z < min[2])
        || (p[0].z > max[2] && p[1].z > max[2] && p[2].z > max[2])
}

/// The first of `triangles` a point falling from (`x`, `top`, `z`) meets within `far`: how far
/// down, how much of its unit normal points up, and its tag. Both faces of a triangle count, as
/// they do for a ray.
fn drop_on(triangles: &[[V3; 3]], tags: &[u32], x: f32, top: f32, z: f32, far: f32) -> Option<(f32, f32, u32)> {
    let mut best: Option<(f32, usize)> = None;
    for (i, [a, b, c]) in triangles.iter().enumerate() {
        // Over it at all: its box about the point, seen from above.
        if (a.x < x && b.x < x && c.x < x) || (a.x > x && b.x > x && c.x > x) || (a.z < z && b.z < z && c.z < z) || (a.z > z && b.z > z && c.z > z) {
            continue;
        }
        // Where the point is in the triangle, seen from above.
        let (e1x, e1z, e2x, e2z) = (b.x - a.x, b.z - a.z, c.x - a.x, c.z - a.z);
        let det = e1x * e2z - e1z * e2x;
        if det > -1e-9 && det < 1e-9 {
            continue;
        }
        let inv = 1.0 / det;
        let (px, pz) = (x - a.x, z - a.z);
        let u = (px * e2z - pz * e2x) * inv;
        if !(0.0..=1.0).contains(&u) {
            continue;
        }
        let v = (e1x * pz - e1z * px) * inv;
        if v < 0.0 || u + v > 1.0 {
            continue;
        }
        let t = top - (a.y + (b.y - a.y) * u + (c.y - a.y) * v);
        if t > 0.0 && t <= far && best.is_none_or(|b| t < b.0) {
            best = Some((t, i));
        }
    }
    best.map(|(t, i)| {
        let [a, b, c] = triangles[i];
        let n = (b - a).cross(c - a);
        (t, abs(n.y) / sqrt(n.len2()), tags[i])
    })
}

#[derive(Clone, Copy, Debug)]
pub struct Body {
    /// The feet.
    pub pos: V3,
    pub vel: V3,
    pub radius: f32,
    pub height: f32,
    /// What it walks over without a jump.
    pub step: f32,
    pub grounded: bool,
    /// The surface under the feet.
    pub surface: u8,
    /// Set by `advance`: the speed it landed at this step (0: it did not), and whether it met a wall.
    pub landed: f32,
    pub blocked: bool,
}

impl Body {
    pub fn new(pos: V3, radius: f32, height: f32, step: f32) -> Body {
        Body {
            pos,
            vel: V3::ZERO,
            radius,
            height,
            step,
            grounded: false,
            surface: 0,
            landed: 0.0,
            blocked: false,
        }
    }

    /// The capsule's two centres.
    fn axis(&self) -> (V3, V3) {
        let low = self.pos.y + self.step + self.radius;
        let high = max(low, self.pos.y + self.height - self.radius);
        (
            v3(self.pos.x, low, self.pos.z),
            v3(self.pos.x, high, self.pos.z),
        )
    }

    /// The box everything a step asks of the world is in: the capsule with the room its pushes
    /// take, and under the feet as far down as ground still holds them.
    fn asks(&self) -> ([f32; 3], [f32; 3]) {
        let side = self.radius + 0.13;
        ([self.pos.x - side, self.pos.y - SNAP - 0.1, self.pos.z - side], [self.pos.x + side, self.pos.y + max(self.height, self.step + 2.0 * self.radius) + 0.13, self.pos.z + side])
    }

    /// Moves by its velocity for `dt` seconds. `near` is this body's own from step to step.
    pub fn advance(&mut self, level: &Level, near: &mut Near, dt: f32) {
        self.landed = 0.0;
        self.blocked = false;
        // Standing on ground that does not move, he stays where he is: nothing to ask the world.
        if self.grounded && self.vel.len2() < 1e-6 {
            return;
        }
        let pieces = ((self.vel.len() * dt / (self.radius * 0.35)) as u32 + 1).min(6);
        let h = dt / pieces as f32;
        for _ in 0..pieces {
            self.pos += self.vel * h;
            self.push_out(level, near);
            self.find_ground(level, near, h);
        }
    }

    /// Out of every wall and ceiling the capsule is in, the deepest first.
    ///
    /// The triangles that could touch it are picked once, a little wider than the capsule, and
    /// each push looks at those alone. A triangle whose plane the capsule's whole axis is
    /// farther from than its radius is left out by two products: the nearest points of a
    /// segment and a triangle are some 350 operations of a floating point unit that takes
    /// twenty cycles for a multiply and an add, and beside a wall a body's box holds fifty
    /// triangles of window frames and planks that do not touch it.
    pub fn push_out(&mut self, level: &Level, near: &mut Near) {
        /// Triangles a push looks at at most; the ones past that are the world's finest pieces.
        const PICKED: usize = 48;
        /// How far the pushes of one call may move the capsule between them.
        const ROOM: f32 = 0.12;
        let r = self.radius;
        let (a, b) = self.axis();
        let reach = r + ROOM;
        let (min, max) = ([a.x - reach, a.y - reach, a.z - reach], [b.x + reach, b.y + reach, b.z + reach]);
        let mut picked = [[V3::ZERO; 3]; PICKED];
        let mut count = 0usize;
        let mut offer = |p: &[V3; 3]| {
            if count == PICKED || outside(p, &min, &max) {
                return;
            }
            // Its plane against the axis: both ends on one side of it and farther than the reach.
            let n = (p[1] - p[0]).cross(p[2] - p[0]);
            let (da, db) = (n.dot(a - p[0]), n.dot(b - p[0]));
            if (da > 0.0) == (db > 0.0) && min_f(da * da, db * db) > reach * reach * n.len2() {
                return;
            }
            picked[count] = *p;
            count += 1;
        };
        let (lo, hi) = self.asks();
        if near.covers(level, &lo, &hi) {
            for p in &near.corners[..near.count] {
                offer(p);
            }
        } else {
            level.solid.within(&min, &max, |tri| offer(&[level.solid.corner(tri, 0), level.solid.corner(tri, 1), level.solid.corner(tri, 2)]));
        }
        if count == 0 {
            return;
        }
        for _ in 0..4 {
            let (a, b) = self.axis();
            let mut deepest = (0.0f32, V3::ZERO);
            for [p, q, s] in &picked[..count] {
                let (p, q, s) = (*p, *q, *s);
                let n = (q - p).cross(s - p);
                let (da, db) = (n.dot(a - p), n.dot(b - p));
                if (da > 0.0) == (db > 0.0) && min_f(da * da, db * db) >= r * r * n.len2() {
                    continue;
                }
                let (on_axis, on_triangle) = nearest_segment_triangle(a, b, p, q, s);
                let d = on_axis - on_triangle;
                let dist2 = d.len2();
                if dist2 >= r * r {
                    continue;
                }
                let dist = sqrt(dist2);
                let out = if dist > 1e-4 {
                    d * (1.0 / dist)
                } else {
                    // The axis is in the triangle: out the way the triangle faces the capsule's middle.
                    let n = n.norm_or(V3::UP);
                    if n.dot((a + b) * 0.5 - p) < 0.0 {
                        -n
                    } else {
                        n
                    }
                };
                if r - dist > deepest.0 {
                    deepest = (r - dist, out);
                }
            }
            let (depth, n) = deepest;
            if depth <= 1e-4 {
                break;
            }
            self.pos += n * (depth + 0.001);
            let into = self.vel.dot(n);
            if into < 0.0 {
                self.vel -= n * into;
            }
            if abs(n.y) < 0.7 {
                self.blocked = true;
            }
        }
    }

    /// Finds the ground under the feet and stands on it.
    fn find_ground(&mut self, level: &Level, near: &mut Near, h: f32) {
        let was = self.grounded;
        let top = self.pos.y + self.step + 0.05;
        // How far below the feet ground still holds them: a walker stays on a stair going down, a
        // faller only meets what he reaches this step.
        let reach = if was && self.vel.y <= 0.01 {
            SNAP
        } else {
            max(0.02, -self.vel.y * h + 0.02)
        };
        let k = self.radius * 0.6;
        let mut best: Option<(f32, u32)> = None;
        // Under the middle, and under the foot ahead and the one behind: a step up is met by the
        // first, a ledge is left by the last. The two feet to the sides are asked only when none
        // of those found ground.
        let way = self.vel.flat().norm_or(v3(1.0, 0.0, 0.0)) * k;
        let far = self.step + 0.05 + reach;
        // The triangles the feet can be over, when they are few enough to hold.
        let (lo, hi) = self.asks();
        let held = near.covers(level, &lo, &hi) && top - far >= lo[1];
        for (i, (dx, dz)) in [(0.0, 0.0), (way.x, way.z), (-way.x, -way.z), (-way.z, way.x), (way.z, -way.x)]
            .into_iter()
            .enumerate()
        {
            if i == 3 && best.is_some() {
                break;
            }
            let (x, z) = (self.pos.x + dx, self.pos.z + dz);
            let under = if held {
                drop_on(&near.corners[..near.count], &near.tags[..near.count], x, top, z, far)
            } else {
                level.solid.raycast(v3(x, top, z), v3(0.0, -1.0, 0.0), far, 0).map(|hit| (hit.t, hit.n.y, hit.tag))
            };
            if let Some((t, up, tag)) = under {
                let y = top - t;
                if up >= SLOPE && best.is_none_or(|b| y > b.0) {
                    best = Some((y, tag));
                }
            }
        }
        match best {
            // Going up through a floor's level is not standing on it, unless the floor is above the feet.
            Some((y, tag)) if self.vel.y <= 0.01 || y > self.pos.y + 0.02 => {
                if !was {
                    self.landed = max(0.0, -self.vel.y);
                }
                self.pos.y = y;
                if self.vel.y < 0.0 {
                    self.vel.y = 0.0;
                }
                self.grounded = true;
                self.surface = (tag & 255) as u8;
            }
            _ => self.grounded = false,
        }
    }

    /// Whether the capsule would fit standing `height` tall where it is.
    pub fn fits(&self, level: &Level, height: f32) -> bool {
        let from = v3(self.pos.x, self.pos.y + self.step + 0.05, self.pos.z);
        !level
            .solid
            .occluded(from, V3::UP, height - self.step - 0.05, 0)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::bvh::{self, Bvh, Node, Tri};
    use crate::level::NavHeader;
    use alloc::vec::Vec;

    /// A solid world of boxes: `(min, max)` each.
    pub struct Boxes {
        pub verts: Vec<[f32; 3]>,
        pub tris: Vec<Tri>,
        pub nodes: Vec<Node>,
    }

    impl Boxes {
        pub fn new(boxes: &[([f32; 3], [f32; 3])]) -> Boxes {
            let (mut verts, mut tris) = (Vec::new(), Vec::new());
            for (lo, hi) in boxes {
                let base = verts.len() as u32;
                for i in 0..8 {
                    verts.push([
                        if i & 1 == 0 { lo[0] } else { hi[0] },
                        if i & 2 == 0 { lo[1] } else { hi[1] },
                        if i & 4 == 0 { lo[2] } else { hi[2] },
                    ]);
                }
                for q in [
                    [0, 2, 3, 1],
                    [4, 5, 7, 6],
                    [0, 1, 5, 4],
                    [2, 6, 7, 3],
                    [0, 4, 6, 2],
                    [1, 3, 7, 5],
                ] {
                    tris.push(Tri {
                        v: [base + q[0], base + q[1], base + q[2]],
                        tag: 0,
                    });
                    tris.push(Tri {
                        v: [base + q[0], base + q[2], base + q[3]],
                        tag: 0,
                    });
                }
            }
            let nodes = bvh::build(&verts, &mut tris);
            Boxes { verts, tris, nodes }
        }

        pub fn level(&self) -> Level<'_> {
            Level {
                solid: Bvh {
                    nodes: &self.nodes,
                    tris: &self.tris,
                    verts: &self.verts,
                },
                nav: NavHeader::default(),
                flags: &[],
                floor: &[],
                cover: &[],
                spawns: &[],
            }
        }
    }

    /// A floor, a wall across +X at x = 4, stairs of 0.19 m going up along +Z from z = 2, and a
    /// crate 0.9 m tall at x = -3.
    pub fn yard() -> Boxes {
        let mut boxes = alloc::vec![
            ([-20.0, -1.0, -20.0], [20.0, 0.0, 20.0]),
            ([4.0, 0.0, -20.0], [4.4, 3.0, 20.0]),
            ([-3.6, 0.0, -0.6], [-2.4, 0.9, 0.6])
        ];
        for i in 0..8 {
            boxes.push((
                [-1.0, 0.0, 2.0 + i as f32 * 0.28],
                [1.0, 0.19 * (i + 1) as f32, 2.0 + (i + 1) as f32 * 0.28],
            ));
        }
        Boxes::new(&boxes)
    }

    fn walk(body: &mut Body, level: &Level, velocity: V3, seconds: f32) {
        let mut near = Near::new();
        walk_with(body, level, &mut near, velocity, seconds);
    }

    fn walk_with(body: &mut Body, level: &Level, near: &mut Near, velocity: V3, seconds: f32) {
        for _ in 0..(seconds * 60.0) as u32 {
            body.vel.x = velocity.x;
            body.vel.z = velocity.z;
            if !body.grounded {
                body.vel.y -= 20.6 / 60.0;
            }
            body.advance(level, near, 1.0 / 60.0);
        }
    }

    #[test]
    fn it_stands_walks_and_stops_at_a_wall() {
        let yard = yard();
        let level = yard.level();
        let mut body = Body::new(v3(0.0, 0.5, 0.0), 0.32, 1.78, 0.42);
        walk(&mut body, &level, V3::ZERO, 0.5);
        assert!(
            body.grounded && abs(body.pos.y) < 0.01,
            "it lands on the floor: {:?}",
            body.pos
        );
        walk(&mut body, &level, v3(4.57, 0.0, 0.0), 2.0);
        assert!(
            body.pos.x > 3.6 && body.pos.x < 3.69,
            "the wall holds it a radius away: {}",
            body.pos.x
        );
        assert!(body.grounded && body.blocked);
    }

    #[test]
    fn the_triangles_it_keeps_move_it_as_the_world_asked_each_step_does() {
        let yard = yard();
        let level = yard.level();
        // The same walk twice: once keeping the triangles about it, once asking the tree at
        // every step (a `Near` that says its place holds too many).
        let (mut kept, mut asked) = (Near::new(), Near::new());
        asked.min = [f32::MIN; 3];
        asked.max = [f32::MAX; 3];
        asked.count = Near::HOLDS + 1;
        let (mut a, mut b) = (Body::new(v3(0.0, 0.5, 0.0), 0.32, 1.78, 0.42), Body::new(v3(0.0, 0.5, 0.0), 0.32, 1.78, 0.42));
        // Down onto the floor, into the wall and along it, back, up the stairs, off their side, into the crate.
        for (velocity, seconds) in [(V3::ZERO, 0.5), (v3(4.57, 0.0, 1.0), 2.0), (v3(-4.57, 0.0, 0.3), 0.9), (v3(0.0, 0.0, 4.57), 0.9), (v3(3.0, 0.0, 0.0), 0.8), (v3(-4.57, 0.0, -2.2), 2.5), (v3(-7.0, 0.0, 0.0), 1.0)] {
            walk_with(&mut a, &level, &mut kept, velocity, seconds);
            walk_with(&mut b, &level, &mut asked, velocity, seconds);
            assert!((a.pos - b.pos).len() < 1e-4 && a.grounded == b.grounded && a.blocked == b.blocked, "{:?} and {:?} after {velocity:?}", a.pos, b.pos);
        }
        // The first kept what it met on the way and the second never held a triangle.
        assert!(kept.count > 0 && kept.count <= Near::HOLDS && asked.count == Near::HOLDS + 1);
    }

    #[test]
    fn it_climbs_stairs_and_comes_back_down_them() {
        let yard = yard();
        let level = yard.level();
        let mut body = Body::new(v3(0.0, 0.0, 0.0), 0.32, 1.78, 0.42);
        walk(&mut body, &level, v3(0.0, 0.0, 4.57), 0.9);
        assert!(
            body.grounded && body.pos.y > 1.0,
            "up the flight: {:?}",
            body.pos
        );
        walk(&mut body, &level, v3(0.0, 0.0, -4.57), 0.9);
        assert!(
            body.grounded && abs(body.pos.y) < 0.01,
            "and down again without leaving the steps: {:?}",
            body.pos
        );
    }

    #[test]
    fn a_crate_taller_than_a_step_stops_it() {
        let yard = yard();
        let level = yard.level();
        let mut body = Body::new(v3(0.0, 0.0, 0.0), 0.32, 1.78, 0.42);
        walk(&mut body, &level, v3(-4.57, 0.0, 0.0), 1.5);
        assert!(
            body.pos.x > -2.1 && abs(body.pos.y) < 0.01,
            "{:?}",
            body.pos
        );
    }
}
