//! 4 × 4 matrices in row-major order: `m[r * 4 + c]`, applied to column vectors.

use crate::math::{cos, sin, sqrt, tan, V3};

pub type Mat4 = [f32; 16];

pub const IDENTITY: Mat4 = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

pub fn mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut o = [0.0f32; 16];
    for r in 0..4 {
        for c in 0..4 {
            o[r * 4 + c] = a[r * 4] * b[c]
                + a[r * 4 + 1] * b[4 + c]
                + a[r * 4 + 2] * b[8 + c]
                + a[r * 4 + 3] * b[12 + c];
        }
    }
    o
}

/// Perspective looking down -Z with clip depth 0 at `near` and `w` at `far`.
pub fn perspective(fov_y_deg: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    let f = 1.0 / tan(fov_y_deg * (core::f32::consts::PI / 180.0) * 0.5);
    let a = far / (near - far);
    [
        f / aspect,
        0.0,
        0.0,
        0.0,
        0.0,
        f,
        0.0,
        0.0,
        0.0,
        0.0,
        a,
        a * near,
        0.0,
        0.0,
        -1.0,
        0.0,
    ]
}

/// The same frustum with clip depth from `-w` at `near` to `w` at `far` (the PSP's GE).
pub fn perspective_gl(fov_y_deg: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    let f = 1.0 / tan(fov_y_deg * (core::f32::consts::PI / 180.0) * 0.5);
    [
        f / aspect,
        0.0,
        0.0,
        0.0,
        0.0,
        f,
        0.0,
        0.0,
        0.0,
        0.0,
        (far + near) / (near - far),
        2.0 * far * near / (near - far),
        0.0,
        0.0,
        -1.0,
        0.0,
    ]
}

/// View matrix for an eye looking along the unit vector `look`, rolled about it.
pub fn view(eye: V3, look: V3, roll: f32) -> Mat4 {
    let right = look.cross(V3::UP).norm_or(V3 {
        x: 1.0,
        y: 0.0,
        z: 0.0,
    });
    let up = right.cross(look);
    let (s, c) = (sin(roll), cos(roll));
    let r = right * c + up * s;
    let u = up * c - right * s;
    [
        r.x,
        r.y,
        r.z,
        -r.dot(eye),
        u.x,
        u.y,
        u.z,
        -u.dot(eye),
        -look.x,
        -look.y,
        -look.z,
        look.dot(eye),
        0.0,
        0.0,
        0.0,
        1.0,
    ]
}

/// `m × T(t) × S(s)`: folds a mesh's dequantization into a matrix.
pub fn with_bounds(m: &Mat4, t: [f32; 3], s: [f32; 3]) -> Mat4 {
    let mut o = [0.0f32; 16];
    for r in 0..4 {
        let (a, b, c, d) = (m[r * 4], m[r * 4 + 1], m[r * 4 + 2], m[r * 4 + 3]);
        o[r * 4] = a * s[0];
        o[r * 4 + 1] = b * s[1];
        o[r * 4 + 2] = c * s[2];
        o[r * 4 + 3] = a * t[0] + b * t[1] + c * t[2] + d;
    }
    o
}

/// `m × T(t)`.
pub fn translated(m: &Mat4, t: V3) -> Mat4 {
    with_bounds(m, [t.x, t.y, t.z], [1.0, 1.0, 1.0])
}

/// Scale and offset that take a mesh's stored `i16` positions (as the device
/// normalizes them: `q / divisor`) to world space.
pub fn dequant(min: &[f32; 3], max: &[f32; 3], divisor: f32) -> ([f32; 3], [f32; 3]) {
    let mut s = [0.0; 3];
    let mut t = [0.0; 3];
    for a in 0..3 {
        let ext = max[a] - min[a];
        s[a] = ext * (divisor / 65535.0);
        t[a] = min[a] + ext * (32768.0 / 65535.0);
    }
    (s, t)
}

/// Six planes `(n, d)` with `n·p + d >= 0` inside, from `perspective × view`.
pub fn planes(m: &Mat4) -> [[f32; 4]; 6] {
    let row = |i: usize| [m[i * 4], m[i * 4 + 1], m[i * 4 + 2], m[i * 4 + 3]];
    let (x, y, z, w) = (row(0), row(1), row(2), row(3));
    let add = |a: [f32; 4], b: [f32; 4]| [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]];
    let sub = |a: [f32; 4], b: [f32; 4]| [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]];
    [add(w, x), sub(w, x), add(w, y), sub(w, y), z, sub(w, z)]
}

/// The same planes with unit normals, so `n·p + d` is a distance.
pub fn unit_planes(planes: &[[f32; 4]; 6]) -> [[f32; 4]; 6] {
    planes.map(|p| {
        let l = sqrt(p[0] * p[0] + p[1] * p[1] + p[2] * p[2]);
        if l > 1e-12 {
            [p[0] / l, p[1] / l, p[2] / l, p[3] / l]
        } else {
            p
        }
    })
}

/// Whether a sphere touches the frustum; `planes` have unit normals.
pub fn sphere_visible(planes: &[[f32; 4]; 6], centre: &[f32; 3], radius: f32) -> bool {
    for p in planes {
        if p[0] * centre[0] + p[1] * centre[1] + p[2] * centre[2] + p[3] < -radius {
            return false;
        }
    }
    true
}

/// How far a box reaches along each plane's normal for each unit of its half sides, the box
/// lying along `x`, up and `z`: `reach[plane][side]`.
pub fn box_reach(planes: &[[f32; 4]; 6], x: &[f32; 3], z: &[f32; 3]) -> [[f32; 3]; 6] {
    core::array::from_fn(|i| {
        let p = &planes[i];
        [
            crate::math::abs(p[0] * x[0] + p[1] * x[1] + p[2] * x[2]),
            crate::math::abs(p[1]),
            crate::math::abs(p[0] * z[0] + p[1] * z[1] + p[2] * z[2]),
        ]
    })
}

/// Whether a box about `centre` with sides twice `half`, inside a sphere of `radius` about the
/// same point, touches the frustum; `planes` have unit normals and `reach` is `box_reach` of
/// them. The box is asked only about the planes the sphere reaches across.
pub fn cluster_visible(
    planes: &[[f32; 4]; 6],
    reach: &[[f32; 3]; 6],
    centre: &[f32; 3],
    radius: f32,
    half: &[f32; 3],
) -> bool {
    for (p, r) in planes.iter().zip(reach) {
        let d = p[0] * centre[0] + p[1] * centre[1] + p[2] * centre[2] + p[3];
        if d < -radius {
            return false;
        }
        if d < 0.0 && d + r[0] * half[0] + r[1] * half[1] + r[2] * half[2] < 0.0 {
            return false;
        }
    }
    true
}

/// Whether a box touches the frustum (conservative).
pub fn visible(planes: &[[f32; 4]; 6], min: &[f32; 3], max: &[f32; 3]) -> bool {
    for p in planes {
        let x = if p[0] >= 0.0 { max[0] } else { min[0] };
        let y = if p[1] >= 0.0 { max[1] } else { min[1] };
        let z = if p[2] >= 0.0 { max[2] } else { min[2] };
        if p[0] * x + p[1] * y + p[2] * z + p[3] < 0.0 {
            return false;
        }
    }
    true
}

/// Whether a box is wholly inside the frustum.
pub fn contained(planes: &[[f32; 4]; 6], min: &[f32; 3], max: &[f32; 3]) -> bool {
    for p in planes {
        // The corner farthest out along this plane.
        let x = if p[0] >= 0.0 { min[0] } else { max[0] };
        let y = if p[1] >= 0.0 { min[1] } else { max[1] };
        let z = if p[2] >= 0.0 { min[2] } else { max[2] };
        if p[0] * x + p[1] * y + p[2] * z + p[3] < 0.0 {
            return false;
        }
    }
    true
}

/// For each plane and axis, 1 where the plane's normal points up the axis: which of a box's
/// two corners along that axis is the one farthest inside the plane.
pub fn plane_signs(planes: &[[f32; 4]; 6]) -> [[usize; 3]; 6] {
    planes.map(|p| [(p[0] >= 0.0) as usize, (p[1] >= 0.0) as usize, (p[2] >= 0.0) as usize])
}

/// Which of the first `count` planes a box reaches across, one bit a plane; `None` when the box
/// is wholly outside one of them. `bounds` is the box's least corner, then its greatest;
/// `signs` is `plane_signs` of the planes.
#[inline]
pub fn box_across(
    planes: &[[f32; 4]; 6],
    signs: &[[usize; 3]; 6],
    count: usize,
    bounds: &[[f32; 3]; 2],
) -> Option<u32> {
    let mut across = 0;
    for k in 0..count {
        let (p, s) = (&planes[k], &signs[k]);
        // The corner farthest inside this plane.
        if p[0] * bounds[s[0]][0] + p[1] * bounds[s[1]][1] + p[2] * bounds[s[2]][2] + p[3] < 0.0 {
            return None;
        }
        // The corner farthest out.
        if p[0] * bounds[1 - s[0]][0] + p[1] * bounds[1 - s[1]][1] + p[2] * bounds[1 - s[2]][2] + p[3]
            < 0.0
        {
            across |= 1 << k;
        }
    }
    Some(across)
}

/// The square of the distance from `p` to a box (0 inside).
#[inline]
pub fn box_distance2(p: V3, min: &[f32; 3], max: &[f32; 3]) -> f32 {
    let d = |v: f32, lo: f32, hi: f32| {
        if v < lo {
            lo - v
        } else if v > hi {
            v - hi
        } else {
            0.0
        }
    };
    let (x, y, z) = (
        d(p.x, min[0], max[0]),
        d(p.y, min[1], max[1]),
        d(p.z, min[2], max[2]),
    );
    x * x + y * y + z * z
}

/// Distance from `p` to a box (0 inside).
pub fn box_distance(p: V3, min: &[f32; 3], max: &[f32; 3]) -> f32 {
    let d = |v: f32, lo: f32, hi: f32| {
        if v < lo {
            lo - v
        } else if v > hi {
            v - hi
        } else {
            0.0
        }
    };
    let (x, y, z) = (
        d(p.x, min[0], max[0]),
        d(p.y, min[1], max[1]),
        d(p.z, min[2], max[2]),
    );
    sqrt(x * x + y * y + z * z)
}

/// Projects a world point to pixels of a `w × h` screen; `None` behind the eye.
pub fn project(m: &Mat4, p: V3, w: f32, h: f32) -> Option<(f32, f32)> {
    let cw = m[12] * p.x + m[13] * p.y + m[14] * p.z + m[15];
    if cw <= 0.01 {
        return None;
    }
    let x = (m[0] * p.x + m[1] * p.y + m[2] * p.z + m[3]) / cw;
    let y = (m[4] * p.x + m[5] * p.y + m[6] * p.z + m[7]) / cw;
    Some(((x * 0.5 + 0.5) * w, (0.5 - y * 0.5) * h))
}
