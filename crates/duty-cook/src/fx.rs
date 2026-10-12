//! The effects' picture: four shapes the device tints and blends, drawn here by formula. Each is
//! white, and how much of it is there is the texel's alpha: a soft dot (a spark, a tracer's
//! head), a puff (dust, smoke), a flash (a muzzle's), and the hole a round leaves.

use crate::tex::Image;

/// A value in 0…1 that repeats for the same cell: the puff's and the flash's grain.
fn grain(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x9e37_79b1) ^ (y as u32).wrapping_mul(0x85eb_ca6b) ^ seed.wrapping_mul(0xc2b2_ae35);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    (h & 0xffff) as f32 / 65535.0
}

/// Grain at a point between cells, of `size` texels a cell.
fn smooth(x: f32, y: f32, size: f32, seed: u32) -> f32 {
    let (fx, fy) = (x / size, y / size);
    let (ix, iy) = (fx.floor() as i32, fy.floor() as i32);
    let (tx, ty) = (fx - ix as f32, fy - iy as f32);
    let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
    let a = grain(ix, iy, seed) * (1.0 - sx) + grain(ix + 1, iy, seed) * sx;
    let b = grain(ix, iy + 1, seed) * (1.0 - sx) + grain(ix + 1, iy + 1, seed) * sx;
    a * (1.0 - sy) + b * sy
}

/// `tile` texels a shape, two shapes a side.
pub fn picture(tile: usize) -> Image {
    let mut image = Image::new(tile * 2, tile * 2);
    for y in 0..tile * 2 {
        for x in 0..tile * 2 {
            let (u, v) = ((x % tile) as f32 + 0.5, (y % tile) as f32 + 0.5);
            let half = tile as f32 * 0.5;
            let (dx, dy) = ((u - half) / half, (v - half) / half);
            let r = (dx * dx + dy * dy).sqrt();
            let alpha = match (x / tile, y / tile) {
                // A dot that falls off to nothing at its rim.
                (0, 0) => (1.0 - r).clamp(0.0, 1.0).powf(1.6),
                // A puff: a dot broken up by two sizes of grain.
                (1, 0) => ((1.0 - r).clamp(0.0, 1.0).powf(0.8) * (0.45 + 0.75 * smooth(u, v, 5.0, 1) * (0.6 + 0.4 * smooth(u, v, 2.0, 2)))).clamp(0.0, 1.0),
                // A flash: a bright heart and uneven tongues.
                (0, 1) => {
                    let turn = dy.atan2(dx);
                    let tongues = 0.55 + 0.45 * (turn * 5.0 + smooth(u, v, 6.0, 3) * 4.0).sin().abs();
                    ((1.0 - r / tongues).clamp(0.0, 1.0).powf(1.2) + (1.0 - r * 3.0).clamp(0.0, 1.0)).clamp(0.0, 1.0)
                }
                // A hole: dark at its heart, a ragged rim, soot around it.
                _ => {
                    let rim = 0.3 + 0.08 * smooth(u, v, 3.0, 4);
                    if r < rim {
                        1.0
                    } else {
                        ((1.0 - (r - rim) / (1.0 - rim)).clamp(0.0, 1.0).powf(2.2) * 0.55).clamp(0.0, 1.0)
                    }
                }
            };
            image.data[y * tile * 2 + x] = [1.0, 1.0, 1.0, alpha];
        }
    }
    image
}
