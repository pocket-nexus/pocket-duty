//! A bounding volume hierarchy over triangles: rays, and the triangles near a
//! capsule.
//!
//! The compiler builds it (`build`) twice: over every triangle the reference
//! draws, to cast the light it bakes, and over the reference's collision
//! triangles, which go into the pack with their nodes. A device reads those
//! in place: `Bvh` borrows three slices and owns nothing.

use alloc::vec::Vec;

use crate::math::*;

/// A box and what is in it. With `count > 0` it is a leaf: `count` triangles
/// from `first`. Otherwise its two children are nodes `first` and `first + 1`.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Node {
    pub min: [f32; 3],
    pub first: u32,
    pub max: [f32; 3],
    pub count: u32,
}

/// Three vertex numbers and a tag the tree carries without reading: its low
/// byte is the surface, the bits above it are the caller's.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Tri {
    pub v: [u32; 3],
    pub tag: u32,
}

#[derive(Clone, Copy)]
pub struct Bvh<'a> {
    pub nodes: &'a [Node],
    pub tris: &'a [Tri],
    pub verts: &'a [[f32; 3]],
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hit {
    /// Distance along the ray.
    pub t: f32,
    pub tri: u32,
    pub tag: u32,
    /// The triangle's unit normal, turned to face the ray's origin.
    pub n: V3,
}

/// Triangles in a leaf at most.
const LEAF: usize = 4;
const BINS: usize = 16;

fn grow(min: &mut [f32; 3], max: &mut [f32; 3], p: &[f32; 3]) {
    for a in 0..3 {
        if p[a] < min[a] {
            min[a] = p[a];
        }
        if p[a] > max[a] {
            max[a] = p[a];
        }
    }
}

fn half_area(min: &[f32; 3], max: &[f32; 3]) -> f32 {
    let (x, y, z) = (max[0] - min[0], max[1] - min[1], max[2] - min[2]);
    if x < 0.0 {
        0.0
    } else {
        x * y + y * z + z * x
    }
}

const EMPTY: ([f32; 3], [f32; 3]) = ([f32::MAX; 3], [f32::MIN; 3]);

/// Builds the tree over `tris`, which it reorders: a leaf's triangles are
/// neighbours in the list. Splits by the surface area heuristic over bins.
pub fn build(verts: &[[f32; 3]], tris: &mut [Tri]) -> Vec<Node> {
    let n = tris.len();
    let boxes: Vec<([f32; 3], [f32; 3])> = tris
        .iter()
        .map(|t| {
            let (mut lo, mut hi) = EMPTY;
            for k in 0..3 {
                grow(&mut lo, &mut hi, &verts[t.v[k] as usize]);
            }
            (lo, hi)
        })
        .collect();
    let centre: Vec<[f32; 3]> = boxes.iter().map(|(lo, hi)| [(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5, (lo[2] + hi[2]) * 0.5]).collect();
    let mut order: Vec<u32> = (0..n as u32).collect();
    let mut nodes: Vec<Node> = Vec::with_capacity(n / 2 + 2);
    nodes.push(Node::default());
    // (node, first, count) still to split
    let mut work: Vec<(usize, usize, usize)> = alloc::vec![(0, 0, n)];
    while let Some((at, first, count)) = work.pop() {
        let (mut lo, mut hi) = EMPTY;
        let (mut clo, mut chi) = EMPTY;
        for &i in &order[first..first + count] {
            let b = &boxes[i as usize];
            grow(&mut lo, &mut hi, &b.0);
            grow(&mut lo, &mut hi, &b.1);
            grow(&mut clo, &mut chi, &centre[i as usize]);
        }
        // (a hair wider than what it holds: a ray along a face of the box still enters it)
        for a in 0..3 {
            lo[a] -= 1e-4;
            hi[a] += 1e-4;
        }
        nodes[at].min = lo;
        nodes[at].max = hi;
        let leaf = |nodes: &mut Vec<Node>| {
            nodes[at].first = first as u32;
            nodes[at].count = count as u32;
        };
        if count <= LEAF {
            leaf(&mut nodes);
            continue;
        }
        // The cheapest of the bins' planes on each axis.
        let mut best = (f32::MAX, 0usize, 0usize);
        for axis in 0..3 {
            let extent = chi[axis] - clo[axis];
            if extent < 1e-6 {
                continue;
            }
            let scale = BINS as f32 / extent;
            let mut bins = [(EMPTY, 0u32); BINS];
            for &i in &order[first..first + count] {
                let b = (((centre[i as usize][axis] - clo[axis]) * scale) as usize).min(BINS - 1);
                grow(&mut bins[b].0 .0, &mut bins[b].0 .1, &boxes[i as usize].0);
                grow(&mut bins[b].0 .0, &mut bins[b].0 .1, &boxes[i as usize].1);
                bins[b].1 += 1;
            }
            let mut right = [0.0f32; BINS];
            let (mut acc, mut k) = (EMPTY, 0u32);
            for b in (1..BINS).rev() {
                grow(&mut acc.0, &mut acc.1, &bins[b].0 .0);
                grow(&mut acc.0, &mut acc.1, &bins[b].0 .1);
                k += bins[b].1;
                right[b] = half_area(&acc.0, &acc.1) * k as f32;
            }
            let (mut acc, mut k) = (EMPTY, 0u32);
            for b in 0..BINS - 1 {
                grow(&mut acc.0, &mut acc.1, &bins[b].0 .0);
                grow(&mut acc.0, &mut acc.1, &bins[b].0 .1);
                k += bins[b].1;
                if k == 0 || k as usize == count {
                    continue;
                }
                let cost = half_area(&acc.0, &acc.1) * k as f32 + right[b + 1];
                if cost < best.0 {
                    best = (cost, axis, b + 1);
                }
            }
        }
        if best.0 == f32::MAX {
            // Every centre in one place: halve the list, down to leaves of any size it takes.
            if count <= LEAF * 4 {
                leaf(&mut nodes);
                continue;
            }
            let mid = count / 2;
            let child = nodes.len();
            nodes.push(Node::default());
            nodes.push(Node::default());
            nodes[at].first = child as u32;
            nodes[at].count = 0;
            work.push((child, first, mid));
            work.push((child + 1, first + mid, count - mid));
            continue;
        }
        let (_, axis, plane) = best;
        let scale = BINS as f32 / (chi[axis] - clo[axis]);
        let slice = &mut order[first..first + count];
        let (mut i, mut j) = (0usize, count);
        while i < j {
            let b = (((centre[slice[i] as usize][axis] - clo[axis]) * scale) as usize).min(BINS - 1);
            if b < plane {
                i += 1;
            } else {
                j -= 1;
                slice.swap(i, j);
            }
        }
        let child = nodes.len();
        nodes.push(Node::default());
        nodes.push(Node::default());
        nodes[at].first = child as u32;
        nodes[at].count = 0;
        work.push((child, first, i));
        work.push((child + 1, first + i, count - i));
    }
    let sorted: Vec<Tri> = order.iter().map(|&i| tris[i as usize]).collect();
    tris.copy_from_slice(&sorted);
    nodes
}

/// Where a ray enters a box, if it does before `far`.
#[inline]
fn enter(node: &Node, o: &[f32; 3], inv: &[f32; 3], far: f32) -> Option<f32> {
    let (mut t0, mut t1) = (0.0f32, far);
    for a in 0..3 {
        let (mut near, mut away) = ((node.min[a] - o[a]) * inv[a], (node.max[a] - o[a]) * inv[a]);
        if near > away {
            core::mem::swap(&mut near, &mut away);
        }
        // (a NaN from 0 × ∞ fails both tests and leaves the slab open)
        if near > t0 {
            t0 = near;
        }
        if away < t1 {
            t1 = away;
        }
    }
    (t0 <= t1).then_some(t0)
}

impl<'a> Bvh<'a> {
    #[inline]
    pub fn corner(&self, tri: u32, k: usize) -> V3 {
        let p = &self.verts[self.tris[tri as usize].v[k] as usize];
        v3(p[0], p[1], p[2])
    }

    /// Distance to triangle `i` along the ray, both of its faces counting.
    #[inline]
    fn triangle(&self, i: usize, o: V3, d: V3) -> Option<f32> {
        let t = &self.tris[i];
        let a = self.verts[t.v[0] as usize];
        let b = self.verts[t.v[1] as usize];
        let c = self.verts[t.v[2] as usize];
        let (e1, e2) = (v3(b[0] - a[0], b[1] - a[1], b[2] - a[2]), v3(c[0] - a[0], c[1] - a[1], c[2] - a[2]));
        let p = d.cross(e2);
        let det = e1.dot(p);
        if det > -1e-9 && det < 1e-9 {
            return None;
        }
        let inv = 1.0 / det;
        let s = o - v3(a[0], a[1], a[2]);
        let u = s.dot(p) * inv;
        if !(0.0..=1.0).contains(&u) {
            return None;
        }
        let q = s.cross(e1);
        let v = d.dot(q) * inv;
        if v < 0.0 || u + v > 1.0 {
            return None;
        }
        let t = e2.dot(q) * inv;
        (t > 0.0).then_some(t)
    }

    /// The nearest triangle a ray from `o` along the unit vector `d` meets within `far`.
    /// A triangle whose tag has a bit of `ignore` is not there.
    pub fn raycast(&self, o: V3, d: V3, far: f32, ignore: u32) -> Option<Hit> {
        if self.nodes.is_empty() {
            return None;
        }
        let oa = [o.x, o.y, o.z];
        let inv = [1.0 / d.x, 1.0 / d.y, 1.0 / d.z];
        let mut best = far;
        let mut found = u32::MAX;
        let mut stack = [0u32; 64];
        let mut top = 1usize;
        while top > 0 {
            top -= 1;
            let node = &self.nodes[stack[top] as usize];
            if enter(node, &oa, &inv, best).is_none() {
                continue;
            }
            if node.count > 0 {
                for i in node.first as usize..(node.first + node.count) as usize {
                    if self.tris[i].tag & ignore != 0 {
                        continue;
                    }
                    if let Some(t) = self.triangle(i, o, d) {
                        if t < best {
                            best = t;
                            found = i as u32;
                        }
                    }
                }
            } else if top + 2 <= stack.len() {
                // The nearer child is looked at first: what it holds shortens the ray for the other.
                let (l, r) = (node.first, node.first + 1);
                let (tl, tr) = (enter(&self.nodes[l as usize], &oa, &inv, best), enter(&self.nodes[r as usize], &oa, &inv, best));
                match (tl, tr) {
                    (Some(a), Some(b)) => {
                        let (first, second) = if a <= b { (l, r) } else { (r, l) };
                        stack[top] = second;
                        stack[top + 1] = first;
                        top += 2;
                    }
                    (Some(_), None) => {
                        stack[top] = l;
                        top += 1;
                    }
                    (None, Some(_)) => {
                        stack[top] = r;
                        top += 1;
                    }
                    (None, None) => {}
                }
            }
        }
        if found == u32::MAX {
            return None;
        }
        let (a, b, c) = (self.corner(found, 0), self.corner(found, 1), self.corner(found, 2));
        let mut n = (b - a).cross(c - a).norm_or(V3::UP);
        if n.dot(d) > 0.0 {
            n = -n;
        }
        Some(Hit { t: best, tri: found, tag: self.tris[found as usize].tag, n })
    }

    /// Whether anything stands between `o` and the point `far` along `d`.
    pub fn occluded(&self, o: V3, d: V3, far: f32, ignore: u32) -> bool {
        if self.nodes.is_empty() {
            return false;
        }
        let oa = [o.x, o.y, o.z];
        let inv = [1.0 / d.x, 1.0 / d.y, 1.0 / d.z];
        let mut stack = [0u32; 64];
        let mut top = 1usize;
        while top > 0 {
            top -= 1;
            let node = &self.nodes[stack[top] as usize];
            if enter(node, &oa, &inv, far).is_none() {
                continue;
            }
            if node.count > 0 {
                for i in node.first as usize..(node.first + node.count) as usize {
                    if self.tris[i].tag & ignore != 0 {
                        continue;
                    }
                    if self.triangle(i, o, d).is_some_and(|t| t < far) {
                        return true;
                    }
                }
            } else if top + 2 <= stack.len() {
                stack[top] = node.first;
                stack[top + 1] = node.first + 1;
                top += 2;
            }
        }
        false
    }

    /// Calls `each` with every triangle whose leaf reaches into the box.
    pub fn within(&self, min: &[f32; 3], max: &[f32; 3], mut each: impl FnMut(u32)) {
        if self.nodes.is_empty() {
            return;
        }
        let mut stack = [0u32; 64];
        let mut top = 1usize;
        while top > 0 {
            top -= 1;
            let node = &self.nodes[stack[top] as usize];
            if node.min[0] > max[0] || node.max[0] < min[0] || node.min[1] > max[1] || node.max[1] < min[1] || node.min[2] > max[2] || node.max[2] < min[2] {
                continue;
            }
            if node.count > 0 {
                for i in node.first..node.first + node.count {
                    each(i);
                }
            } else if top + 2 <= stack.len() {
                stack[top] = node.first;
                stack[top + 1] = node.first + 1;
                top += 2;
            }
        }
    }
}

/// The point of triangle `abc` nearest to `p`.
pub fn nearest_on_triangle(p: V3, a: V3, b: V3, c: V3) -> V3 {
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}

/// The nearest points of two segments: on `p1 q1`, then on `p2 q2`.
pub fn nearest_on_segments(p1: V3, q1: V3, p2: V3, q2: V3) -> (V3, V3) {
    let (d1, d2, r) = (q1 - p1, q2 - p2, p1 - p2);
    let (a, e, f) = (d1.len2(), d2.len2(), d2.dot(r));
    let (s, t);
    if a <= 1e-12 && e <= 1e-12 {
        return (p1, p2);
    }
    if a <= 1e-12 {
        s = 0.0;
        t = clamp(f / e, 0.0, 1.0);
    } else {
        let c = d1.dot(r);
        if e <= 1e-12 {
            t = 0.0;
            s = clamp(-c / a, 0.0, 1.0);
        } else {
            let b = d1.dot(d2);
            let denom = a * e - b * b;
            let mut s0 = if denom > 1e-12 { clamp((b * f - c * e) / denom, 0.0, 1.0) } else { 0.0 };
            let mut t0 = (b * s0 + f) / e;
            if t0 < 0.0 {
                t0 = 0.0;
                s0 = clamp(-c / a, 0.0, 1.0);
            } else if t0 > 1.0 {
                t0 = 1.0;
                s0 = clamp((b - c) / a, 0.0, 1.0);
            }
            s = s0;
            t = t0;
        }
    }
    (p1 + d1 * s, p2 + d2 * t)
}

/// The nearest points of the segment `p q` and the triangle `abc`: on the
/// segment, then on the triangle. Where the segment passes through the
/// triangle both are the point where it does.
pub fn nearest_segment_triangle(p: V3, q: V3, a: V3, b: V3, c: V3) -> (V3, V3) {
    let n = (b - a).cross(c - a);
    let (dp, dq) = (n.dot(p - a), n.dot(q - a));
    if (dp > 0.0) != (dq > 0.0) && (dp - dq).abs() > 1e-12 {
        let x = p + (q - p) * (dp / (dp - dq));
        // Inside the triangle when it is on the inner side of all three edges.
        let inside = |u: V3, v: V3| (v - u).cross(x - u).dot(n) >= 0.0;
        if inside(a, b) && inside(b, c) && inside(c, a) {
            return (x, x);
        }
    }
    let mut best = (f32::MAX, p, a);
    let mut offer = |s: V3, t: V3| {
        let d = (s - t).len2();
        if d < best.0 {
            best = (d, s, t);
        }
    };
    offer(p, nearest_on_triangle(p, a, b, c));
    offer(q, nearest_on_triangle(q, a, b, c));
    for (u, v) in [(a, b), (b, c), (c, a)] {
        let (s, t) = nearest_on_segments(p, q, u, v);
        offer(s, t);
    }
    (best.1, best.2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    /// A floor of two triangles at y = 0 and a wall of two at x = 2.
    fn room() -> (Vec<[f32; 3]>, Vec<Tri>) {
        let verts = vec![[-4.0, 0.0, -4.0], [4.0, 0.0, -4.0], [4.0, 0.0, 4.0], [-4.0, 0.0, 4.0], [2.0, 0.0, -4.0], [2.0, 3.0, -4.0], [2.0, 3.0, 4.0], [2.0, 0.0, 4.0]];
        let tris = vec![Tri { v: [0, 2, 1], tag: 1 }, Tri { v: [0, 3, 2], tag: 1 }, Tri { v: [4, 5, 6], tag: 2 }, Tri { v: [4, 6, 7], tag: 2 }];
        (verts, tris)
    }

    #[test]
    fn a_ray_finds_the_nearest_triangle() {
        let (verts, mut tris) = room();
        let nodes = build(&verts, &mut tris);
        let bvh = Bvh { nodes: &nodes, tris: &tris, verts: &verts };
        let down = bvh.raycast(v3(0.0, 2.0, 0.0), v3(0.0, -1.0, 0.0), 10.0, 0).unwrap();
        assert!((down.t - 2.0).abs() < 1e-4 && down.tag == 1 && down.n.y > 0.99);
        let side = bvh.raycast(v3(0.0, 1.0, 0.0), v3(1.0, 0.0, 0.0), 10.0, 0).unwrap();
        assert!((side.t - 2.0).abs() < 1e-4 && side.tag == 2 && side.n.x < -0.99);
        assert!(bvh.raycast(v3(0.0, 1.0, 0.0), v3(-1.0, 0.0, 0.0), 10.0, 0).is_none());
        assert!(bvh.raycast(v3(0.0, 1.0, 0.0), v3(1.0, 0.0, 0.0), 1.5, 0).is_none());
        assert!(bvh.occluded(v3(0.0, 1.0, 0.0), v3(1.0, 0.0, 0.0), 3.0, 0));
        assert!(!bvh.occluded(v3(0.0, 1.0, 0.0), v3(1.0, 0.0, 0.0), 3.0, 2));
    }

    #[test]
    fn many_triangles_agree_with_a_search_of_all_of_them() {
        // A grid of tilted quads, and rays through it from a fixed sequence.
        let mut verts = Vec::new();
        let mut tris = Vec::new();
        for i in 0..40 {
            for j in 0..40 {
                let (x, z, y) = (i as f32 * 1.3, j as f32 * 1.1, ((i * 7 + j * 13) % 11) as f32 * 0.4);
                let base = verts.len() as u32;
                verts.extend_from_slice(&[[x, y, z], [x + 1.0, y + 0.3, z], [x + 1.0, y, z + 1.0], [x, y - 0.2, z + 1.0]]);
                tris.push(Tri { v: [base, base + 1, base + 2], tag: 0 });
                tris.push(Tri { v: [base, base + 2, base + 3], tag: 0 });
            }
        }
        let plain = tris.clone();
        let nodes = build(&verts, &mut tris);
        let bvh = Bvh { nodes: &nodes, tris: &tris, verts: &verts };
        let all = Bvh { nodes: &[], tris: &plain, verts: &verts };
        let mut seed = 12345u32;
        let mut next = || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed >> 8) as f32 / 16777216.0
        };
        let mut hits = 0;
        for _ in 0..400 {
            let o = v3(next() * 52.0, 8.0 + next() * 4.0, next() * 44.0);
            let d = v3(next() - 0.5, -0.3 - next(), next() - 0.5).norm();
            let slow = (0..plain.len()).filter_map(|i| all.triangle(i, o, d)).fold(f32::MAX, f32::min);
            match bvh.raycast(o, d, 100.0, 0) {
                Some(hit) => {
                    hits += 1;
                    assert!((hit.t - slow).abs() < 1e-3, "{} against {}", hit.t, slow);
                }
                None => assert!(slow > 100.0),
            }
        }
        assert!(hits > 200);
    }

    #[test]
    fn a_segment_and_a_triangle() {
        let (a, b, c) = (v3(0.0, 0.0, 0.0), v3(2.0, 0.0, 0.0), v3(0.0, 0.0, 2.0));
        // Above the inside: straight down onto the face.
        let (s, t) = nearest_segment_triangle(v3(0.5, 1.0, 0.5), v3(0.5, 3.0, 0.5), a, b, c);
        assert!((s - v3(0.5, 1.0, 0.5)).len() < 1e-5 && (t - v3(0.5, 0.0, 0.5)).len() < 1e-5);
        // Through it.
        let (s, t) = nearest_segment_triangle(v3(0.5, 1.0, 0.5), v3(0.5, -1.0, 0.5), a, b, c);
        assert!((s - t).len() < 1e-5 && s.y.abs() < 1e-5);
        // Beside an edge.
        let (s, t) = nearest_segment_triangle(v3(-1.0, 0.5, 0.5), v3(-1.0, 2.0, 0.5), a, b, c);
        assert!((s - v3(-1.0, 0.5, 0.5)).len() < 1e-5 && (t - v3(0.0, 0.0, 0.5)).len() < 1e-5);
    }
}
