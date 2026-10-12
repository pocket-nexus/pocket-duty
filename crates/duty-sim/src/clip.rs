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
    /// How far a box along the world's axes reaches along each row for each unit of its half sides.
    axes: [[f32; 3]; 3],
    /// A box whose diagonal, squared, is under this share of the square of its distance from
    /// the eye cannot hold both a point in the view and a point outside the band (`far_safe`).
    spread: f32,
    /// The square of the distance from the eye past which everything inside the band is also in
    /// front of the near plane.
    clear: f32,
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

/// A vertex as the guard sees it: its clip coordinates, and which limits it is outside of.
#[derive(Clone, Copy, Default, Debug)]
pub struct Corner {
    pub clip: [f32; 3],
    /// Bit 0: behind the near plane. Bits 1 to 4: outside the band to the left, right, bottom,
    /// top. Bits 5 to 8: outside the view on those sides.
    pub out: u16,
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
        use crate::math::{atan2, min, sin, sqrt};
        let row = |i: usize| [vp[i * 4], vp[i * 4 + 1], vp[i * 4 + 2], vp[i * 4 + 3]];
        let lengths =
            [row(0), row(1), row(3)].map(|r| sqrt(r[0] * r[0] + r[1] * r[1] + r[2] * r[2]));
        let (gx, gy) = (band / (width * 0.5) * 0.9, band / (height * 0.5) * 0.9);
        // The angle from the view's middle to its corner, and to the nearest point outside the
        // band. Two points at least `d` from the eye and that angle apart are at least
        // `2 d sin(angle / 2)` from each other.
        let (lx, ly, lw) = (lengths[0].max(1e-6), lengths[1].max(1e-6), lengths[2]);
        let corner = atan2(sqrt(lw * lw / (lx * lx) + lw * lw / (ly * ly)), 1.0);
        let outside = atan2(min(gx * lw / lx, gy * lw / ly), 1.0);
        let chord = 2.0 * sin((outside - corner).max(0.0) * 0.5);
        let axes = [row(0), row(1), row(3)].map(|r| [r[0].abs(), r[1].abs(), r[2].abs()]);
        Guard {
            rows: [row(0), row(1), row(3)],
            near: near * 1.02,
            gx,
            gy,
            reach: axes,
            lengths,
            axes,
            // (a tenth less, for the rounding of the angles)
            spread: chord * chord * 0.9,
            clear: {
                let (tx, ty) = (gx * lw / lx, gy * lw / ly);
                near * near * 1.1 * (1.0 + tx * tx + ty * ty)
            },
        }
    }

    /// The guard of a rasterizer that cuts a triangle at every side of its view (the PICA200):
    /// everything is its to draw, and `view::select` hands no cluster over to be tested.
    pub fn open() -> Guard {
        Guard { rows: [[0.0; 4]; 3], near: 0.0, gx: 0.0, gy: 0.0, reach: [[0.0; 3]; 3], lengths: [0.0; 3], axes: [[0.0; 3]; 3], spread: f32::MAX, clear: 0.0 }
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

    /// The share of its distance from the eye that a triangle's longest edge must pass before the
    /// triangle can reach from the view to outside the band.
    pub fn chord(&self) -> f32 {
        crate::math::sqrt(self.spread)
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
        self.clip_box_safe(&self.to_clip(centre), half, &self.reach)
    }

    fn clip_box_safe(&self, clip: &[f32; 3], half: &[f32; 3], reach: &[[f32; 3]; 3]) -> bool {
        use crate::math::abs;
        let [x, y, w] = *clip;
        let along = |r: &[f32; 3]| r[0] * half[0] + r[1] * half[1] + r[2] * half[2];
        let near = w - along(&reach[2]);
        near >= self.near
            && abs(x) + along(&reach[0]) <= near * self.gx
            && abs(y) + along(&reach[1]) <= near * self.gy
    }

    /// Whether a cluster's triangles are all the rasterizer's to draw: its sphere is inside the
    /// band, or its box is. `half` is asked for only when the sphere is not.
    #[inline]
    pub fn cluster_safe(&self, centre: V3, radius: f32, half: impl FnOnce() -> [f32; 3]) -> bool {
        use crate::math::abs;
        let clip = self.to_clip(centre);
        let near = clip[2] - radius * self.lengths[2];
        (near >= self.near
            && abs(clip[0]) + radius * self.lengths[0] <= near * self.gx
            && abs(clip[1]) + radius * self.lengths[1] <= near * self.gy)
            || self.clip_box_safe(&clip, &half(), &self.reach)
    }

    /// Whether everything in a box along the world's axes is the rasterizer's to draw.
    /// `distance2` is the square of the box's distance from the eye: a box far off for its size
    /// is not asked about the band at all.
    #[inline]
    pub fn far_safe(&self, min: &[f32; 3], max: &[f32; 3], distance2: f32) -> bool {
        let side = [max[0] - min[0], max[1] - min[1], max[2] - min[2]];
        // Too small for its distance to reach from the view to the band's edge: when any of it
        // is in view all of it is inside the band, and that far off in front of the near plane.
        if distance2 >= self.clear
            && side[0] * side[0] + side[1] * side[1] + side[2] * side[2] < self.spread * distance2
        {
            return true;
        }
        let centre = V3 {
            x: (min[0] + max[0]) * 0.5,
            y: (min[1] + max[1]) * 0.5,
            z: (min[2] + max[2]) * 0.5,
        };
        let half = [side[0] * 0.5, side[1] * 0.5, side[2] * 0.5];
        self.clip_box_safe(&self.to_clip(centre), &half, &self.axes)
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
            axes: self.axes,
            spread: self.spread,
            clear: self.clear,
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

    /// A vertex's clip coordinates with what `judge` needs to know of them. Triangles share
    /// vertices: the caller keeps a vertex's corner and hands it to each triangle that has it.
    #[inline]
    pub fn corner(&self, p: V3) -> Corner {
        let [x, y, w] = self.to_clip(p);
        let (bx, by) = (w * self.gx, w * self.gy);
        let out = (w < self.near) as u16
            | ((x < -bx) as u16) << 1
            | ((x > bx) as u16) << 2
            | ((y < -by) as u16) << 3
            | ((y > by) as u16) << 4
            | ((x < -w) as u16) << 5
            | ((x > w) as u16) << 6
            | ((y < -w) as u16) << 7
            | ((y > w) as u16) << 8;
        Corner { clip: [x, y, w], out }
    }

    /// `classify` from three corners. A triangle with every corner in front of the near plane and
    /// inside the band is `Safe` without another look, in view or not: the rasterizer may have it.
    #[inline]
    pub fn judge(&self, a: &Corner, b: &Corner, c: &Corner) -> Verdict {
        if (a.out | b.out | c.out) & 0b11111 == 0 {
            return Verdict::Safe;
        }
        if a.out & b.out & c.out & 0b1_1110_0001 != 0 {
            return Verdict::Culled;
        }
        self.classify(&[a.clip, b.clip, c.clip])
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
        let mut corners = [ClipVertex::default(); MAX_POLY];
        let n = self.cut(tri, c, &mut corners);
        let mut k = 0;
        for i in 1..n.max(1) - 1 {
            if k + 3 > out.len() {
                break;
            }
            out[k] = corners[0];
            out[k + 1] = corners[i];
            out[k + 2] = corners[i + 1];
            k += 3;
        }
        k
    }

    /// Cuts a triangle to the near plane and the enlarged frustum. Writes the corners of what is
    /// left, in order (a fan from the first is the piece), and returns how many: 0 when nothing
    /// is left, at most `MAX_POLY`.
    ///
    /// Every side the triangle leaves the enlarged frustum on is cut, though the rasterizer
    /// could take a piece that reaches to the band: it shades a triangle by the screen, not by
    /// depth, and over a piece many screens wide the baked light slides off its place.
    pub fn cut(&self, tri: &[ClipVertex; 3], c: &[[f32; 3]; 3], out: &mut [ClipVertex; MAX_POLY]) -> usize {
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
            for plane in 0..5 {
                // How far inside the plane each corner is. Most triangles cross one plane or two:
                // the others cost a look at each corner.
                let mut d = [0.0f32; MAX_POLY];
                let (mut crossed, mut kept) = (false, false);
                for i in 0..n {
                    let c = &(*from.add(i)).c;
                    let inside = match plane {
                        0 => c[2] - near,
                        1 => c[0] + c[2] * CUT,
                        2 => c[2] * CUT - c[0],
                        3 => c[1] + c[2] * CUT,
                        _ => c[2] * CUT - c[1],
                    };
                    d[i] = inside;
                    crossed |= inside < 0.0;
                    kept |= inside >= 0.0;
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
            for i in 0..n {
                out[i] = (*from.add(i)).v;
            }
            n
        }
    }

    /// Whether a triangle is seen from behind: the sign of its area on the screen, which the
    /// clip coordinates give without the divide, and so for corners behind the eye too.
    #[inline]
    pub fn turned(c: &[[f32; 3]; 3]) -> bool {
        let [a, b, d] = c;
        a[0] * (b[1] * d[2] - d[1] * b[2]) - a[1] * (b[0] * d[2] - d[0] * b[2]) + a[2] * (b[0] * d[1] - d[0] * b[1]) < 0.0
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

    #[test]
    fn a_box_far_safe_passes_holds_nothing_the_rasterizer_drops_when_any_of_it_is_in_view() {
        use crate::rng::Rng;
        let mut rng = Rng::new(7);
        let (mut passed, mut by_size) = (0, 0);
        for _ in 0..4000 {
            let eye = v3(rng.range(-20.0, 20.0), rng.range(0.5, 6.0), rng.range(-20.0, 20.0));
            let (yaw, pitch) = (rng.range(-3.1, 3.1), rng.range(-1.2, 1.2));
            let look = crate::math::forward(yaw, pitch);
            // (the widest view a sprint gives, and the narrowest of the sights)
            let fov = rng.range(40.0, 74.0);
            let vp = mat::mul(&mat::perspective(fov, 480.0 / 272.0, 0.15, 420.0), &mat::view(eye, look, 0.0));
            let g = Guard::new(&vp, 0.15, 480.0, 272.0, 2048.0);
            let centre = eye + v3(rng.range(-30.0, 30.0), rng.range(-6.0, 8.0), rng.range(-30.0, 30.0));
            let half = [rng.range(0.2, 9.0), rng.range(0.2, 5.0), rng.range(0.2, 9.0)];
            let (min, max) = ([centre.x - half[0], centre.y - half[1], centre.z - half[2]], [centre.x + half[0], centre.y + half[1], centre.z + half[2]]);
            let distance2 = mat::box_distance2(eye, &min, &max);
            if !g.far_safe(&min, &max, distance2) {
                continue;
            }
            passed += 1;
            let side = [max[0] - min[0], max[1] - min[1], max[2] - min[2]];
            by_size += (side[0] * side[0] + side[1] * side[1] + side[2] * side[2] < g.spread * distance2) as u32;
            // Points of the box, its corners among them.
            let (mut in_view, mut dropped) = (false, false);
            for i in 0..5 {
                for j in 0..5 {
                    for k in 0..5 {
                        let p = v3(min[0] + side[0] * i as f32 / 4.0, min[1] + side[1] * j as f32 / 4.0, min[2] + side[2] * k as f32 / 4.0);
                        let [x, y, w] = g.to_clip(p);
                        in_view |= w >= 0.15 && x.abs() <= w && y.abs() <= w;
                        // (the band itself, without the tenth `Guard` keeps clear of it)
                        dropped |= w < 0.15 || x.abs() > w * g.gx / 0.9 || y.abs() > w * g.gy / 0.9;
                    }
                }
            }
            assert!(!(in_view && dropped), "eye {eye:?} look {look:?} fov {fov} box {min:?} {max:?}");
        }
        // The test asked both ways of passing.
        assert!(passed > 400 && by_size > 100 && by_size < passed, "{passed} {by_size}");
    }

    #[test]
    fn judge_agrees_with_classify_and_what_cut_leaves_is_inside_the_band() {
        use crate::rng::Rng;
        let g = guard();
        let mut rng = Rng::new(11);
        let (mut cut, mut safe, mut quick) = (0, 0, 0);
        for _ in 0..20000 {
            let at = v3(rng.range(-8.0, 8.0), rng.range(-1.0, 4.0), rng.range(-14.0, 6.0));
            let size = rng.range(0.3, 9.0);
            let p = [0, 1, 2].map(|_| at + v3(rng.range(-size, size), rng.range(-size, size), rng.range(-size, size)));
            let (v, c) = tri(&g, p);
            let corners = p.map(|p| g.corner(p));
            let (judged, classified) = (g.judge(&corners[0], &corners[1], &corners[2]), g.classify(&c));
            match judged {
                // (a triangle out of view and inside the band is the rasterizer's either way)
                Verdict::Safe => assert!(classified != Verdict::Clip, "{p:?}"),
                other => assert_eq!(other, classified, "{p:?}"),
            }
            safe += (judged == Verdict::Safe) as u32;
            quick += ((corners[0].out | corners[1].out | corners[2].out) & 31 == 0) as u32;
            if judged != Verdict::Clip {
                continue;
            }
            cut += 1;
            let mut out = [ClipVertex::default(); MAX_POLY];
            let n = g.cut(&v, &c, &mut out);
            assert!(n == 0 || n >= 3);
            for piece in &out[..n] {
                let [x, y, w] = g.to_clip(v3(piece.pos[0], piece.pos[1], piece.pos[2]));
                // In front of the near plane and inside the band, with room for rounding.
                assert!(w >= g.near * 0.999, "{p:?} -> w {w}");
                assert!(x.abs() <= w * g.gx * 1.001 + 1e-3 && y.abs() <= w * g.gy * 1.001 + 1e-3, "{p:?} -> {x} {y} {w}");
            }
        }
        assert!(cut > 1000 && safe > 1000 && quick > 500, "{cut} {safe} {quick}");
    }

    #[test]
    fn a_triangle_is_turned_when_its_corners_go_the_other_way_round_on_the_screen() {
        let g = guard();
        // Counter-clockwise on the screen (x to the right, y up), 5 m ahead.
        let p = [v3(-1.0, 1.0, -5.0), v3(1.0, 1.0, -5.0), v3(0.0, 2.5, -5.0)];
        let (_, c) = tri(&g, p);
        let (_, back) = tri(&g, [p[0], p[2], p[1]]);
        assert_ne!(Guard::turned(&c), Guard::turned(&back));
        // The same answer with a corner behind the eye.
        let q = [v3(-1.0, 1.0, -5.0), v3(1.0, 1.0, -5.0), v3(0.0, 1.2, 3.0)];
        let (_, c2) = tri(&g, q);
        let (_, back2) = tri(&g, [q[0], q[2], q[1]]);
        assert_ne!(Guard::turned(&c2), Guard::turned(&back2));
        assert!(!Guard::turned(&c));
    }
}
