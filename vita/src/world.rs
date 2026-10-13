//! The street on GXM: the pack's chunks, pictures and sky as the GPU reads
//! them, and a frame's draws in the order of the shader renderer the browser
//! has (`wgpu/src/render.rs`):
//!
//! 1. the haze's colour over the whole screen, then the sky: a band of
//!    pictures round the horizon and the zenith's colour above it;
//! 2. the street's solid chunks, each run of indices `duty_sim::view` chose;
//!    the ground's chunks take the sun's shadow from a picture in the same
//!    pass; then the glass, the farthest first;
//! 3. the effects' squares, and the muzzle's flash in the held weapon's
//!    frustum (`figures.rs` draws the soldiers before them and the weapon
//!    after).
//!
//! What this console adds to that frame is in the pack (`profiles/vita60.json`:
//! true-colour pictures at the size the reference bakes, the nearest level of
//! detail out to 48 m, a shadow picture of 2048 texels a side, a sky of 4096
//! texels round) and in two places here: the pictures are mixed between two
//! levels, and a muzzle's light falls on what is near it (`FLASH`).

use duty_pack::{self as pack, chunk, texture, Chunk, Scene, Texture as Record, WorldVertex};
use duty_sim::fx::{self, FxVertex};
use duty_sim::mat::{self, Mat4};
use duty_sim::math::*;
use duty_sim::view::Draw;
use pocket_vita_gxm::mem::{Arena, Block, Kind};
use pocket_vita_gxm::program::{F32, S16, U8N};
use pocket_vita_gxm::texture::{Format, Texture, Uploader, Wrap};
use vita2d_sys as g;

use crate::gpu::{self, Blend, Cull, Gpu, Param, Program, Stream, Uniforms};
use crate::store::PackFile;

pub const WIDTH: f32 = 960.0;
pub const HEIGHT: f32 = 544.0;
/// The share of the depth range the held weapon is drawn in: nothing of the street comes in front of it.
pub const HELD_DEPTH: f32 = 0.0625;
/// The sky's band: quads across one picture and down it.
const SKY_SEGMENTS: usize = 8;
const SKY_ROWS: usize = 6;
/// The longest side of a picture the transfer engine is asked to put in the GPU's own order.
const SWIZZLED_SIDE: u32 = 512;
/// Vertices read from the pack at a time.
const PART: usize = 65536;

/// A world vertex as the GPU reads it: the pack's (`duty_pack::WorldVertex`) with its colour as four bytes.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Vertex {
    pos: [i16; 3],
    /// The red and the green of the vertex's colour with the sun off (`pack::WSHD`); its blue is `colour`'s
    /// fourth byte. The vertex's own colour again in a pack without that section.
    shade: [u8; 2],
    uv: [i16; 2],
    colour: [u8; 4],
}

/// What a fragment of the street does beside the picture and the baked light: one program for each set.
mod variant {
    pub const CUTOUT: usize = 1;
    pub const GROUND: usize = 2;
    pub const FLASH: usize = 4;
    /// The sun's shadow from its depth picture (walls and things; the ground's is `GROUND`'s picture).
    pub const SUN: usize = 8;
    pub const COUNT: usize = 16;
}

struct Street {
    prog: Program,
    clip: Param,
    place: Param,
    uv: Param,
    sun: Param,
    haze: Param,
    look: Param,
    flash: Param,
    flash_colour: Param,
    depth: Param,
    sun_edge: Param,
    picture: u32,
    shadow: u32,
    depth_picture: u32,
}

struct Flat {
    prog: Program,
    clip: Param,
    picture: u32,
}

/// The switches a development host sets, each a part of the frame that can be left out to learn its cost.
#[derive(Clone, Copy)]
pub struct Look {
    pub sky: bool,
    pub haze: bool,
    pub shadows: bool,
    pub effects: bool,
    pub figures: bool,
    /// The soldiers' shadows on the ground.
    pub shades: bool,
    /// The muzzle's light on the street.
    pub flash: bool,
    /// A measurement: every fragment of the street reads the ground's shadow picture, not the ground's alone.
    pub walls: bool,
    /// The sun's shadow on walls and things from its depth picture, where the pack has one.
    pub sun: bool,
    /// Two runs of one chunk with no more indices than this between them are drawn as one: the triangles
    /// between are cheaper than a draw.
    pub merge: u32,
    /// Which faces are left out: 0 the back ones, 1 the front ones, 2 none (to tell a wrong winding from a hole).
    pub cull: u8,
}

impl Look {
    /// (two runs are joined across 50 triangles: measured on the console in a view of 137 000 triangles, the
    /// GPU takes 0.45 ms less than across 200 and this thread's draws 0.18 ms more)
    pub const DEFAULT: Look = Look { sky: true, haze: true, shadows: true, effects: true, figures: true, shades: true, flash: true, walls: false, sun: true, merge: 150, cull: 0 };

    pub fn cull(&self, two_sided: bool) -> Cull {
        match (two_sided, self.cull) {
            (true, _) | (_, 2) => Cull::None,
            (_, 1) => Cull::Ccw,
            _ => Cull::Cw,
        }
    }
}

/// A frame of the street.
pub struct View<'a> {
    /// Clip from world, and clip from the eye's own directions (the view turned, not moved).
    pub world: Mat4,
    pub eye_turn: Mat4,
    pub eye: V3,
    pub draws: &'a [Draw],
    pub look: Look,
    /// A muzzle's light: where in the world, and how strong.
    pub flash: Option<(V3, f32)>,
}

/// What a frame drew.
#[derive(Clone, Copy, Default)]
pub struct Drawn {
    pub draws: u32,
    pub triangles: u32,
}

/// The programs of the street, the sky and the effects: made once, for every map.
pub struct Programs {
    street: Vec<Street>,
    band: Flat,
    cap: Flat,
    cover: Flat,
    glow: Flat,
}

/// One map on the GPU: its geometry and its pictures.
pub struct Gfx {
    pub scene: Scene,
    /// Vertices, indices, the sky and the squares' order, in memory the GPU reads.
    geometry: Block,
    vertices: *const u8,
    indices: *const u16,
    sky_vertices: *const u8,
    sky_indices: *const u16,
    sky_column: u32,
    sky_cap: u32,
    pub quads: *const u16,
    pictures: Vec<Texture>,
    /// How much of the sun each texel of the ground's shadow picture lets through, of 255.
    sun: (Vec<u8>, usize, usize),
    glass: Vec<(f32, Draw)>,
    /// The sun's depth picture and its numbers, in a pack whose vertices keep the sun's light apart.
    pub sun_depth: Option<pack::SunDepth>,
    pub geometry_bytes: usize,
    pub picture_bytes: usize,
    pub vertex_count: usize,
    pub index_count: usize,
}

fn abgr(colour: u32) -> [f32; 3] {
    [(colour & 255) as f32 / 255.0, ((colour >> 8) & 255) as f32 / 255.0, ((colour >> 16) & 255) as f32 / 255.0]
}

/// Draws two runs of one chunk's level as one when no more than `gap` indices lie between them.
pub fn merge(draws: &mut Vec<Draw>, gap: u32) {
    let mut kept = 0usize;
    for i in 0..draws.len() {
        let d = draws[i];
        if kept > 0 {
            let last = &mut draws[kept - 1];
            let end = last.first + last.count;
            if last.chunk == d.chunk && last.lod == d.lod && d.first >= end && d.first - end <= gap {
                last.count = d.first + d.count - last.first;
                continue;
            }
        }
        draws[kept] = d;
        kept += 1;
    }
    draws.truncate(kept);
}

/// A picture of the pack as the GPU takes it, every level after the one above it: rows of four bytes a
/// texel, or the picture's own blocks of 4 × 4 texels.
fn rows(file: &mut PackFile, r: &Record) -> Result<Vec<u8>, String> {
    let levels = (r.levels as u32).max(1);
    let mut out = Vec::new();
    if r.format == texture::RGBA8 || r.block().is_some() {
        // (true colour is rows already, and blocks are what the GPU reads)
        for l in 0..levels {
            let (at, len) = r.level(l);
            let from = out.len();
            out.resize(from + len, 0);
            file.read_into(pack::TEXD, r.offset as usize + at, &mut out[from..])?;
        }
        return Ok(out);
    }
    // Another device's pack (a palette, or 16-bit tiles): read whole and taken apart a texel at a time.
    let (at, len) = r.level(levels - 1);
    let mut raw = vec![0u8; at + len];
    file.read_into(pack::TEXD, r.offset as usize, &mut raw)?;
    let local = Record { offset: 0, ..*r };
    for l in 0..levels {
        let (w, h) = ((r.width as usize >> l).max(1), (r.height as usize >> l).max(1));
        for y in 0..h {
            for x in 0..w {
                out.extend_from_slice(&local.texel(&raw, l, x, y));
            }
        }
    }
    Ok(out)
}

/// A picture in video memory in the GPU's own order of texels: the transfer engine turns each level's rows
/// into it, as PocketJS's uploader does for compressed blocks.
unsafe fn swizzled(staging: &mut Block, vram: &mut Arena, w: u32, h: u32, levels: u32, data: &[u8]) -> Result<Texture, String> {
    let size = |l: u32| ((w >> l).max(1), (h >> l).max(1));
    let total: usize = (0..levels).map(|l| (size(l).0 * size(l).1 * 4) as usize).sum();
    if data.len() < total {
        return Err(format!("a picture of {w} x {h} has {} bytes of {total}", data.len()));
    }
    let dst = vram.alloc(total, 512)?;
    let mut at = 0usize;
    for l in 0..levels {
        let (lw, lh) = size(l);
        let n = (lw * lh * 4) as usize;
        staging.reset();
        let src = staging.alloc(n, 16).ok_or("the staging block is smaller than a picture's level")?;
        core::ptr::copy_nonoverlapping(data.as_ptr().add(at), src, n);
        let format = g::SceGxmTransferFormat_SCE_GXM_TRANSFER_FORMAT_U8U8U8U8_ABGR;
        let r = g::sceGxmTransferCopy(
            lw,
            lh,
            0,
            0,
            g::SceGxmTransferColorKeyMode_SCE_GXM_TRANSFER_COLORKEY_NONE,
            format,
            g::SceGxmTransferType_SCE_GXM_TRANSFER_LINEAR,
            src.cast(),
            0,
            0,
            (lw * 4) as i32,
            format,
            g::SceGxmTransferType_SCE_GXM_TRANSFER_SWIZZLED,
            dst.add(at).cast(),
            0,
            0,
            (lw * 4) as i32,
            core::ptr::null_mut(),
            0,
            core::ptr::null_mut(),
        );
        if r < 0 {
            return Err(format!("sceGxmTransferCopy {lw} x {lh} 0x{:08x}", r as u32));
        }
        // (the staging block is written again for the next level)
        g::sceGxmTransferFinish();
        at += n;
    }
    let mut gxm: g::SceGxmTexture = core::mem::zeroed();
    let r = g::sceGxmTextureInitSwizzled(&mut gxm, dst.cast(), g::SceGxmTextureFormat_SCE_GXM_TEXTURE_FORMAT_U8U8U8U8_ABGR, w, h, levels);
    if r < 0 {
        return Err(format!("sceGxmTextureInitSwizzled {w} x {h} 0x{:08x}", r as u32));
    }
    Ok(Texture { gxm, width: w, height: h, mips: levels, format: Format::Rgba8, bytes: total })
}

impl Programs {
    /// Compiles the programs, or takes them from the cache on the memory card or from the package.
    ///
    /// # Safety
    /// Render thread, outside any scene.
    pub unsafe fn new(gpu: &mut Gpu, msaa: u32) -> Result<Programs, String> {
        const WORLD_V: &str = include_str!("../shaders/world_v.cg");
        const WORLD_F: &str = include_str!("../shaders/world_f.cg");
        const FLAT_V: &str = include_str!("../shaders/flat_v.cg");
        const FLAT_F: &str = include_str!("../shaders/flat_f.cg");
        let world_attrs = [("aPosition", 0u16, S16, 3u8), ("aShade", 6, U8N, 2), ("aUv", 8, S16, 2), ("aColor", 12, U8N, 4)];
        let mut street = Vec::with_capacity(variant::COUNT);
        for v in 0..variant::COUNT {
            let mut defines = String::new();
            for (bit, name) in [(variant::CUTOUT, "CUTOUT"), (variant::GROUND, "GROUND"), (variant::FLASH, "FLASH"), (variant::SUN, "SUN")] {
                if v & bit != 0 {
                    defines.push_str(&format!("#define {name}\n"));
                }
            }
            // (the vertex program has one switch: whether it sends the place's depth from the sun on)
            let prog = gpu.program(&format!("street{v}"), if v & variant::SUN != 0 { "#define SUN\n" } else { "" }, &defines, WORLD_V, WORLD_F, &[Stream { stride: 16, instanced: false, attrs: &world_attrs }], msaa, [Blend::Opaque, Blend::Alpha])?;
            street.push(Street {
                clip: prog.vs.param("uClip"),
                place: prog.vs.param("uPlace"),
                uv: prog.vs.param("uUv"),
                sun: prog.vs.param("uSun"),
                haze: prog.vs.param("uHaze"),
                look: prog.fs.param("uLook"),
                flash: prog.fs.param("uFlash"),
                flash_colour: prog.fs.param("uFlashColour"),
                depth: prog.vs.param("uDepth"),
                sun_edge: prog.fs.param("uSunEdge"),
                picture: prog.fs.sampler_index("uPicture").unwrap_or(0),
                shadow: prog.fs.sampler_index("uShadow").unwrap_or(1),
                depth_picture: prog.fs.sampler_index("uDepthPicture").unwrap_or(1),
                prog,
            });
        }
        let flat_attrs = [("aUv", 0u16, F32, 2u8), ("aColor", 8, U8N, 4), ("aPosition", 12, F32, 3)];
        let mut flat = |name: &str, defines: &str, blends: [Blend; 2]| -> Result<Flat, String> {
            let prog = gpu.program(name, "", defines, FLAT_V, FLAT_F, &[Stream { stride: 24, instanced: false, attrs: &flat_attrs }], msaa, blends)?;
            Ok(Flat { clip: prog.vs.param("uClip"), picture: prog.fs.sampler_index("uPicture").unwrap_or(0), prog })
        };
        let band = flat("band", "#define PICTURE\n", [Blend::Opaque, Blend::Opaque])?;
        let cap = flat("cap", "#define PLAIN\n", [Blend::Opaque, Blend::Opaque])?;
        let cover = flat("cover", "", [Blend::Alpha, Blend::Alpha])?;
        // (its colour comes times its alpha: the blend adds it to what is there)
        let glow = flat("glow", "#define GLOW\n", [Blend::Additive, Blend::Additive])?;

        Ok(Programs { street, band, cap, cover, glow })
    }
}

impl Gfx {
    /// Reads a map's street from its pack: the geometry and the pictures. `stage` is told each step. `linear`
    /// keeps the pictures as rows in video memory (PocketJS's own upload) instead of the GPU's order.
    ///
    /// # Safety
    /// Render thread, outside any scene; `vram` outlives what this returns.
    pub unsafe fn load(file: &mut PackFile, vram: &mut Arena, scene: Scene, linear: bool, stage: &mut dyn FnMut(&str, &PackFile)) -> Result<Gfx, String> {
        // ---- the sky: a band of pictures round the horizon, then a fan to the zenith
        let columns = scene.sky_columns as usize;
        let (mut sky_points, mut sky_order) = (Vec::<FxVertex>::new(), Vec::<u16>::new());
        let around = columns.max(1) * SKY_SEGMENTS;
        for c in 0..columns {
            let base = sky_points.len() as u16;
            for row in 0..=SKY_ROWS {
                let v = row as f32 / SKY_ROWS as f32;
                let elevation = scene.sky_top + (scene.sky_floor - scene.sky_top) * v;
                for k in 0..=SKY_SEGMENTS {
                    let u = k as f32 / SKY_SEGMENTS as f32;
                    let azimuth = (c as f32 + u) / columns as f32 * TAU;
                    let d = v3(cos(elevation) * cos(azimuth), sin(elevation), cos(elevation) * sin(azimuth)) * 200.0;
                    // (half a texel in from the picture's edges: its neighbour is another picture)
                    sky_points.push(FxVertex { uv: [0.002 + u * 0.996, 0.004 + v * 0.992], colour: [255, 255, 255, 255], pos: [d.x, d.y, d.z] });
                }
            }
            let stride = SKY_SEGMENTS as u16 + 1;
            for row in 0..SKY_ROWS as u16 {
                for k in 0..SKY_SEGMENTS as u16 {
                    let (a, b, c2, d) = (base + row * stride + k, base + row * stride + k + 1, base + (row + 1) * stride + k, base + (row + 1) * stride + k + 1);
                    sky_order.extend_from_slice(&[a, c2, b, b, c2, d]);
                }
            }
        }
        let z = abgr(scene.zenith).map(|c| (c * 255.0 + 0.5) as u8);
        let zenith = [z[0], z[1], z[2], 255];
        let first = sky_points.len() as u16;
        for k in 0..around {
            let azimuth = k as f32 / around as f32 * TAU;
            let d = v3(cos(scene.sky_top) * cos(azimuth), sin(scene.sky_top), cos(scene.sky_top) * sin(azimuth)) * 200.0;
            sky_points.push(FxVertex { uv: [0.0, 0.0], colour: zenith, pos: [d.x, d.y, d.z] });
        }
        sky_points.push(FxVertex { uv: [0.0, 0.0], colour: zenith, pos: [0.0, 200.0, 0.0] });
        for k in 0..around as u16 {
            sky_order.extend_from_slice(&[first + k, first + (k + 1) % around as u16, first + around as u16]);
        }
        // The squares of the effects and the flash: four vertices each, two triangles.
        let quads: Vec<u16> = (0..(fx::VERTICES / 4 + 1) as u16).flat_map(|q| [q * 4, q * 4 + 1, q * 4 + 2, q * 4, q * 4 + 2, q * 4 + 3]).collect();

        // ---- the geometry, in memory the GPU reads
        stage("Reading the street", file);
        let vertex_count = file.size(pack::WVTX)? / core::mem::size_of::<WorldVertex>();
        let index_bytes = file.size(pack::WIDX)?;
        let pad = |n: usize| (n + 63) & !63;
        let sizes = [vertex_count * 16, index_bytes, sky_points.len() * 24, sky_order.len() * 2, quads.len() * 2];
        let total: usize = sizes.iter().map(|&n| pad(n)).sum();
        let mut block = Block::with_access(Kind::Main, total, false)?;
        let mut at = [core::ptr::null_mut::<u8>(); 5];
        for (k, &n) in sizes.iter().enumerate() {
            at[k] = block.alloc(pad(n), 64).ok_or("the geometry block")?;
        }
        // The sun's depth picture and the vertices' colours with the sun off, in a pack that has them.
        let sun_depth = if file.has(pack::SUND) && file.size(pack::WSHD)? == vertex_count * 2 { file.records::<pack::SunDepth>(pack::SUND)?.first().copied() } else { None };
        let mut part = vec![0u8; PART * core::mem::size_of::<WorldVertex>()];
        let mut shades = vec![0u8; if sun_depth.is_some() { PART * 2 } else { 0 }];
        let mut done = 0usize;
        // Five bits of red, six of green, five of blue, red the lowest.
        let wide = |c: u16| -> [u8; 3] { [((c & 31) as u32 * 255 / 31) as u8, (((c >> 5) & 63) as u32 * 255 / 63) as u8, ((c >> 11) as u32 * 255 / 31) as u8] };
        while done < vertex_count {
            let n = (vertex_count - done).min(PART);
            let bytes = &mut part[..n * core::mem::size_of::<WorldVertex>()];
            file.read_into(pack::WVTX, done * core::mem::size_of::<WorldVertex>(), bytes)?;
            if sun_depth.is_some() {
                file.read_into(pack::WSHD, done * 2, &mut shades[..n * 2])?;
            }
            let out = core::slice::from_raw_parts_mut(at[0].add(done * 16) as *mut Vertex, n);
            for (i, o) in out.iter_mut().enumerate() {
                let v: WorldVertex = pack::read(bytes, i * core::mem::size_of::<WorldVertex>()).unwrap_or_default();
                let lit = wide(v.colour);
                let off = if sun_depth.is_some() { wide(u16::from_le_bytes([shades[i * 2], shades[i * 2 + 1]])) } else { lit };
                *o = Vertex { pos: v.pos, shade: [off[0], off[1]], uv: v.uv, colour: [lit[0], lit[1], lit[2], off[2]] };
            }
            done += n;
            if done % (PART * 8) == 0 {
                stage("Reading the street", file);
            }
        }
        drop(part);
        let mut done = 0usize;
        while done < index_bytes {
            let n = (index_bytes - done).min(2 << 20);
            file.read_into(pack::WIDX, done, core::slice::from_raw_parts_mut(at[1].add(done), n))?;
            done += n;
            stage("Reading the street", file);
        }
        core::ptr::copy_nonoverlapping(sky_points.as_ptr() as *const u8, at[2], sizes[2]);
        core::ptr::copy_nonoverlapping(sky_order.as_ptr() as *const u8, at[3], sizes[3]);
        core::ptr::copy_nonoverlapping(quads.as_ptr() as *const u8, at[4], sizes[4]);

        // ---- the pictures
        let records: Vec<Record> = file.records(pack::TEXT)?;
        let largest = records.iter().map(|r| r.width as usize * r.height as usize * 4).max().unwrap_or(0);
        let mut staging = Block::new(Kind::Main, largest.max(4096))?;
        // (the uploader stages a level of blocks whole before the transfer engine reorders it)
        let staged = records.iter().filter(|r| r.block().is_some()).map(|r| r.level(0).1).max().unwrap_or(0);
        let mut up = Uploader::new(staged.max(64 * 1024))?;
        let mut pictures = Vec::with_capacity(records.len());
        let mut sun = (Vec::new(), 0, 0);
        let mut picture_bytes = 0usize;
        for (number, r) in records.iter().enumerate() {
            if number % 4 == 0 {
                stage("Reading the pictures", file);
            }
            let (w, h, levels) = (r.width as u32, r.height as u32, (r.levels as u32).max(1));
            if r.format == texture::D16 {
                // The sun's depths: one number of 16 bits a texel, as rows, mixed between four texels (a
                // flat face's depth then follows the face, and its own shadow does not band it).
                let bytes = (w * h * 2) as usize;
                let dst = vram.alloc(bytes, 512)?;
                file.read_into(pack::TEXD, r.offset as usize, core::slice::from_raw_parts_mut(dst, bytes))?;
                let mut gxm: g::SceGxmTexture = core::mem::zeroed();
                let made = g::sceGxmTextureInitLinear(&mut gxm, dst.cast(), g::SceGxmTextureFormat_SCE_GXM_TEXTURE_FORMAT_U16_R, w, h, 1);
                if made < 0 {
                    return Err(format!("sceGxmTextureInitLinear {w} x {h} of 16 bits 0x{:08x}", made as u32));
                }
                let mut t = Texture { gxm, width: w, height: h, mips: 1, format: Format::Rgba8, bytes };
                t.set_wrap(Wrap::Clamp, Wrap::Clamp);
                t.set_filter(true, false);
                picture_bytes += bytes;
                pictures.push(t);
                continue;
            }
            let data = rows(file, r)?;
            if number as u32 == scene.shadow_texture {
                // The picture's green: one half is all of the sun.
                sun = (data[..(w * h * 4) as usize].chunks_exact(4).map(|p| ((p[1] as u32 * 255) / 127).min(255) as u8).collect(), w as usize, h as usize);
            }
            // The transfer engine refuses a copy of 2048 texels a side into the GPU's order (0x805b0003 on the
            // console); 512 goes through. A larger picture stays as rows: the ground's shadow, which has one
            // level and is not repeated.
            let mut t = match r.format {
                // A surface's picture in the GPU's blocks: PocketJS's uploader puts the blocks in its order.
                texture::BC1 => up.texture(vram, Format::Bc1, w, h, levels, &data)?,
                texture::BC3 => up.texture(vram, Format::Bc3, w, h, levels, &data)?,
                _ if linear || w.max(h) > SWIZZLED_SIDE => up.texture(vram, Format::Rgba8, w, h, levels, &data)?,
                _ => swizzled(&mut staging, vram, w, h, levels, &data)?,
            };
            let wrap = if r.flags & texture::CLAMP != 0 { Wrap::Clamp } else { Wrap::Repeat };
            t.set_wrap(wrap, wrap);
            // Between two texels, and between two levels: the GE and the PICA200 take the nearest level.
            t.set_filter(true, true);
            picture_bytes += t.bytes;
            pictures.push(t);
        }
        up.free();
        g::sceGxmTransferFinish();
        staging.free();

        Ok(Gfx {
            scene,
            vertices: at[0],
            indices: at[1] as *const u16,
            sky_vertices: at[2],
            sky_indices: at[3] as *const u16,
            sky_column: (SKY_SEGMENTS * SKY_ROWS * 6) as u32,
            sky_cap: (around * 3) as u32,
            quads: at[4] as *const u16,
            geometry: block,
            pictures,
            sun,
            glass: Vec::new(),
            sun_depth,
            geometry_bytes: total,
            picture_bytes,
            vertex_count,
            index_count: index_bytes / 2,
        })
    }

    /// Gives the geometry's memory back; the pictures go with their arena.
    ///
    /// # Safety
    /// The GPU has drawn the last frame that read it.
    pub unsafe fn free(self) {
        self.geometry.free();
    }

    /// How much of the sun reaches the ground at a place, 0…1: a texel of the ground's shadow picture.
    pub fn sunlit(&self, x: f32, z: f32) -> f32 {
        let (r, (lit, w, h)) = (&self.scene.shadow_rows, &self.sun);
        let (u, v) = (x * r[0][0] + z * r[0][1] + r[0][2], x * r[1][0] + z * r[1][1] + r[1][2]);
        if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) || lit.is_empty() {
            return 1.0;
        }
        lit[((v * *h as f32) as usize).min(h - 1) * w + ((u * *w as f32) as usize).min(w - 1)] as f32 / 255.0
    }

    /// The whole screen as the street's part of the depth range, or as the held weapon's.
    pub unsafe fn viewport(ctx: *mut g::SceGxmContext, held: bool) {
        let (from, to) = if held { (0.0, HELD_DEPTH) } else { (HELD_DEPTH, 1.0) };
        g::sceGxmSetViewport(ctx, WIDTH * 0.5, WIDTH * 0.5, HEIGHT * 0.5, -HEIGHT * 0.5, from, to - from);
        g::sceGxmSetRegionClip(ctx, g::SceGxmRegionClipMode_SCE_GXM_REGION_CLIP_OUTSIDE, 0, 0, WIDTH as u32 - 1, HEIGHT as u32 - 1);
    }

    unsafe fn picture(&self, ctx: *mut g::SceGxmContext, unit: u32, number: u32) {
        if let Some(t) = self.pictures.get(number as usize) {
            g::sceGxmSetFragmentTexture(ctx, unit, &t.gxm);
        }
    }

    /// The haze's colour over the whole screen, the sky, the street and its glass.
    ///
    /// # Safety
    /// Render thread, inside the display scene.
    pub unsafe fn street(&mut self, ctx: *mut g::SceGxmContext, p: &Programs, chunks: &[Chunk], view: &View) -> Drawn {
        let scene = self.scene;
        let mut drawn = Drawn::default();
        let haze = abgr(scene.haze_colour);
        Self::viewport(ctx, false);
        // What no surface and no sky covers has the haze's colour.
        gpu::state_overlay(ctx, false);
        g::vita2d_set_clear_color(scene.haze_colour | 0xff00_0000);
        g::vita2d_clear_screen();

        if view.look.sky && scene.sky_columns > 0 {
            gpu::state_overlay(ctx, false);
            p.band.prog.bind(ctx, false);
            Uniforms::vertex(ctx).set(p.band.clip, &view.eye_turn);
            g::sceGxmSetVertexStream(ctx, 0, self.sky_vertices.cast());
            for c in 0..scene.sky_columns {
                self.picture(ctx, p.band.picture, scene.sky_texture + c);
                g::sceGxmDraw(ctx, g::SceGxmPrimitiveType_SCE_GXM_PRIMITIVE_TRIANGLES, g::SceGxmIndexFormat_SCE_GXM_INDEX_FORMAT_U16, self.sky_indices.add((c * self.sky_column) as usize).cast(), self.sky_column);
            }
            p.cap.prog.bind(ctx, false);
            Uniforms::vertex(ctx).set(p.cap.clip, &view.eye_turn);
            g::sceGxmSetVertexStream(ctx, 0, self.sky_vertices.cast());
            g::sceGxmDraw(ctx, g::SceGxmPrimitiveType_SCE_GXM_PRIMITIVE_TRIANGLES, g::SceGxmIndexFormat_SCE_GXM_INDEX_FORMAT_U16, self.sky_indices.add((scene.sky_columns * self.sky_column) as usize).cast(), self.sky_cap);
            drawn.draws += scene.sky_columns + 1;
        }

        let hazy = view.look.haze && scene.haze[1] > scene.haze[0];
        let haze_v = [scene.haze[0], if hazy { 1.0 / (scene.haze[1] - scene.haze[0]) } else { 0.0 }, hazy as u32 as f32, 0.0];
        let sun_rows = [scene.shadow_rows[0][0], scene.shadow_rows[0][1], scene.shadow_rows[0][2], 0.0, scene.shadow_rows[1][0], scene.shadow_rows[1][1], scene.shadow_rows[1][2], 0.0];
        let flash = view.flash.filter(|_| view.look.flash);
        let flash_bit = if flash.is_some() { variant::FLASH } else { 0 };
        let sun_depth = self.sun_depth.filter(|_| view.look.shadows && view.look.sun);
        let quantum = scene.quantum;

        // One run of a chunk's level. A chunk's own numbers go to the vertex program once, with its first run.
        let run = |gfx: &Gfx, s: &Street, d: &Draw, c: &Chunk, fresh: bool| {
            let level = c.lod(d.lod as usize);
            if fresh {
                let centre = scene.cell_centre(c.cell as u32);
                let v = Uniforms::vertex(ctx);
                v.set(s.clip, &mat::with_bounds(&view.world, centre, [quantum; 3]));
                v.set(s.place, &[centre[0], centre[1], centre[2], quantum]);
                v.set(s.uv, &[c.uv_scale[0] / 32768.0, c.uv_scale[1] / 32768.0, c.uv_offset[0], c.uv_offset[1]]);
                v.set(s.sun, &sun_rows);
                v.set(s.haze, &haze_v);
                if let Some(sun) = sun_depth.filter(|_| !s.depth.is_null()) {
                    v.set(s.depth, &[sun.rows[0][0], sun.rows[0][1], sun.rows[0][2], sun.rows[0][3], sun.rows[1][0], sun.rows[1][1], sun.rows[1][2], sun.rows[1][3], sun.rows[2][0], sun.rows[2][1], sun.rows[2][2], sun.rows[2][3]]);
                }
            }
            g::sceGxmSetVertexStream(ctx, 0, gfx.vertices.add(level.vertex as usize * 16).cast());
            g::sceGxmDraw(ctx, g::SceGxmPrimitiveType_SCE_GXM_PRIMITIVE_TRIANGLES, g::SceGxmIndexFormat_SCE_GXM_INDEX_FORMAT_U16, gfx.indices.add((level.index + d.first) as usize).cast(), d.count);
        };
        // The numbers a fragment program takes: the haze's colour and how opaque the surface is, and the flash.
        let fragment = |s: &Street, opacity: f32| {
            let f = Uniforms::fragment(ctx);
            f.set(s.look, &[haze[0], haze[1], haze[2], opacity]);
            if let Some((at, strength)) = flash {
                f.set(s.flash, &[at.x, at.y, at.z, strength]);
                f.set(s.flash_colour, &[1.0, 0.8, 0.5, 0.0]);
            }
            // (a program that does not take the sun's depth picture has no such number)
            if let Some(sun) = sun_depth {
                // (lit up to `bias` past the picture's depth, then out over `edge`)
                f.set(s.sun_edge, &[sun.bias + sun.edge, 1.0 / sun.edge.max(1e-9), 0.0, 0.0]);
            }
        };

        // ---- the solid chunks, in the pack's order: one picture after another
        let (mut program, mut bound, mut record, mut sides) = (usize::MAX, u32::MAX, (u32::MAX, u8::MAX), Cull::None);
        gpu::state_opaque(ctx, Cull::None);
        self.glass.clear();
        for d in view.draws {
            let c = &chunks[d.chunk as usize];
            if c.flags & chunk::BLEND != 0 {
                self.glass.push((mat::box_distance2(view.eye, &c.min, &c.max), *d));
                continue;
            }
            let want = (if c.flags & chunk::CUTOUT != 0 { variant::CUTOUT } else { 0 })
                | (if view.look.shadows && (c.flags & chunk::GROUND != 0 || view.look.walls) { variant::GROUND } else { 0 })
                // (what is neither ground nor a cut-out keeps the sun's light apart in such a pack)
                | (if sun_depth.is_some() && c.flags & (chunk::GROUND | chunk::CUTOUT) == 0 { variant::SUN } else { 0 })
                | flash_bit;
            let cull = view.look.cull(c.flags & chunk::TWO_SIDED != 0);
            if want != program {
                program = want;
                let s = &p.street[want];
                s.prog.bind(ctx, false);
                fragment(s, 1.0);
                if want & variant::GROUND != 0 {
                    self.picture(ctx, s.shadow, scene.shadow_texture);
                }
                if let Some(sun) = sun_depth.filter(|_| want & variant::SUN != 0) {
                    self.picture(ctx, s.depth_picture, sun.texture);
                }
                // (a program's own numbers and pictures go with it)
                bound = u32::MAX;
                record = (u32::MAX, u8::MAX);
                gpu::state_opaque(ctx, cull);
                sides = cull;
            }
            if cull != sides {
                sides = cull;
                gpu::state_opaque(ctx, cull);
            }
            let s = &p.street[program];
            if c.texture as u32 != bound {
                bound = c.texture as u32;
                self.picture(ctx, s.picture, bound);
            }
            let fresh = record.0 != d.chunk as u32;
            record = (d.chunk as u32, d.lod);
            run(self, s, d, c, fresh);
            drawn.draws += 1;
            drawn.triangles += d.count / 3;
        }

        // ---- the glass goes over everything solid, the farthest first
        if !self.glass.is_empty() {
            self.glass.sort_by(|a, b| b.0.total_cmp(&a.0));
            gpu::state_overlay(ctx, true);
            let mut program = usize::MAX;
            for (_, d) in &self.glass {
                let c = &chunks[d.chunk as usize];
                let want = (if c.flags & chunk::CUTOUT != 0 { variant::CUTOUT } else { 0 }) | flash_bit;
                let s = &p.street[want];
                if want != program {
                    program = want;
                    s.prog.bind(ctx, true);
                }
                fragment(s, c.opacity as f32 / 255.0);
                self.picture(ctx, s.picture, c.texture as u32);
                g::sceGxmSetCullMode(
                    ctx,
                    match view.look.cull(c.flags & chunk::TWO_SIDED != 0) {
                        Cull::Cw => g::SceGxmCullMode_SCE_GXM_CULL_CW,
                        Cull::Ccw => g::SceGxmCullMode_SCE_GXM_CULL_CCW,
                        Cull::None => g::SceGxmCullMode_SCE_GXM_CULL_NONE,
                    },
                );
                run(self, s, d, c, true);
                drawn.draws += 1;
                drawn.triangles += d.count / 3;
            }
        }
        drawn
    }

    /// The effects' squares: first those that cover what is behind them, then those that add their light.
    /// `vertices` holds both, in memory the GPU reads until the frame is drawn.
    ///
    /// # Safety
    /// Render thread, inside the display scene.
    pub unsafe fn effects(&self, ctx: *mut g::SceGxmContext, p: &Programs, eye_turn: &Mat4, vertices: *const FxVertex, covering: usize, glowing: usize) -> Drawn {
        let mut drawn = Drawn::default();
        if covering + glowing == 0 {
            return drawn;
        }
        gpu::state_overlay(ctx, true);
        for (flat, from, count) in [(&p.cover, 0, covering), (&p.glow, covering, glowing)] {
            if count == 0 {
                continue;
            }
            flat.prog.bind(ctx, false);
            Uniforms::vertex(ctx).set(flat.clip, eye_turn);
            self.picture(ctx, flat.picture, self.scene.fx_texture);
            g::sceGxmSetVertexStream(ctx, 0, vertices.add(from).cast());
            g::sceGxmDraw(ctx, g::SceGxmPrimitiveType_SCE_GXM_PRIMITIVE_TRIANGLES, g::SceGxmIndexFormat_SCE_GXM_INDEX_FORMAT_U16, self.quads.cast(), (count / 4 * 6) as u32);
            drawn.draws += 1;
            drawn.triangles += (count / 2) as u32;
        }
        drawn
    }

    /// The muzzle's flash: one square that adds its light, in the held weapon's frustum and over it.
    ///
    /// # Safety
    /// Render thread, inside the display scene, the viewport the held weapon's.
    pub unsafe fn flash(&self, ctx: *mut g::SceGxmContext, p: &Programs, held: &Mat4, square: *const FxVertex) {
        gpu::state_overlay(ctx, false);
        p.glow.prog.bind(ctx, false);
        Uniforms::vertex(ctx).set(p.glow.clip, held);
        self.picture(ctx, p.glow.picture, self.scene.fx_texture);
        g::sceGxmSetVertexStream(ctx, 0, square.cast());
        g::sceGxmDraw(ctx, g::SceGxmPrimitiveType_SCE_GXM_PRIMITIVE_TRIANGLES, g::SceGxmIndexFormat_SCE_GXM_INDEX_FORMAT_U16, self.quads.cast(), 6);
    }
}
