//! Pictures for a handheld: 8-bit indices into a palette of 256 colours, every level stored
//! swizzled as the GE reads it.

use std::path::Path;

use duty_pack::texture::{self, PALETTE_BYTES};
use duty_pack::Texture;

/// RGBA, 0…1, rows from the top.
#[derive(Clone)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub data: Vec<[f32; 4]>,
}

impl Image {
    pub fn new(width: usize, height: usize) -> Image {
        Image { width, height, data: vec![[0.0, 0.0, 0.0, 1.0]; width * height] }
    }

    pub fn open(path: &Path) -> Result<Image, String> {
        let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut decoder = png::Decoder::new(file);
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = decoder.read_info().map_err(|e| format!("{}: {e}", path.display()))?;
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).map_err(|e| format!("{}: {e}", path.display()))?;
        let channels = info.color_type.samples();
        let (width, height) = (info.width as usize, info.height as usize);
        let data = (0..width * height)
            .map(|i| {
                let p = &buf[i * channels..];
                match channels {
                    4 => [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0, p[3] as f32 / 255.0],
                    3 => [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0, 1.0],
                    2 => [p[0] as f32 / 255.0, p[0] as f32 / 255.0, p[0] as f32 / 255.0, p[1] as f32 / 255.0],
                    _ => [p[0] as f32 / 255.0, p[0] as f32 / 255.0, p[0] as f32 / 255.0, 1.0],
                }
            })
            .collect();
        Ok(Image { width, height, data })
    }

    /// Averaged down to `width` × `height` (each a whole fraction of the picture's own).
    pub fn reduced(&self, width: usize, height: usize) -> Image {
        let (fx, fy) = (self.width / width, self.height / height);
        let mut out = Image::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let mut sum = [0.0f32; 4];
                for j in 0..fy {
                    for i in 0..fx {
                        let p = self.data[(y * fy + j) * self.width + x * fx + i];
                        for k in 0..4 {
                            sum[k] += p[k];
                        }
                    }
                }
                out.data[y * width + x] = sum.map(|s| s / (fx * fy) as f32);
            }
        }
        out
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), self.width as u32, self.height as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        let bytes: Vec<u8> = self.data.iter().flat_map(|p| p.map(|c| (c.clamp(0.0, 1.0) * 255.0 + 0.5) as u8)).collect();
        writer.write_image_data(&bytes).map_err(|e| e.to_string())
    }
}

/// 256 colours for a set of texels: the box holding the most spread is cut at its median until
/// there are enough boxes, then each box's mean is settled by a few rounds of nearest-mean.
fn palette(texels: &[[f32; 4]], colours: usize) -> Vec<[f32; 4]> {
    let mut boxes: Vec<Vec<[f32; 4]>> = vec![texels.to_vec()];
    while boxes.len() < colours {
        // The box and the channel with the widest range, weighted by how many texels it holds.
        let mut best = (0.0f32, 0usize, 0usize);
        for (i, b) in boxes.iter().enumerate() {
            if b.len() < 2 {
                continue;
            }
            for k in 0..4 {
                let (lo, hi) = b.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(p[k]), hi.max(p[k])));
                let score = (hi - lo) * (b.len() as f32).sqrt();
                if score > best.0 {
                    best = (score, i, k);
                }
            }
        }
        if best.0 <= 0.0 {
            break;
        }
        let (_, i, k) = best;
        let mut b = boxes.swap_remove(i);
        b.sort_by(|p, q| p[k].partial_cmp(&q[k]).unwrap());
        let upper = b.split_off(b.len() / 2);
        boxes.push(b);
        boxes.push(upper);
    }
    let mean = |b: &[[f32; 4]]| {
        let mut m = [0.0f32; 4];
        for p in b {
            for k in 0..4 {
                m[k] += p[k] / b.len() as f32;
            }
        }
        m
    };
    let mut out: Vec<[f32; 4]> = boxes.iter().map(|b| mean(b)).collect();
    for _ in 0..4 {
        let mut sums = vec![([0.0f32; 4], 0u32); out.len()];
        for p in texels {
            let i = nearest(&out, p);
            for k in 0..4 {
                sums[i].0[k] += p[k];
            }
            sums[i].1 += 1;
        }
        for (o, (sum, n)) in out.iter_mut().zip(&sums) {
            if *n > 0 {
                *o = sum.map(|s| s / *n as f32);
            }
        }
    }
    out.resize(colours, [0.0, 0.0, 0.0, 1.0]);
    out
}

fn nearest(palette: &[[f32; 4]], p: &[f32; 4]) -> usize {
    let mut best = (f32::MAX, 0);
    for (i, c) in palette.iter().enumerate() {
        let d = (c[0] - p[0]) * (c[0] - p[0]) + (c[1] - p[1]) * (c[1] - p[1]) + (c[2] - p[2]) * (c[2] - p[2]) + (c[3] - p[3]) * (c[3] - p[3]) * 4.0;
        if d < best.0 {
            best = (d, i);
        }
    }
    best.1
}

/// The pack's texel section as it grows.
#[derive(Default)]
pub struct Texels {
    pub records: Vec<Texture>,
    pub bytes: Vec<u8>,
}

pub struct Options {
    pub levels: usize,
    pub flags: u16,
    /// Spreads each texel's rounding error over its neighbours: for skies, whose gradients would band.
    pub dither: bool,
}

impl Texels {
    /// Adds a picture and returns its number. Its sides are powers of two.
    pub fn add(&mut self, image: &Image, options: &Options) -> u32 {
        assert!(image.width.is_power_of_two() && image.height.is_power_of_two() && image.width >= 16 && image.height >= 16 && image.width <= 512 && image.height <= 512, "a texture of {} × {}", image.width, image.height);
        let cutout = options.flags & texture::CUTOUT != 0;
        let mut levels = vec![image.clone()];
        while levels.len() < options.levels && levels.last().unwrap().width > 16 && levels.last().unwrap().height > 16 {
            let last = levels.last().unwrap();
            levels.push(last.reduced(last.width / 2, last.height / 2));
        }
        // The palette is made from the first two levels: the ones seen most.
        let mut sample: Vec<[f32; 4]> = levels.iter().take(2).flat_map(|l| l.data.iter().copied()).collect();
        if cutout {
            // A texel is a hole or it is not.
            for p in &mut sample {
                p[3] = if p[3] < 0.5 { 0.0 } else { 1.0 };
            }
            sample.retain(|p| p[3] > 0.5);
        } else {
            for p in &mut sample {
                p[3] = 1.0;
            }
        }
        let mut colours = palette(&sample, if cutout { 255 } else { 256 });
        if cutout {
            colours.insert(0, [0.0, 0.0, 0.0, 0.0]);
        }
        self.bytes.resize((self.bytes.len() + 15) & !15, 0);
        let offset = self.bytes.len() as u32;
        for c in &colours {
            // ABGR in a little-endian word: red is the first byte.
            self.bytes.extend(c.map(|v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8));
        }
        debug_assert_eq!(self.bytes.len() - offset as usize, PALETTE_BYTES);
        for level in &levels {
            let (w, h) = (level.width, level.height);
            let mut indices = vec![0u8; w * h];
            let mut carried = vec![[0.0f32; 3]; if options.dither { w * h } else { 0 }];
            for y in 0..h {
                for x in 0..w {
                    let mut p = level.data[y * w + x];
                    if cutout && p[3] < 0.5 {
                        continue;
                    }
                    p[3] = 1.0;
                    if options.dither {
                        for k in 0..3 {
                            p[k] += carried[y * w + x][k];
                        }
                    }
                    let i = if cutout { 1 + nearest(&colours[1..], &p) } else { nearest(&colours, &p) };
                    indices[y * w + x] = i as u8;
                    if options.dither {
                        // Floyd and Steinberg's weights.
                        let error: [f32; 3] = core::array::from_fn(|k| p[k] - colours[i][k]);
                        for (dx, dy, share) in [(1i32, 0i32, 7.0 / 16.0), (-1, 1, 3.0 / 16.0), (0, 1, 5.0 / 16.0), (1, 1, 1.0 / 16.0)] {
                            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                            if nx >= 0 && nx < w as i32 && ny < h as i32 {
                                for k in 0..3 {
                                    carried[ny as usize * w + nx as usize][k] += error[k] * share;
                                }
                            }
                        }
                    }
                }
            }
            self.bytes.resize((self.bytes.len() + 15) & !15, 0);
            self.bytes.extend(pocket_psp_ge::swizzle::swizzle_rows(&indices, w, h).expect("swizzle"));
        }
        self.records.push(Texture { offset, width: image.width as u16, height: image.height as u16, levels: levels.len() as u8, format: texture::T8, flags: options.flags });
        self.records.len() as u32 - 1
    }
}

/// A texel of a picture in the pack, for drawing a frame on this computer: `(x, y)` wrap.
pub fn fetch(record: &Texture, texels: &[u8], level: u32, x: i32, y: i32) -> [f32; 4] {
    let (w, h) = ((record.width as i32 >> level).max(1), (record.height as i32 >> level).max(1));
    let (x, y) = (x.rem_euclid(w) as usize, y.rem_euclid(h) as usize);
    let (at, _) = record.level(level);
    let base = record.offset as usize;
    // Swizzled: blocks of 16 bytes by 8 rows.
    let row_blocks = (w as usize) / 16;
    let block = (y / 8) * row_blocks + x / 16;
    let index = texels[base + at + block * 128 + (y % 8) * 16 + x % 16] as usize;
    let c = &texels[base + index * 4..base + index * 4 + 4];
    [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, c[3] as f32 / 255.0]
}
