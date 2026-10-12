//! Clipping for a rasterizer that has none (the PSP's GE).
//!
//! The GE clips a triangle against the near plane and nothing else. A vertex
//! that lands outside its 4096 × 4096 pixel coordinate space (the guard band,
//! centred on the screen) makes it drop the whole triangle. A large triangle
//! near the eye reaches that far with most of it in view, and the ground
//! disappears under the camera.
//!
//! Only a triangle with a long edge can reach from inside the view to outside
//! the guard band. The world compiler puts each cluster's large triangles at
//! the start of its run. A cluster whose sphere is inside the band
//! (`sphere_safe`, `box_safe`) is drawn as it is. For any other, `classify` decides from
//! each large triangle's clip coordinates whether the GE may draw it; if not,
//! `clip` cuts it against a frustum twice the view's size, well inside the
//! guard band, and the device draws the pieces as float vertices.

use crate::math::V3;

use crate::mat::Mat4;

/// A float vertex in world space, in the GE's component order.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct ClipVertex {
    pub uv: [f32; 2],
    pub color: [u8; 4],
    pub pos: [f32; 3],
}

/// The view a frame clips against.
pub struct Guard {
    /// The `x`, `y` and `w` rows of projection × view: inside the view, `|x| <= w` and `|y| <= w`.
    rows: [[f32; 4]; 3],
    near: f32,
    /// How many screen half-widths and half-heights the guard band reaches, with a margin.
    gx: f32,
    gy: f32,
    /// How far a box reaches along each row for each unit of its half sides (`boxes_along`).
    reach: [[f32; 3]; 3],
    /// How far a sphere reaches along each row for each unit of its radius.
    lengths: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Verdict {
    /// Every vertex the rasterizer ends with is inside the guard band: the triangle's own in
    /// front of the near plane, and the ones its cut at that plane makes.
    Safe,
    /// Outside the view.
    Culled,
    /// In view with a vertex the rasterizer cannot take.
    Clip,
}

/// The frustum the pieces are cut to, in units of the view's.
const CUT: f32 = 2.0;
/// Most vertices a triangle has after five planes.
pub const MAX_POLY: usize = 8;
/// Vertices `clip` writes at most: a fan of `MAX_POLY` corners.
pub const MAX_OUT: usize = (MAX_POLY - 2) * 3;

#[derive(Clone, Copy)]
struct P {
    v: ClipVertex,
    /// Clip x, y, w.
    c: [f32; 3],
}

impl Guard {
    /// `vp` is projection × view; `width` and `height` are the screen's pixels and
    /// `band` the guard band's half-size in pixels (2048 on the PSP).
    pub fn new(vp: &Mat4, near: f32, width: f32, height: f32, band: f32) -> Guard {
        let row = |i: usize| [vp[i * 4], vp[i * 4 + 1], vp[i * 4 + 2], vp[i * 4 + 3]];
        Guard {
            rows: [row(0), row(1), row(3)],
            near: near * 1.02,
            gx: band / (width * 0.5) * 0.9,
            gy: band / (height * 0.5) * 0.9,
            reach: [row(0), row(1), row(3)].map(|r| [r[0].abs(), r[1].abs(), r[2].abs()]),
            lengths: [row(0), row(1), row(3)]
                .map(|r| crate::math::sqrt(r[0] * r[0] + r[1] * r[1] + r[2] * r[2])),
        }
    }

    /// Says which way the boxes `box_safe` is asked about lie: along `x`, up and `z`.
    pub fn boxes_along(mut self, x: &[f32; 3], z: &[f32; 3]) -> Guard {
        use crate::math::abs;
        self.reach = self.rows.map(|r| {
            [
                abs(r[0] * x[0] + r[1] * x[1] + r[2] * x[2]),
                abs(r[1]),
                abs(r[0] * z[0] + r[1] * z[1] + r[2] * z[2]),
            ]
        });
        self
    }

    /// The pass's near distance, with the margin `classify` uses.
    pub fn near(&self) -> f32 {
        self.near
    }

    /// Whether everything inside a sphere is in front of the near plane and inside the guard
    /// band: the rasterizer takes any triangle in it.
    pub fn sphere_safe(&self, centre: V3, radius: f32) -> bool {
        let [x, y, w] = self.to_clip(centre);
        let near = w - radius * self.lengths[2];
        near >= self.near
            && crate::math::abs(x) + radius * self.lengths[0] <= near * self.gx
            && crate::math::abs(y) + radius * self.lengths[1] <= near * self.gy
    }

    /// The same for a box about `centre` with sides twice `half`, lying as `boxes_along` said.
    pub fn box_safe(&self, centre: V3, half: &[f32; 3]) -> bool {
        use crate::math::abs;
        let [x, y, w] = self.to_clip(centre);
        let reach = |r: &[f32; 3]| r[0] * half[0] + r[1] * half[1] + r[2] * half[2];
        let near = w - reach(&self.reach[2]);
        near >= self.near
            && abs(x) + reach(&self.reach[0]) <= near * self.gx
            && abs(y) + reach(&self.reach[1]) <= near * self.gy
    }

    /// This guard for positions stored as steps of `quantum` from `centre`: `to_clip` then takes
    /// the stored numbers as they are.
    pub fn stored(&self, centre: [f32; 3], quantum: f32) -> Guard {
        let row = |r: &[f32; 4]| {
            [
                r[0] * quantum,
                r[1] * quantum,
                r[2] * quantum,
                r[0] * centre[0] + r[1] * centre[1] + r[2] * centre[2] + r[3],
            ]
        };
        Guard {
            rows: [row(&self.rows[0]), row(&self.rows[1]), row(&self.rows[2])],
            near: self.near,
            gx: self.gx,
            gy: self.gy,
            reach: self.reach,
            lengths: self.lengths,
        }
    }

    #[inline]
    pub fn to_clip(&self, p: V3) -> [f32; 3] {
        let r = &self.rows;
        [
            r[0][0] * p.x + r[0][1] * p.y + r[0][2] * p.z + r[0][3],
            r[1][0] * p.x + r[1][1] * p.y + r[1][2] * p.z + r[1][3],
            r[2][0] * p.x + r[2][1] * p.y + r[2][2] * p.z + r[2][3],
        ]
    }

    #[inline]
    pub fn classify(&self, c: &[[f32; 3]; 3]) -> Verdict {
        let (mut safe, mut behind) = (true, 0);
        // Bits: left, right, bottom, top, near.
        let mut all = 0b11111u32;
        for [x, y, w] in c {
            let mut out = 0;
            if *x < -*w {
                out |= 1;
            }
            if *x > *w {
                out |= 2;
            }
            if *y < -*w {
                out |= 4;
            }
            if *y > *w {
                out |= 8;
            }
            if *w < self.near {
                out |= 16;
            }
            all &= out;
            if *w < self.near {
                behind += 1;
            } else if *x < -*w * self.gx
                || *x > *w * self.gx
                || *y < -*w * self.gy
                || *y > *w * self.gy
            {
                safe = false;
            }
        }
        if all != 0 {
            return Verdict::Culled;
        }
        if safe && behind > 0 {
            // The rasterizer cuts at its near plane itself. What it then draws has a vertex where
            // each edge crosses that plane, and those must be inside the band too: the ground
            // under a standing eye crosses it below the band, a wall ahead crosses it inside.
            let (lx, ly) = (self.near * self.gx, self.near * self.gy);
            for i in 0..3 {
                let (p, q) = (&c[i], &c[(i + 1) % 3]);
                if (p[2] < self.near) != (q[2] < self.near) {
                    let t = (self.near - p[2]) / (q[2] - p[2]);
                    let (x, y) = (p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t);
                    if x < -lx || x > lx || y < -ly || y > ly {
                        safe = false;
                    }
                }
            }
        }
        if safe {
            Verdict::Safe
        } else {
            Verdict::Clip
        }
    }

    /// Cuts a triangle to the near plane and the enlarged frustum and writes the
    /// pieces as a triangle list. Returns the number of vertices written.
    pub fn clip(&self, tri: &[ClipVertex; 3], c: &[[f32; 3]; 3], out: &mut [ClipVertex]) -> usize {
        use core::mem::MaybeUninit;
        // Two polygons, one read while the other is written; only the corners in use hold anything.
        let mut store: [MaybeUninit<[P; MAX_POLY]>; 2] = [MaybeUninit::uninit(), MaybeUninit::uninit()];
        let (mut from, mut to) = (store[0].as_mut_ptr() as *mut P, store[1].as_mut_ptr() as *mut P);
        let mut n = 3;
        // SAFETY: `from` and `to` each point at room for `MAX_POLY` corners; `n` and `m` stay at or
        // under `MAX_POLY`, and a corner is read only after it was written.
        unsafe {
            for i in 0..3 {
                from.add(i).write(P { v: tri[i], c: c[i] });
            }
            let near = self.near;
            // How far inside one of the five planes a vertex is.
            #[inline(always)]
            fn inside(plane: usize, c: &[f32; 3], near: f32) -> f32 {
                match plane {
                    0 => c[2] - near,
                    1 => c[0] + c[2] * CUT,
                    2 => c[2] * CUT - c[0],
                    3 => c[1] + c[2] * CUT,
                    _ => c[2] * CUT - c[1],
                }
            }
            for plane in 0..5 {
                // Most triangles cross one plane or two: the others cost a look at each corner.
                let mut d = [0.0f32; MAX_POLY];
                let (mut crossed, mut kept) = (false, false);
                for i in 0..n {
                    d[i] = inside(plane, &(*from.add(i)).c, near);
                    crossed |= d[i] < 0.0;
                    kept |= d[i] >= 0.0;
                }
                if !kept {
                    return 0;
                }
                if !crossed {
                    continue;
                }
                let mut m = 0;
                for i in 0..n {
                    let j = if i + 1 == n { 0 } else { i + 1 };
                    let (p, q) = (&*from.add(i), &*from.add(j));
                    let (dp, dq) = (d[i], d[j]);
                    if dp >= 0.0 && m < MAX_POLY {
                        to.add(m).write(*p);
                        m += 1;
                    }
                    if (dp >= 0.0) != (dq >= 0.0) && m < MAX_POLY {
                        let t = dp / (dp - dq);
                        let l = |x: f32, y: f32| x + (y - x) * t;
                        // The colour's four parts in whole numbers: 256 steps of the way.
                        let step = (t * 256.0) as i32;
                        let part = |k: usize| (p.v.color[k] as i32 + (((q.v.color[k] as i32 - p.v.color[k] as i32) * step) >> 8)) as u8;
                        to.add(m).write(P {
                            v: ClipVertex {
                                uv: [l(p.v.uv[0], q.v.uv[0]), l(p.v.uv[1], q.v.uv[1])],
                                color: [part(0), part(1), part(2), part(3)],
                                pos: [l(p.v.pos[0], q.v.pos[0]), l(p.v.pos[1], q.v.pos[1]), l(p.v.pos[2], q.v.pos[2])],
                            },
                            c: [l(p.c[0], q.c[0]), l(p.c[1], q.c[1]), l(p.c[2], q.c[2])],
                        });
                        m += 1;
                    }
                }
                core::mem::swap(&mut from, &mut to);
                n = m;
                if n < 3 {
                    return 0;
                }
            }
            let mut k = 0;
            for i in 1..n - 1 {
                if k + 3 > out.len() {
                    break;
                }
                out[k] = (*from).v;
                out[k + 1] = (*from.add(i)).v;
                out[k + 2] = (*from.add(i + 1)).v;
                k += 3;
            }
            k
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mat;
    use crate::math::v3;

    fn guard() -> Guard {
        let vp = mat::mul(
            &mat::perspective(62.0, 480.0 / 272.0, 0.4, 2000.0),
            &mat::view(v3(0.0, 1.6, 0.0), v3(0.0, 0.0, -1.0), 0.0),
        );
        Guard::new(&vp, 0.4, 480.0, 272.0, 2048.0)
    }

    fn tri(g: &Guard, p: [V3; 3]) -> ([ClipVertex; 3], [[f32; 3]; 3]) {
        (
            p.map(|p| ClipVertex {
                uv: [p.x, p.z],
                color: [255; 4],
                pos: [p.x, p.y, p.z],
            }),
            p.map(|p| g.to_clip(p)),
        )
    }

    #[test]
    fn a_small_triangle_ahead_is_safe() {
        let g = guard();
        let (_, c) = tri(
            &g,
            [
                v3(-1.0, 0.0, -10.0),
                v3(1.0, 0.0, -10.0),
                v3(0.0, 2.0, -10.0),
            ],
        );
        assert_eq!(g.classify(&c), Verdict::Safe);
    }

    #[test]
    fn a_wall_that_passes_the_eye_is_left_to_the_rasterizer_and_the_ground_under_it_is_not() {
        let g = guard();
        // A wall face 0.6 m to the right, from 3 m behind the eye to 6 m ahead of it: it crosses
        // the near plane 0.6 m from the view's middle, inside the band.
        let (_, wall) = tri(
            &g,
            [
                v3(0.6, 0.0, 3.0),
                v3(0.6, 3.0, -6.0),
                v3(0.6, 0.0, -6.0),
            ],
        );
        assert_eq!(g.classify(&wall), Verdict::Safe);
        // The ground 12 m wide, from behind the eye to far ahead: it crosses the near plane
        // 6 m to the side, and 0.4 m ahead the band ends well short of that.
        let (_, ground) = tri(
            &g,
            [
                v3(-6.0, 0.0, 3.0),
                v3(6.0, 0.0, 3.0),
                v3(0.0, 0.0, -30.0),
            ],
        );
        assert_eq!(g.classify(&ground), Verdict::Clip);
    }

    #[test]
    fn a_triangle_behind_is_culled() {
        let g = guard();
        let (_, c) = tri(
            &g,
            [v3(-1.0, 0.0, 10.0), v3(1.0, 0.0, 10.0), v3(0.0, 2.0, 10.0)],
        );
        assert_eq!(g.classify(&c), Verdict::Culled);
    }

    #[test]
    fn the_ground_under_the_eye_is_clipped_into_the_guard_band() {
        let g = guard();
        // A 64 m ground quad's triangle with the eye standing on it.
        let (v, c) = tri(
            &g,
            [
                v3(-32.0, 0.0, 32.0),
                v3(32.0, 0.0, 32.0),
                v3(-32.0, 0.0, -32.0),
            ],
        );
        assert_eq!(g.classify(&c), Verdict::Clip);
        let mut out = [ClipVertex::default(); MAX_OUT];
        let n = g.clip(&v, &c, &mut out);
        assert!(n >= 3 && n % 3 == 0);
        for p in &out[..n] {
            let [x, y, w] = g.to_clip(v3(p.pos[0], p.pos[1], p.pos[2]));
            assert!(w >= 0.4, "in front of the near plane: {w}");
            assert!(
                x.abs() <= w * 2.01 && y.abs() <= w * 2.01,
                "inside the cut frustum: {x} {y} {w}"
            );
            // The attributes follow the position: uv was set to (x, z).
            assert!((p.uv[0] - p.pos[0]).abs() < 1e-3 && (p.uv[1] - p.pos[2]).abs() < 1e-3);
        }
    }
}
