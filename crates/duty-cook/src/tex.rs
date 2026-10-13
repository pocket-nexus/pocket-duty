//! Pictures for a handheld. For the PSP: 8-bit indices into a palette of 256 colours, every level
//! stored swizzled as the GE reads it. For the 3DS, whose PICA200 has no palettes: 16 bits a
//! texel, every level in the tiles it reads.

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
        assert!(width <= self.width && height <= self.height, "a picture of {} × {} is not made larger ({width} × {height})", self.width, self.height);
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

    /// Averaged down as `reduced` does, a texel's colour counting as much as the texel is there: the
    /// colour of a shape's edge is the shape's, however much of the edge is hole.
    pub fn reduced_by_cover(&self, width: usize, height: usize) -> Image {
        assert!(width <= self.width && height <= self.height, "a picture of {} × {} is not made larger ({width} × {height})", self.width, self.height);
        let (fx, fy) = (self.width / width, self.height / height);
        let mut out = Image::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let (mut sum, mut cover) = ([0.0f32; 3], 0.0f32);
                for j in 0..fy {
                    for i in 0..fx {
                        let p = self.data[(y * fy + j) * self.width + x * fx + i];
                        for k in 0..3 {
                            sum[k] += p[k] * p[3];
                        }
                        cover += p[3];
                    }
                }
                out.data[y * width + x] = if cover > 0.0 { [sum[0] / cover, sum[1] / cover, sum[2] / cover, cover / (fx * fy) as f32] } else { [0.0; 4] };
            }
        }
        out
    }

    /// Gives every texel that is not there at all (alpha 0) the colour of the texels about it that
    /// are, ring by ring until none is left; its alpha stays 0. The picture repeats at its edges.
    pub fn spread(&mut self) {
        let (w, h) = (self.width as i32, self.height as i32);
        let mut known: Vec<bool> = self.data.iter().map(|p| p[3] > 0.0).collect();
        if !known.contains(&true) {
            return;
        }
        while known.contains(&false) {
            let mut found: Vec<(usize, [f32; 3])> = Vec::new();
            for y in 0..h {
                for x in 0..w {
                    let at = (y * w + x) as usize;
                    if known[at] {
                        continue;
                    }
                    let (mut sum, mut weight) = ([0.0f32; 3], 0.0f32);
                    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)] {
                        let near = ((y + dy).rem_euclid(h) * w + (x + dx).rem_euclid(w)) as usize;
                        if known[near] {
                            // (a texel that is there counts by its cover, one this spread reached counts as a faint one)
                            let share = self.data[near][3].max(1.0 / 255.0) * if dx != 0 && dy != 0 { 0.7 } else { 1.0 };
                            for k in 0..3 {
                                sum[k] += self.data[near][k] * share;
                            }
                            weight += share;
                        }
                    }
                    if weight > 0.0 {
                        found.push((at, sum.map(|c| c / weight)));
                    }
                }
            }
            if found.is_empty() {
                break;
            }
            for (at, colour) in found {
                self.data[at] = [colour[0], colour[1], colour[2], 0.0];
                known[at] = true;
            }
        }
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
    /// The PICA200's layouts (`texture::RGB565` and the others) instead of the GE's palettes.
    pub pica: bool,
    /// True colour, four bytes a texel in rows, every level down to one texel (`texture::RGBA8`):
    /// a device with room for it (the PS Vita). No palette, so nothing is rounded to 256 colours.
    pub rgba: bool,
    /// With `rgba`: a picture that repeats (a surface's) is held in the GPU's blocks of 4 × 4 texels
    /// (`texture::BC1`, or `BC3` when it has holes), a sixth or a quarter of the bytes. A picture that
    /// is clamped (the sky's, the ground's shadow) keeps its four bytes a texel: its slow changes of
    /// colour would show the blocks.
    pub blocks: bool,
}

/// The two 16-bit colours and the sixteen 2-bit choices that stand for a block of texels (BC1 with
/// four colours: the two and the two thirds between them), as the block's 8 bytes.
fn colour_block(texels: &[[f32; 3]; 16]) -> [u8; 8] {
    let pack = |c: [f32; 3]| -> u16 { ((c[0].clamp(0.0, 1.0) * 31.0 + 0.5) as u16) << 11 | ((c[1].clamp(0.0, 1.0) * 63.0 + 0.5) as u16) << 5 | (c[2].clamp(0.0, 1.0) * 31.0 + 0.5) as u16 };
    let wide = |v: u16| -> [f32; 3] { [(v >> 11) as f32 / 31.0, (v >> 5 & 63) as f32 / 63.0, (v & 31) as f32 / 31.0] };
    // What a pair of colours makes of the block: each texel's nearest of the four, and the squared error.
    let fit = |c0: u16, c1: u16| -> ([u8; 16], f32) {
        let (a, z) = (wide(c0), wide(c1));
        let four: [[f32; 3]; 4] = [a, z, core::array::from_fn(|k| (2.0 * a[k] + z[k]) / 3.0), core::array::from_fn(|k| (a[k] + 2.0 * z[k]) / 3.0)];
        let mut chosen = [0u8; 16];
        let mut error = 0.0;
        for (i, t) in texels.iter().enumerate() {
            let mut best = f32::MAX;
            for (n, c) in four.iter().enumerate() {
                let d = (t[0] - c[0]) * (t[0] - c[0]) + (t[1] - c[1]) * (t[1] - c[1]) + (t[2] - c[2]) * (t[2] - c[2]);
                if d < best {
                    best = d;
                    chosen[i] = n as u8;
                }
            }
            error += best;
        }
        (chosen, error)
    };
    // The line through the texels' colours along which they spread most, and its two ends.
    let mean: [f32; 3] = core::array::from_fn(|k| texels.iter().map(|t| t[k]).sum::<f32>() / 16.0);
    let mut cov = [[0.0f32; 3]; 3];
    for t in texels {
        for i in 0..3 {
            for j in 0..3 {
                cov[i][j] += (t[i] - mean[i]) * (t[j] - mean[j]);
            }
        }
    }
    let mut axis: [f32; 3] = core::array::from_fn(|k| texels.iter().map(|t| t[k]).fold(f32::MIN, f32::max) - texels.iter().map(|t| t[k]).fold(f32::MAX, f32::min));
    for _ in 0..8 {
        let next: [f32; 3] = core::array::from_fn(|i| cov[i][0] * axis[0] + cov[i][1] * axis[1] + cov[i][2] * axis[2]);
        let length = (next[0] * next[0] + next[1] * next[1] + next[2] * next[2]).sqrt();
        if length < 1e-12 {
            break;
        }
        axis = next.map(|c| c / length);
    }
    let along = |t: &[f32; 3]| (t[0] - mean[0]) * axis[0] + (t[1] - mean[1]) * axis[1] + (t[2] - mean[2]) * axis[2];
    let (low, high) = texels.iter().fold((f32::MAX, f32::MIN), |(low, high), t| (low.min(along(t)), high.max(along(t))));
    let end = |d: f32| -> [f32; 3] { core::array::from_fn(|k| mean[k] + axis[k] * d) };
    let (mut c0, mut c1) = (pack(end(high)), pack(end(low)));
    let (mut chosen, mut error) = fit(c0, c1);
    // The two colours that fit the texels best for the choices made, a few times over.
    for _ in 0..4 {
        // Least squares for `a·w + z·(1 − w)` with `w` the share of the first colour in each texel's choice.
        let (mut aa, mut az, mut zz) = (0.0f32, 0.0f32, 0.0f32);
        let (mut ta, mut tz) = ([0.0f32; 3], [0.0f32; 3]);
        for (t, n) in texels.iter().zip(&chosen) {
            let w = [1.0, 0.0, 2.0 / 3.0, 1.0 / 3.0][*n as usize];
            aa += w * w;
            az += w * (1.0 - w);
            zz += (1.0 - w) * (1.0 - w);
            for k in 0..3 {
                ta[k] += t[k] * w;
                tz[k] += t[k] * (1.0 - w);
            }
        }
        let det = aa * zz - az * az;
        if det.abs() < 1e-6 {
            break;
        }
        let (n0, n1) = (pack(core::array::from_fn(|k| (ta[k] * zz - tz[k] * az) / det)), pack(core::array::from_fn(|k| (tz[k] * aa - ta[k] * az) / det)));
        let (again, now) = fit(n0, n1);
        if now >= error {
            break;
        }
        (c0, c1, chosen, error) = (n0, n1, again, now);
    }
    // Four colours are the first above the second; the other order is three and a hole.
    if c0 < c1 {
        core::mem::swap(&mut c0, &mut c1);
        for n in &mut chosen {
            *n ^= 1;
        }
    } else if c0 == c1 {
        chosen = [0; 16];
    }
    let bits = chosen.iter().enumerate().fold(0u32, |all, (i, n)| all | (*n as u32) << (i * 2));
    let mut out = [0u8; 8];
    out[..2].copy_from_slice(&c0.to_le_bytes());
    out[2..4].copy_from_slice(&c1.to_le_bytes());
    out[4..].copy_from_slice(&bits.to_le_bytes());
    out
}

/// The two alphas and the sixteen 3-bit choices among them and the six between (BC3's first 8 bytes).
fn alpha_block(alphas: &[f32; 16]) -> [u8; 8] {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    let (a0, a1) = (byte(alphas.iter().copied().fold(f32::MIN, f32::max)), byte(alphas.iter().copied().fold(f32::MAX, f32::min)));
    let mut bits = 0u64;
    if a0 > a1 {
        let eight: [f32; 8] = core::array::from_fn(|i| match i {
            0 => a0 as f32,
            1 => a1 as f32,
            i => (a0 as f32 * (8 - i) as f32 + a1 as f32 * (i - 1) as f32) / 7.0,
        });
        for (i, a) in alphas.iter().enumerate() {
            let want = a.clamp(0.0, 1.0) * 255.0;
            let n = (0..8).min_by(|p, q| (eight[*p] - want).abs().total_cmp(&(eight[*q] - want).abs())).unwrap();
            bits |= (n as u64) << (i * 3);
        }
    }
    let mut out = [0u8; 8];
    out[0] = a0;
    out[1] = a1;
    out[2..].copy_from_slice(&bits.to_le_bytes()[..6]);
    out
}

/// A level in blocks of 4 × 4 texels, the top row of blocks first: a block at the edge of a level
/// smaller than itself repeats the level's last texels.
fn blocks(level: &Image, alpha: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(level.width.div_ceil(4) * level.height.div_ceil(4) * if alpha { 16 } else { 8 });
    for by in 0..level.height.div_ceil(4) {
        for bx in 0..level.width.div_ceil(4) {
            let texel = |i: usize| level.data[(by * 4 + i / 4).min(level.height - 1) * level.width + (bx * 4 + i % 4).min(level.width - 1)];
            if alpha {
                out.extend(alpha_block(&core::array::from_fn(|i| texel(i)[3])));
            }
            out.extend(colour_block(&core::array::from_fn(|i| {
                let p = texel(i);
                [p[0], p[1], p[2]]
            })));
        }
    }
    out
}

/// `values` (one for each texel, rows from the top) in the PICA200's order: tiles of 8 × 8, the
/// top row of tiles first, a tile's texels in Z order.
fn tiled<T: Copy + Default>(values: &[T], width: usize, height: usize) -> Vec<T> {
    let spread = |v: usize| (v & 1) | (v & 2) << 1 | (v & 4) << 2;
    let mut out = vec![T::default(); width * height];
    for y in 0..height {
        for x in 0..width {
            out[((y / 8) * (width / 8) + x / 8) * 64 + (spread(x % 8) | spread(y % 8) << 1)] = values[y * width + x];
        }
    }
    out
}

pub struct Options {
    pub levels: usize,
    pub flags: u16,
    /// Spreads each texel's rounding error over its neighbours: for skies, whose gradients would band.
    pub dither: bool,
}

impl Texels {
    /// Adds a picture that is one colour at 256 strengths: the palette's entry `i` is white as
    /// opaque as `i`, and a texel is its alpha.
    pub fn add_ramp(&mut self, image: &Image, flags: u16) -> u32 {
        self.bytes.resize((self.bytes.len() + 15) & !15, 0);
        let offset = self.bytes.len() as u32;
        if self.rgba {
            self.bytes.extend(image.data.iter().flat_map(|p| [255, 255, 255, (p[3].clamp(0.0, 1.0) * 255.0 + 0.5) as u8]));
            self.records.push(Texture { offset, width: image.width as u16, height: image.height as u16, levels: 1, format: texture::RGBA8, flags });
            return self.records.len() as u32 - 1;
        }
        if self.pica {
            let alpha: Vec<u8> = image.data.iter().map(|p| (p[3].clamp(0.0, 1.0) * 255.0 + 0.5) as u8).collect();
            self.bytes.extend(tiled(&alpha, image.width, image.height));
            self.records.push(Texture { offset, width: image.width as u16, height: image.height as u16, levels: 1, format: texture::A8, flags });
            return self.records.len() as u32 - 1;
        }
        for i in 0..256u32 {
            self.bytes.extend([255, 255, 255, i as u8]);
        }
        let indices: Vec<u8> = image.data.iter().map(|p| (p[3].clamp(0.0, 1.0) * 255.0 + 0.5) as u8).collect();
        self.bytes.extend(pocket_psp_ge::swizzle::swizzle_rows(&indices, image.width, image.height).expect("swizzle"));
        self.records.push(Texture { offset, width: image.width as u16, height: image.height as u16, levels: 1, format: texture::T8, flags });
        self.records.len() as u32 - 1
    }

    /// Adds a picture and returns its number. Its sides are powers of two.
    pub fn add(&mut self, image: &Image, options: &Options) -> u32 {
        if self.rgba {
            return self.add_rgba(image, options);
        }
        assert!(image.width.is_power_of_two() && image.height.is_power_of_two() && image.width >= 16 && image.height >= 16 && image.width <= 512 && image.height <= 512, "a texture of {} × {}", image.width, image.height);
        let cutout = options.flags & texture::CUTOUT != 0;
        let mut levels = vec![image.clone()];
        while levels.len() < options.levels && levels.last().unwrap().width > 16 && levels.last().unwrap().height > 16 {
            let last = levels.last().unwrap();
            levels.push(last.reduced(last.width / 2, last.height / 2));
        }
        if self.pica {
            return self.add_pica(&levels, options);
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

impl Texels {
    /// The picture as it is, four bytes a texel, and every smaller level down to one texel: each a
    /// mean of the four texels above it. A picture with holes keeps its alpha; the device tests it
    /// against one half.
    fn add_rgba(&mut self, image: &Image, options: &Options) -> u32 {
        assert!(image.width.is_power_of_two() && image.height.is_power_of_two() && image.width <= 4096 && image.height <= 4096, "a texture of {} × {}", image.width, image.height);
        let cutout = options.flags & texture::CUTOUT != 0;
        let mut levels = vec![image.clone()];
        if cutout {
            // A clear texel's colour is nobody's (black, in the pictures the reference forges), and the
            // device's filter mixes it into the edge of every shape: it takes the colour of the texels
            // about it that are there, at every level.
            levels[0].spread();
        }
        while levels.len() < options.levels && (levels.last().unwrap().width > 1 || levels.last().unwrap().height > 1) {
            let last = levels.last().unwrap();
            let (width, height) = ((last.width / 2).max(1), (last.height / 2).max(1));
            let next = if cutout {
                let mut next = last.reduced_by_cover(width, height);
                next.spread();
                next
            } else {
                last.reduced(width, height)
            };
            levels.push(next);
        }
        if cutout {
            // A smaller level is a mean of texels, and the mean of a thin shape (a leaf, a wire) is under one
            // half everywhere: the shape would go at a distance. Each level's alpha is raised until as much
            // of the picture passes the device's test as passes it in the first.
            let passing = |level: &Image, gain: f32| level.data.iter().filter(|p| p[3] * gain >= 0.5).count() as f32 / level.data.len() as f32;
            let want = passing(&levels[0], 1.0);
            for level in levels.iter_mut().skip(1) {
                let (mut low, mut high) = (1.0f32, 16.0f32);
                for _ in 0..12 {
                    let gain = (low + high) * 0.5;
                    if passing(level, gain) < want {
                        low = gain;
                    } else {
                        high = gain;
                    }
                }
                for p in &mut level.data {
                    p[3] = (p[3] * high).min(1.0);
                }
            }
        }
        self.bytes.resize((self.bytes.len() + 15) & !15, 0);
        let offset = self.bytes.len() as u32;
        // (a surface's picture goes in blocks where the profile asks; the sky's and the shadow's are clamped)
        let in_blocks = self.blocks && options.flags & texture::CLAMP == 0 && image.width >= 4 && image.height >= 4;
        for level in &levels {
            self.bytes.resize((self.bytes.len() + 15) & !15, 0);
            if in_blocks {
                self.bytes.extend(blocks(level, cutout));
                continue;
            }
            let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            self.bytes.extend(level.data.iter().flat_map(|p| [byte(p[0]), byte(p[1]), byte(p[2]), if cutout { byte(p[3]) } else { 255 }]));
        }
        let format = match (in_blocks, cutout) {
            (false, _) => texture::RGBA8,
            (true, false) => texture::BC1,
            (true, true) => texture::BC3,
        };
        self.records.push(Texture { offset, width: image.width as u16, height: image.height as u16, levels: levels.len() as u8, format, flags: options.flags });
        self.records.len() as u32 - 1
    }

    /// The levels as 16-bit texels: five bits each and one of alpha where the picture has holes,
    /// five, six and five where it has none.
    fn add_pica(&mut self, levels: &[Image], options: &Options) -> u32 {
        let cutout = options.flags & texture::CUTOUT != 0;
        self.bytes.resize((self.bytes.len() + 15) & !15, 0);
        let offset = self.bytes.len() as u32;
        let bits: [u32; 3] = if cutout { [5, 5, 5] } else { [5, 6, 5] };
        for level in levels {
            let (w, h) = (level.width, level.height);
            let mut texels = vec![0u16; w * h];
            let mut carried = vec![[0.0f32; 3]; if options.dither { w * h } else { 0 }];
            for y in 0..h {
                for x in 0..w {
                    let mut p = level.data[y * w + x];
                    if options.dither {
                        for k in 0..3 {
                            p[k] += carried[y * w + x][k];
                        }
                    }
                    let q: [u16; 3] = core::array::from_fn(|k| (p[k].clamp(0.0, 1.0) * ((1 << bits[k]) - 1) as f32 + 0.5) as u16);
                    texels[y * w + x] = if cutout { q[0] << 11 | q[1] << 6 | q[2] << 1 | (p[3] >= 0.5) as u16 } else { q[0] << 11 | q[1] << 5 | q[2] };
                    if options.dither {
                        // Floyd and Steinberg's weights.
                        let error: [f32; 3] = core::array::from_fn(|k| p[k] - q[k] as f32 / ((1 << bits[k]) - 1) as f32);
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
            self.bytes.extend(tiled(&texels, w, h).iter().flat_map(|t| t.to_le_bytes()));
        }
        let first = &levels[0];
        self.records.push(Texture { offset, width: first.width as u16, height: first.height as u16, levels: levels.len() as u8, format: if cutout { texture::RGBA5551 } else { texture::RGB565 }, flags: options.flags });
        self.records.len() as u32 - 1
    }
}

/// A texel of a picture in the pack, for drawing a frame on this computer: `(x, y)` wrap.
pub fn fetch(record: &Texture, texels: &[u8], level: u32, x: i32, y: i32) -> [f32; 4] {
    let (w, h) = ((record.width as i32 >> level).max(1), (record.height as i32 >> level).max(1));
    record.texel(texels, level, x.rem_euclid(w) as usize, y.rem_euclid(h) as usize).map(|c| c as f32 / 255.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A true-colour picture is its rows, every level down to one texel, and reads back texel by texel.
    #[test]
    fn a_true_colour_picture_keeps_its_texels_and_every_level() {
        let mut image = Image::new(8, 4);
        for (i, p) in image.data.iter_mut().enumerate() {
            *p = [i as f32 / 31.0, 0.25, 1.0 - i as f32 / 31.0, 1.0];
        }
        let mut texels = Texels { rgba: true, ..Default::default() };
        let at = texels.add(&image, &Options { levels: 10, flags: 0, dither: false }) as usize;
        let record = texels.records[at];
        assert_eq!((record.format, record.width, record.height, record.levels), (texture::RGBA8, 8, 4, 4));
        assert_eq!(record.texel(&texels.bytes, 0, 5, 2), [((21.0f32 / 31.0) * 255.0 + 0.5) as u8, 64, ((10.0f32 / 31.0) * 255.0 + 0.5) as u8, 255]);
        // (the last level is one texel: the mean of them all)
        let (start, bytes) = record.level(3);
        assert_eq!((bytes, start + bytes <= texels.bytes.len() - record.offset as usize), (4, true));
        assert_eq!(record.texel(&texels.bytes, 3, 0, 0)[1], 64);
    }

    /// A surface's picture in blocks reads back within the blocks' rounding, level by level, and a
    /// picture with holes keeps them.
    #[test]
    fn a_picture_in_blocks_reads_back_close_to_its_texels() {
        let mut image = Image::new(16, 16);
        for (i, p) in image.data.iter_mut().enumerate() {
            let (x, y) = ((i % 16) as f32 / 15.0, (i / 16) as f32 / 15.0);
            *p = [0.3 + 0.4 * x, 0.5, 0.7 - 0.4 * y, if (i % 16) < 8 { 1.0 } else { 0.0 }];
        }
        for (flags, format, block) in [(0, texture::BC1, 8), (texture::CUTOUT, texture::BC3, 16)] {
            let mut texels = Texels { rgba: true, blocks: true, ..Default::default() };
            let at = texels.add(&image, &Options { levels: 10, flags, dither: false }) as usize;
            let record = texels.records[at];
            assert_eq!((record.format, record.levels, record.block()), (format, 5, Some(block)));
            // (16 blocks, then 4, then one for each level under a block's size)
            assert_eq!((record.level(0).1, record.level(1).1, record.level(2).1, record.level(4).1), (16 * block, 4 * block, block, block));
            for (x, y) in [(0usize, 0usize), (5, 3), (15, 15), (2, 12)] {
                let c = record.texel(&texels.bytes, 0, x, y);
                let want = image.data[y * 16 + x];
                // (a hole's colour is the colour about it: only a texel that is there is compared)
                for k in 0..3 {
                    assert!(flags != 0 && x >= 8 || (c[k] as f32 / 255.0 - want[k]).abs() < 0.06, "texel {x},{y} channel {k}: {} for {}", c[k], want[k]);
                }
                if flags != 0 {
                    assert_eq!(c[3], if x < 8 { 255 } else { 0 });
                }
            }
        }
        // A clamped picture (the sky's) keeps its four bytes a texel.
        let mut texels = Texels { rgba: true, blocks: true, ..Default::default() };
        let at = texels.add(&image, &Options { levels: 1, flags: texture::CLAMP, dither: false }) as usize;
        assert_eq!(texels.records[at].format, texture::RGBA8);
    }

    /// A hole's texels take the colour of the shape beside them, at every level, and stay holes.
    #[test]
    fn a_hole_takes_the_colour_of_what_is_about_it() {
        let mut image = Image::new(32, 32);
        for (i, p) in image.data.iter_mut().enumerate() {
            // A green column in eight; the rest is clear and black.
            *p = if i % 8 == 0 { [0.25, 0.75, 0.5, 1.0] } else { [0.0, 0.0, 0.0, 0.0] };
        }
        let mut texels = Texels { rgba: true, ..Default::default() };
        let at = texels.add(&image, &Options { levels: 10, flags: texture::CUTOUT, dither: false }) as usize;
        let record = texels.records[at];
        for l in 0..record.levels as u32 {
            let side = (32usize >> l).max(1);
            for i in 0..side * side {
                let c = record.texel(&texels.bytes, l, i % side, i / side);
                assert_eq!((c[0], c[1], c[2]), (64, 191, 128), "level {l}, texel {i}");
            }
        }
        // (the first level's holes are still holes)
        assert_eq!(record.texel(&texels.bytes, 0, 3, 3)[3], 0);
        assert_eq!(record.texel(&texels.bytes, 0, 8, 3)[3], 255);
    }

    /// A thin shape passes the device's test in as much of a smaller level as of the first.
    #[test]
    fn a_picture_with_holes_keeps_its_cover_in_the_smaller_levels() {
        let mut image = Image::new(32, 32);
        for (i, p) in image.data.iter_mut().enumerate() {
            // One column in four is there.
            *p = [1.0, 1.0, 1.0, if i % 4 == 0 { 1.0 } else { 0.0 }];
        }
        let mut texels = Texels { rgba: true, ..Default::default() };
        let at = texels.add(&image, &Options { levels: 10, flags: texture::CUTOUT, dither: false }) as usize;
        let record = texels.records[at];
        let passing = |l: u32| {
            let side = 32usize >> l;
            (0..side * side).filter(|i| record.texel(&texels.bytes, l, i % side, i / side)[3] >= 128).count() as f32 / (side * side) as f32
        };
        assert_eq!(passing(0), 0.25);
        // (the plain mean of a level's four texels is a quarter: nothing would pass)
        for l in 1..3 {
            assert!(passing(l) >= 0.25, "level {l} keeps {}", passing(l));
        }
    }
}
