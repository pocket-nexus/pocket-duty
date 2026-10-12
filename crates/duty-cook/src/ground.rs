//! The ground's shadow picture.
//!
//! The street's shadows are the picture's largest shapes, and the ground's faces are too large for
//! their corners to carry them. So the ground's vertices are lit as if nothing stood between them
//! and the sun, and the device draws the ground once more with this picture, which holds for every
//! quarter metre of the level how much of that light is left: what the display shows in shadow over
//! what it shows in the sun.

use duty_sim::math::v3;
use rayon::prelude::*;

use crate::light::{Lighting, Tone};
use crate::tex::Image;

pub struct Level {
    pub origin: [f32; 3],
    pub axis_x: [f32; 3],
    pub axis_z: [f32; 3],
}

pub struct Shadow {
    pub image: Image,
    /// `u = x·a + z·b + c`, then `v`.
    pub rows: [[f32; 3]; 2],
}

/// `side` texels over the square of `extent` metres about the level's origin. `albedo` and
/// `ambient` are the open ground's: its colour before light, and the light it sends back with the
/// sun off, both linear.
pub fn shadow(lighting: &Lighting, tone: &Tone, level: &Level, extent: f32, side: usize, albedo: [f32; 3], ambient: [f32; 3]) -> Shadow {
    let half = extent * 0.5;
    let to_world = |lx: f32, lz: f32| v3(level.origin[0] + level.axis_x[0] * lx + level.axis_z[0] * lz, 0.0, level.origin[2] + level.axis_x[2] * lx + level.axis_z[2] * lz);
    let sun = lighting.sky.sun;
    let direct: [f32; 3] = core::array::from_fn(|k| albedo[k] * lighting.sky.sun_colour[k] * sun.y.max(0.0) * core::f32::consts::FRAC_1_PI);
    let lit = tone.display(core::array::from_fn(|k| ambient[k] + direct[k]));
    let texels: Vec<[f32; 4]> = (0..side * side)
        .into_par_iter()
        .map(|i| {
            let (x, y) = (i % side, i / side);
            // How much of the sun four points of the texel see.
            let mut seen = 0.0;
            for (dx, dy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                let p = to_world((x as f32 + dx) / side as f32 * extent - half, (y as f32 + dy) / side as f32 * extent - half);
                seen += lighting.sun_seen(v3(p.x, lighting.ground(p.x, p.z) + 0.04, p.z), 0.02, 4) * 0.25;
            }
            let now = tone.display(core::array::from_fn(|k| ambient[k] + direct[k] * seen));
            // (the device doubles what the texture stage gives: one half leaves the ground as it is)
            [0.5 * now[0] / lit[0].max(1e-3), 0.5 * now[1] / lit[1].max(1e-3), 0.5 * now[2] / lit[2].max(1e-3), 1.0]
        })
        .collect();
    // World to picture: the level's axes are unit vectors at right angles.
    let (ax, az, o) = (level.axis_x, level.axis_z, level.origin);
    let row = |axis: [f32; 3]| [axis[0] / extent, axis[2] / extent, (-(o[0] * axis[0] + o[2] * axis[2]) + half) / extent];
    Shadow { image: Image { width: side, height: side, data: texels }, rows: [row(ax), row(az)] }
}
