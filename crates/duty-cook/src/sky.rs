//! The sky as pictures: the reference's dome program rendered the sky into six faces of linear
//! radiance; here it becomes a band of pictures around the horizon, as the display would show it.

use duty_sim::math::v3;

use crate::light::{Sky, Tone};
use crate::tex::Image;

pub struct Band {
    pub columns: Vec<Image>,
    /// Radians above the horizon of the pictures' bottom and top edges.
    pub floor: f32,
    pub top: f32,
    pub zenith: [f32; 3],
}

/// `columns` pictures of `width` × `height` side by side around the horizon. Column 0 starts at
/// the direction +X and the columns turn toward +Z.
pub fn band(sky: &Sky, tone: &Tone, columns: usize, width: usize, height: usize, floor: f32, top: f32) -> Band {
    let zenith = tone.display(sky.soft(v3(0.0, 1.0, 0.0)));
    let mut out = Vec::new();
    for c in 0..columns {
        let mut image = Image::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let mut sum = [0.0f32; 3];
                for (dx, dy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                    let azimuth = (c as f32 + (x as f32 + dx) / width as f32) / columns as f32 * core::f32::consts::TAU;
                    let elevation = top + (floor - top) * (y as f32 + dy) / height as f32;
                    let d = v3(elevation.cos() * azimuth.cos(), elevation.sin(), elevation.cos() * azimuth.sin());
                    let l = tone.display(sky.radiance(d));
                    for k in 0..3 {
                        sum[k] += l[k] * 0.25;
                    }
                }
                // The top rows run into the zenith's colour: above them the device draws that alone.
                let fade = (y as f32 / height as f32 * 10.0).min(1.0);
                image.data[y * width + x] = [zenith[0] + (sum[0] - zenith[0]) * fade, zenith[1] + (sum[1] - zenith[1]) * fade, zenith[2] + (sum[2] - zenith[2]) * fade, 1.0];
            }
        }
        out.push(image);
    }
    Band { columns: out, floor, top, zenith }
}
