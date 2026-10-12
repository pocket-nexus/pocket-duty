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
use crate::tune::player::{SLOPE, SNAP};

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

    /// Moves by its velocity for `dt` seconds.
    pub fn advance(&mut self, level: &Level, dt: f32) {
        self.landed = 0.0;
        self.blocked = false;
        let pieces = ((self.vel.len() * dt / (self.radius * 0.35)) as u32 + 1).min(6);
        let h = dt / pieces as f32;
        for _ in 0..pieces {
            self.pos += self.vel * h;
            self.push_out(level);
            self.find_ground(level, h);
        }
    }

    /// Out of every wall and ceiling the capsule is in, the deepest first.
    pub fn push_out(&mut self, level: &Level) {
        let r = self.radius;
        for _ in 0..4 {
            let (a, b) = self.axis();
            let (min, max) = ([a.x - r, a.y - r, a.z - r], [b.x + r, b.y + r, b.z + r]);
            let mut deepest = (0.0f32, V3::ZERO);
            level.solid.within(&min, &max, |tri| {
                let (p, q, s) = (
                    level.solid.corner(tri, 0),
                    level.solid.corner(tri, 1),
                    level.solid.corner(tri, 2),
                );
                let (on_axis, on_triangle) = nearest_segment_triangle(a, b, p, q, s);
                let d = on_axis - on_triangle;
                let dist2 = d.len2();
                if dist2 >= r * r {
                    return;
                }
                let dist = sqrt(dist2);
                let n = if dist > 1e-4 {
                    d * (1.0 / dist)
                } else {
                    // The axis is in the triangle: out the way the triangle faces the capsule's middle.
                    let n = (q - p).cross(s - p).norm_or(V3::UP);
                    if n.dot((a + b) * 0.5 - p) < 0.0 {
                        -n
                    } else {
                        n
                    }
                };
                if r - dist > deepest.0 {
                    deepest = (r - dist, n);
                }
            });
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
    fn find_ground(&mut self, level: &Level, h: f32) {
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
        for (dx, dz) in [(0.0, 0.0), (k, 0.0), (-k, 0.0), (0.0, k), (0.0, -k)] {
            if let Some(hit) = level.solid.raycast(
                v3(self.pos.x + dx, top, self.pos.z + dz),
                v3(0.0, -1.0, 0.0),
                self.step + 0.05 + reach,
                0,
            ) {
                let y = top - hit.t;
                if hit.n.y >= SLOPE && best.is_none_or(|b| y > b.0) {
                    best = Some((y, hit.tag));
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
        for _ in 0..(seconds * 60.0) as u32 {
            body.vel.x = velocity.x;
            body.vel.z = velocity.z;
            if !body.grounded {
                body.vel.y -= 20.6 / 60.0;
            }
            body.advance(level, 1.0 / 60.0);
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
