//! The renderer: what the PSP's GE draws from the pack (`psp/src/gfx.rs`,
//! `psp/src/figures.rs`), as WGSL programs (`shaders/`), and the passes of a frame.
//!
//! A frame is one pass into a target of the screen's size:
//!
//! 1. the sky: a band of pictures round the horizon and the zenith's colour
//!    over it, with no depth;
//! 2. the street: a texel times the corner's baked colour, doubled, under the
//!    haze; then the ground once more under its shadow picture; then the
//!    glass, the farthest first;
//! 3. the soldiers, the effects, and the held weapon in a share of the depth
//!    range of its own.
//!
//! A second pass puts the target on the screen in the PSP display's 16-bit
//! colours with the GE's dither. The interface's picture goes over that
//! (`pocket_web_wgpu::overlay`).
//!
//! The GE needs two depth ranges for the street, cuts large triangles on the
//! CPU and holds a frame to a number of triangles (see `psp/src/gfx.rs`). A
//! depth buffer of 24 bits, a rasterizer that clips and this GPU need none of
//! the three.
//!
//! The PS Vita's pack (`profiles/vita60.json`: pictures in the GPU's blocks or
//! in true colour) is drawn as that console's own renderer draws it
//! (`vita/src/world.rs`, `vita/shaders/`): four samples a pixel, a picture's
//! texels mixed between two of its levels, the ground's shadow in the street's
//! own pass, the sun's shadow on walls and things a pixel at a time from the
//! pack's depth picture, and a muzzle's light on what is near it.

use duty_pack::{self as pack, chunk, texture, Chunk, Scene, Texture, WorldVertex};
use duty_sim::fx::{self, FxVertex};
use duty_sim::mat::Mat4;
use duty_sim::math::*;
use duty_sim::view::Draw;
use pocket_web_wgpu::gpu::{Gpu, Screen, DEPTH};
use pocket_web_wgpu::overlay::Overlay;
use pocket_web_wgpu::wgpu::{self, util::DeviceExt, TextureFormat};

use crate::figures::{Drawn, Figures, RECORD};
use crate::pack::{bytes_of, level_size, records, rgba, section};

/// The frame's target: colours as the display shows them.
const TARGET: TextureFormat = TextureFormat::Rgba8Unorm;
/// A record of a uniform buffer starts on a multiple of this (WebGPU's `minUniformBufferOffsetAlignment`).
const ALIGN: u64 = 256;
const SCENE_FLOATS: usize = 16 * 3 + 4 * 4 + 4 * 6;
const CHUNK_FLOATS: usize = 12;
const FIGURE_STRIDE: u64 = ((RECORD * 4) as u64).div_ceil(ALIGN) * ALIGN;
/// Batches of soldiers and of the held weapon a frame draws at most.
const FIGURE_DRAWS: u64 = 1024;
/// The share of the depth range the held weapon has to itself, as on the PSP (4096 of 65536).
const HELD_DEPTH: f32 = 0.0625;

const SKY_SEGMENTS: usize = 8;
const SKY_ROWS: usize = 6;

/// What a development host can leave out of a frame, and the display's colours.
#[derive(Clone, Copy, Debug)]
pub struct Look {
    pub sky: bool,
    pub haze: bool,
    pub shadows: bool,
    pub effects: bool,
    pub figures: bool,
    /// The PSP display's 16-bit colours with the GE's dither; without it, the frame as it was drawn.
    pub sixteen: bool,
}

impl Look {
    pub const DEFAULT: Look = Look { sky: true, haze: true, shadows: true, effects: true, figures: true, sixteen: true };
}

/// What a frame draws.
pub struct View<'a> {
    /// Projection × view, and projection × the view's turn alone.
    pub world: Mat4,
    pub eye_turn: Mat4,
    pub eye: V3,
    pub draws: &'a [Draw],
    /// The batches' records, and the batches: the soldiers' first, then from `held_from` the held weapon's.
    pub records: &'a [f32],
    pub figures: &'a [Drawn],
    pub held_from: usize,
    /// The held weapon's frustum, when it is drawn.
    pub held: Option<Mat4>,
    /// The effects' squares: those that cover first, then those that glow; positions counted from the eye.
    pub effects: &'a [FxVertex],
    pub covering: usize,
    pub glowing: usize,
    /// The flash at the muzzle, in the held weapon's own space.
    pub flash: Option<[FxVertex; 4]>,
    /// The muzzle's light on the street, in the PS Vita's pack: where in the world, and how strong.
    pub light: Option<(V3, f32)>,
    pub look: Look,
}

/// The targets of one screen size.
struct Targets {
    width: u32,
    height: u32,
    /// What the scene is drawn into: several samples a pixel for the PS Vita's pack, resolved into `resolve`.
    colour: wgpu::TextureView,
    resolve: Option<wgpu::TextureView>,
    depth: wgpu::TextureView,
    group: wgpu::BindGroup,
}

pub struct Renderer {
    solid: [wgpu::RenderPipeline; 2],
    glass: [wgpu::RenderPipeline; 2],
    shadow: wgpu::RenderPipeline,
    sky: wgpu::RenderPipeline,
    cap: wgpu::RenderPipeline,
    cover: wgpu::RenderPipeline,
    glow: wgpu::RenderPipeline,
    flash: wgpu::RenderPipeline,
    skin: wgpu::RenderPipeline,
    present: wgpu::RenderPipeline,
    post_layout: wgpu::BindGroupLayout,
    scene_buf: wgpu::Buffer,
    scene_group: wgpu::BindGroup,
    chunk_group: wgpu::BindGroup,
    figure_buf: wgpu::Buffer,
    figure_group: wgpu::BindGroup,
    post_buf: wgpu::Buffer,
    /// One a picture of the pack, in the pack's order.
    pictures: Vec<wgpu::BindGroup>,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    skin_vb: wgpu::Buffer,
    skin_ib: wgpu::Buffer,
    sky_vb: wgpu::Buffer,
    sky_ib: wgpu::Buffer,
    /// Indices of one column of the sky's band, and of the cap over it.
    sky_column: u32,
    sky_cap: u32,
    flat_vb: wgpu::Buffer,
    quad_ib: wgpu::Buffer,
    targets: Option<Targets>,
    scene: Scene,
    /// The ground's shadow picture: how much of the sun is left at each texel, 0…255, and its size.
    sun: (Vec<u8>, usize, usize),
    glass_order: Vec<(f32, Draw)>,
    scratch: Vec<u8>,
    /// Bytes of geometry and texels handed to the GPU, for the status.
    pub bytes: u64,
    /// The pack is the PS Vita's: drawn as that console draws it.
    pub rich: bool,
    /// Samples a pixel of the scene's target.
    pub samples: u32,
    /// The sun's depth picture and its numbers, in a pack whose corners keep the sun's light apart.
    pub sun_depth: Option<pack::SunDepth>,
    /// How the pack's block pictures reached the GPU: "" (it has none), "gpu" (as blocks) or "decoded" (as
    /// rows of true colour, on a GPU that reads no blocks).
    pub blocks: &'static str,
    /// The ground's shadow picture and the sun's depths, for the PS Vita's street.
    light_group: Option<wgpu::BindGroup>,
}

/// A colour of the pack (alpha, blue, green, red in a word) as 0…1.
fn colour(abgr: u32) -> [f32; 3] {
    [(abgr & 255) as f32 / 255.0, ((abgr >> 8) & 255) as f32 / 255.0, ((abgr >> 16) & 255) as f32 / 255.0]
}

fn floats(v: &[f32]) -> &[u8] {
    bytes_of(v)
}

impl Renderer {
    /// The programs for a screen of `format`, and the pack's geometry and pictures on the GPU.
    pub fn new(gpu: &Gpu, format: TextureFormat, bytes: &[u8], scene: &Scene, chunks: &[Chunk], figures: &Figures) -> Result<Renderer, String> {
        let device = &gpu.device;
        let mut held = 0u64;
        // The pictures say whose pack this is: the PSP's are palettes, the PS Vita's blocks and true colour.
        let textures: Vec<Texture> = records(bytes, pack::TEXT)?;
        let texels = section(bytes, pack::TEXD)?;
        let stored: Vec<WorldVertex> = records(bytes, pack::WVTX)?;
        let rich = textures.iter().any(|t| t.format != texture::T8);
        let samples = if rich { 4 } else { 1 };
        // (the colours with the sun off: one of 16 bits a corner)
        let shades = pack::section(bytes, pack::WSHD).filter(|s| s.len() == stored.len() * 2);
        let sun_depth = match (shades, pack::section(bytes, pack::SUND)) {
            (Some(_), Some(_)) => records::<pack::SunDepth>(bytes, pack::SUND)?.first().copied().filter(|sun| textures.get(sun.texture as usize).is_some_and(|t| t.format == texture::D16)),
            _ => None,
        };
        let common = include_str!("shaders/scene.wgsl");
        let module = |label: &str, source: &str| device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(label), source: wgpu::ShaderSource::Wgsl(source.into()) });
        let world = module("world", &format!("{common}{}", include_str!("shaders/world.wgsl")));
        let skin = module("skin", &format!("{common}{}", include_str!("shaders/skin.wgsl")));
        let post = module("post", include_str!("shaders/post.wgsl"));

        // ---- layouts
        let uniform = |dynamic: bool| wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: dynamic, min_binding_size: None };
        let filtered = wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false };
        let sampler = wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering);
        let entry = |binding: u32, visibility: wgpu::ShaderStages, ty: wgpu::BindingType| wgpu::BindGroupLayoutEntry { binding, visibility, ty, count: None };
        let both = wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT;
        let scene_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("scene"), entries: &[entry(0, both, uniform(false))] });
        let picture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("picture"), entries: &[entry(0, wgpu::ShaderStages::FRAGMENT, filtered), entry(1, wgpu::ShaderStages::FRAGMENT, sampler)] });
        let record_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("record"), entries: &[entry(0, both, uniform(true))] });
        let post_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("post"), entries: &[entry(0, wgpu::ShaderStages::FRAGMENT, uniform(false)), entry(1, wgpu::ShaderStages::FRAGMENT, wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: false }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false })] });
        let layout = |label: &str, groups: &[&wgpu::BindGroupLayout]| device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some(label), bind_group_layouts: groups, push_constant_ranges: &[] });
        let world_layout = layout("world", &[&scene_layout, &picture_layout, &record_layout]);
        // (the PS Vita's street also reads the ground's shadow picture and the sun's depths)
        let light_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("light"),
            entries: &[entry(0, wgpu::ShaderStages::FRAGMENT, filtered), entry(1, wgpu::ShaderStages::FRAGMENT, sampler), entry(2, wgpu::ShaderStages::FRAGMENT, wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Uint, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false })],
        });
        let rich_layout = layout("street", &[&scene_layout, &picture_layout, &record_layout, &light_layout]);
        let flat_layout = layout("flat", &[&scene_layout, &picture_layout]);
        let skin_layout = layout("skin", &[&scene_layout, &record_layout]);
        let present_layout = layout("present", &[&post_layout]);

        // ---- programs
        use wgpu::VertexFormat as F;
        let attr = |location: u32, offset: u64, format: F| wgpu::VertexAttribute { shader_location: location, offset, format };
        // (the layouts live as long as the pipelines made from them)
        let world_attrs: &'static [wgpu::VertexAttribute] = Box::leak(Box::new([attr(0, 0, F::Sint16x4), attr(1, 8, F::Sint16x2), attr(2, 12, F::Unorm8x4)]));
        let flat_attrs: &'static [wgpu::VertexAttribute] = Box::leak(Box::new([attr(0, 0, F::Float32x2), attr(1, 8, F::Unorm8x4), attr(2, 12, F::Float32x3)]));
        let skin_attrs: &'static [wgpu::VertexAttribute] = Box::leak(Box::new([attr(0, 0, F::Uint8x4), attr(1, 4, F::Unorm8x4), attr(2, 8, F::Sint8x4), attr(3, 12, F::Float32x3)]));
        let buffer = |stride: u64, attributes: &'static [wgpu::VertexAttribute]| wgpu::VertexBufferLayout { array_stride: stride, step_mode: wgpu::VertexStepMode::Vertex, attributes };
        use wgpu::BlendFactor as B;
        let mix = |src: B, dst: B| Some(wgpu::BlendState { color: wgpu::BlendComponent { src_factor: src, dst_factor: dst, operation: wgpu::BlendOperation::Add }, alpha: wgpu::BlendComponent { src_factor: B::Zero, dst_factor: B::One, operation: wgpu::BlendOperation::Add } });
        #[allow(clippy::too_many_arguments)]
        let pipeline = |label: &str, layout: &wgpu::PipelineLayout, module: &wgpu::ShaderModule, vertex: &str, fragment: &str, buffers: &[wgpu::VertexBufferLayout], to: TextureFormat, blend: Option<wgpu::BlendState>, cull: bool, depth: Option<(wgpu::CompareFunction, bool)>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
                vertex: wgpu::VertexState { module, entry_point: Some(vertex), compilation_options: Default::default(), buffers },
                fragment: Some(wgpu::FragmentState { module, entry_point: Some(fragment), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format: to, blend, write_mask: wgpu::ColorWrites::ALL })] }),
                primitive: wgpu::PrimitiveState { front_face: wgpu::FrontFace::Ccw, cull_mode: cull.then_some(wgpu::Face::Back), ..Default::default() },
                depth_stencil: depth.map(|(compare, write)| wgpu::DepthStencilState { format: DEPTH, depth_write_enabled: write, depth_compare: compare, stencil: Default::default(), bias: Default::default() }),
                // (everything drawn with a depth goes into the scene's target; the last pass goes onto the screen)
                multisample: wgpu::MultisampleState { count: if depth.is_some() { samples } else { 1 }, mask: !0, alpha_to_coverage_enabled: false },
                multiview: None,
                cache: None,
            })
        };
        use wgpu::CompareFunction::{Always, Equal, LessEqual};
        let street = |label: &str, fragment: &str, blend: Option<wgpu::BlendState>, cull: bool, depth: (wgpu::CompareFunction, bool)| pipeline(label, &world_layout, &world, "world_vertex", fragment, &[buffer(16, world_attrs)], TARGET, blend, cull, Some(depth));
        // The PS Vita's street is one program: its ground takes the shadow picture in the same pass.
        let street_rich = |label: &str, blend: Option<wgpu::BlendState>, cull: bool, depth: (wgpu::CompareFunction, bool)| pipeline(label, &rich_layout, &world, "rich_vertex", "rich_fragment", &[buffer(16, world_attrs)], TARGET, blend, cull, Some(depth));
        let solid = [true, false].map(|cull| if rich { street_rich("solid", None, cull, (LessEqual, true)) } else { street("solid", "world_fragment", None, cull, (LessEqual, true)) });
        // Glass: its colour by its opacity over what is behind it. The opacity is the draw's blend constant.
        let glass = [true, false].map(|cull| if rich { street_rich("glass", mix(B::Constant, B::OneMinusConstant), cull, (LessEqual, false)) } else { street("glass", "world_fragment", mix(B::Constant, B::OneMinusConstant), cull, (LessEqual, false)) });
        // The ground once more: what is there times the shadow picture, where the depth is the ground's own.
        let shadow = street("shadow", "shadow_fragment", mix(B::Dst, B::Zero), true, (Equal, false));
        let flat = |label: &str, vertex: &str, fragment: &str, blend: Option<wgpu::BlendState>, depth: wgpu::CompareFunction| pipeline(label, &flat_layout, &world, vertex, fragment, &[buffer(24, flat_attrs)], TARGET, blend, false, Some((depth, false)));
        let sky = flat("sky", "eye_vertex", "picture_fragment", None, Always);
        let cap = flat("cap", "eye_vertex", "colour_fragment", None, Always);
        let cover = flat("cover", "eye_vertex", "effect_fragment", mix(B::SrcAlpha, B::OneMinusSrcAlpha), LessEqual);
        let glow = flat("glow", "eye_vertex", "effect_fragment", mix(B::SrcAlpha, B::One), LessEqual);
        let flash = flat("flash", "held_vertex", "effect_fragment", mix(B::SrcAlpha, B::One), Always);
        let skin_program = pipeline("skin", &skin_layout, &skin, "skin_vertex", "skin_fragment", &[buffer(24, skin_attrs)], TARGET, None, true, Some((LessEqual, true)));
        let present = pipeline("present", &present_layout, &post, "corner", "present", &[], format, None, false, None);

        // ---- the pictures: indices into a palette in the PSP's pack, texels here; blocks and true colour in
        // the PS Vita's, which a GPU that reads blocks takes as they are
        let samplers = [wgpu::AddressMode::Repeat, wgpu::AddressMode::ClampToEdge].map(|wrap| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("picture"),
                address_mode_u: wrap,
                address_mode_v: wrap,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                // (the GE takes one level, the nearest in size, and mixes none; the PS Vita mixes two)
                mipmap_filter: if rich { wgpu::FilterMode::Linear } else { wgpu::FilterMode::Nearest },
                ..Default::default()
            })
        });
        let reads_blocks = gpu.features.contains(wgpu::Features::TEXTURE_COMPRESSION_BC);
        let mut blocks = "";
        let picture = |label: &str, format: TextureFormat, width: u32, height: u32, levels: u32| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: levels,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            })
        };
        let write = |to: &wgpu::Texture, level: u32, bytes: &[u8], row: u32, rows: u32, width: u32, height: u32| {
            gpu.queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture: to, mip_level: level, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                bytes,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(rows) },
                wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            );
        };
        let mut pictures = Vec::with_capacity(textures.len());
        // (the ground's shadow picture and the sun's depths are also read by the PS Vita's street)
        let (mut shadow_view, mut depth_view) = (None, None);
        let mut sun = (Vec::new(), 0, 0);
        for (number, t) in textures.iter().enumerate() {
            if t.format != texture::T8 && t.format != texture::RGBA8 && t.format != texture::D16 && t.block().is_none() {
                return Err(format!("picture {number} has format {} (this renderer reads the PSP's pack and the PS Vita's)", t.format));
            }
            let levels = (t.levels as u32).max(1);
            let (width, height) = (t.width as u32, t.height as u32);
            if t.format == texture::D16 {
                // The sun's depths: one number of 16 bits a texel, read as whole numbers (`sun_reached`).
                let (at, bytes) = t.level(0);
                let rows = texels.get(t.offset as usize + at..t.offset as usize + at + bytes).ok_or("a picture's texels are cut short")?;
                let made = picture("depths", TextureFormat::R16Uint, width, height, 1);
                write(&made, 0, rows, width * 2, height, width, height);
                held += rows.len() as u64;
                if sun_depth.is_some_and(|sun| sun.texture as usize == number) {
                    depth_view = Some(made.create_view(&Default::default()));
                }
                // (no surface draws from it: its number in the list is kept by a picture of one texel)
                let blank = picture("blank", TextureFormat::Rgba8Unorm, 1, 1, 1);
                write(&blank, 0, &[255; 4], 4, 1, 1, 1);
                pictures.push(device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("picture"),
                    layout: &picture_layout,
                    entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&blank.create_view(&Default::default())) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&samplers[1]) }],
                }));
                continue;
            }
            // A picture in blocks goes to a GPU that reads blocks as it is; to another as rows of true colour.
            let native = match t.format {
                texture::BC1 if reads_blocks => Some(TextureFormat::Bc1RgbaUnorm),
                texture::BC3 if reads_blocks => Some(TextureFormat::Bc3RgbaUnorm),
                _ => None,
            };
            if t.block().is_some() {
                blocks = if native.is_some() { "gpu" } else { "decoded" };
            }
            let made = picture("picture", native.unwrap_or(TextureFormat::Rgba8Unorm), width, height, levels);
            for level in 0..levels {
                let (w, h) = level_size(t, level);
                match (native, t.block()) {
                    (Some(_), Some(block)) => {
                        let (at, bytes) = t.level(level);
                        let rows = texels.get(t.offset as usize + at..t.offset as usize + at + bytes).ok_or("a picture's texels are cut short")?;
                        held += rows.len() as u64;
                        // (a level is written as whole blocks)
                        write(&made, level, rows, w.div_ceil(4) * block as u32, h.div_ceil(4), w.div_ceil(4) * 4, h.div_ceil(4) * 4);
                    }
                    _ => {
                        let rows = rgba(t, texels, level)?;
                        held += rows.len() as u64;
                        write(&made, level, &rows, w * 4, h, w, h);
                    }
                }
            }
            if number as u32 == scene.shadow_texture {
                // The palette's green (the picture's, in the PS Vita's pack): one half is all of the sun.
                let rows = rgba(t, texels, 0)?;
                sun = (rows.chunks_exact(4).map(|p| ((p[1] as u32 * 255) / 127).min(255) as u8).collect(), width as usize, height as usize);
                shadow_view = Some((made.create_view(&Default::default()), (t.flags & texture::CLAMP != 0) as usize));
            }
            pictures.push(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("picture"),
                layout: &picture_layout,
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&made.create_view(&Default::default())) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&samplers[(t.flags & texture::CLAMP != 0) as usize]) }],
            }));
        }
        for wanted in [scene.shadow_texture, scene.fx_texture, scene.sky_texture + scene.sky_columns.max(1) - 1] {
            if wanted as usize >= pictures.len() {
                return Err(format!("the scene names picture {wanted} and the pack has {}", pictures.len()));
            }
        }

        // ---- uniforms
        let uniforms = |label: &str, size: u64| device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let init = |label: &str, contents: &[u8], usage: wgpu::BufferUsages| device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some(label), contents, usage });
        let scene_buf = uniforms("scene", (SCENE_FLOATS * 4) as u64);
        let scene_group = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("scene"), layout: &scene_layout, entries: &[wgpu::BindGroupEntry { binding: 0, resource: scene_buf.as_entire_binding() }] });
        // A chunk's record never changes: where its cell is, its picture's scale, whether its clear texels are holes.
        let mut chunk_records = vec![0u8; chunks.len().max(1) * ALIGN as usize];
        for (i, c) in chunks.iter().enumerate() {
            let centre = scene.cell_centre(c.cell as u32);
            // (the PS Vita's street: ground takes the shadow picture, and what is neither ground nor a cut-out nor
            // glass keeps the sun's light apart, in a pack that has the sun's depths)
            let ground = rich && c.flags & chunk::GROUND != 0 && c.flags & chunk::BLEND == 0;
            let apart = sun_depth.is_some() && c.flags & (chunk::GROUND | chunk::CUTOUT | chunk::BLEND) == 0;
            let record: [f32; CHUNK_FLOATS] = [centre[0], centre[1], centre[2], scene.quantum, c.uv_scale[0], c.uv_scale[1], c.uv_offset[0], c.uv_offset[1], (c.flags & chunk::CUTOUT != 0) as u32 as f32, ground as u32 as f32, apart as u32 as f32, 0.0];
            chunk_records[i * ALIGN as usize..][..CHUNK_FLOATS * 4].copy_from_slice(floats(&record));
        }
        let chunk_buf = init("chunks", &chunk_records, wgpu::BufferUsages::UNIFORM);
        let record_group = |label: &str, buffer: &wgpu::Buffer, size: u64| {
            device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some(label), layout: &record_layout, entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer, offset: 0, size: wgpu::BufferSize::new(size) }) }] })
        };
        let chunk_group = record_group("chunks", &chunk_buf, (CHUNK_FLOATS * 4) as u64);
        let figure_buf = uniforms("figures", FIGURE_DRAWS * FIGURE_STRIDE);
        let figure_group = record_group("figures", &figure_buf, (RECORD * 4) as u64);
        let post_buf = uniforms("post", 16);
        let light_group = match (rich, shadow_view) {
            (true, Some((shadow, wrap))) => {
                // (a pack with no depth picture: one texel that nothing is asked of)
                let depths = depth_view.unwrap_or_else(|| picture("depths", TextureFormat::R16Uint, 1, 1, 1).create_view(&Default::default()));
                Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("light"),
                    layout: &light_layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&shadow) },
                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&samplers[wrap]) },
                        wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&depths) },
                    ],
                }))
            }
            (true, None) => return Err("the pack's scene names no shadow picture".into()),
            _ => None,
        };

        // ---- the street: a corner in the GE's order (texture, colour, position: 12 bytes) laid out on
        // four-byte boundaries, its colour of 16 bits as the eight a GE reads it as
        let mut corners = Vec::with_capacity(stored.len() * 16);
        // The PS Vita's corner (`Vertex` in vita/src/world.rs): five, six and five bits each widened to a byte's
        // whole range, with the colour the corner has with the sun off in the padding and the fourth byte
        // (the corner's own colour again in a pack that has none).
        let wide = |c: u16| -> [u8; 3] { [((c & 31) as u32 * 255 / 31) as u8, (((c >> 5) & 63) as u32 * 255 / 63) as u8, ((c >> 11) as u32 * 255 / 31) as u8] };
        for (i, v) in stored.iter().enumerate() {
            if rich {
                let lit = wide(v.colour);
                let off = if sun_depth.is_some() { shades.map_or(lit, |s| wide(u16::from_le_bytes([s[i * 2], s[i * 2 + 1]]))) } else { lit };
                for part in [v.pos[0], v.pos[1], v.pos[2]] {
                    corners.extend_from_slice(&part.to_le_bytes());
                }
                corners.extend_from_slice(&[off[0], off[1]]);
                for part in [v.uv[0], v.uv[1]] {
                    corners.extend_from_slice(&part.to_le_bytes());
                }
                corners.extend_from_slice(&[lit[0], lit[1], lit[2], off[2]]);
                continue;
            }
            let (r, g, b) = ((v.colour & 31) as u8, ((v.colour >> 5) & 63) as u8, (v.colour >> 11) as u8);
            for part in [v.pos[0], v.pos[1], v.pos[2], 0, v.uv[0], v.uv[1]] {
                corners.extend_from_slice(&part.to_le_bytes());
            }
            corners.extend_from_slice(&[(r << 3) | (r >> 2), (g << 2) | (g >> 4), (b << 3) | (b >> 2), 255]);
        }
        let vertices = init("street", &corners, wgpu::BufferUsages::VERTEX);
        let indices = init("street", section(bytes, pack::WIDX)?, wgpu::BufferUsages::INDEX);
        let skin_vb = init("figures", &figures.vertices, wgpu::BufferUsages::VERTEX);
        let skin_ib = init("figures", bytes_of(&figures.indices), wgpu::BufferUsages::INDEX);
        held += corners.len() as u64 + section(bytes, pack::WIDX)?.len() as u64 + figures.vertices.len() as u64 + figures.indices.len() as u64 * 2 + chunk_records.len() as u64;

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
        let z = colour(scene.zenith).map(|c| (c * 255.0 + 0.5) as u8);
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
        let sky_vb = init("sky", bytes_of(&sky_points), wgpu::BufferUsages::VERTEX);
        let sky_ib = init("sky", bytes_of(&sky_order), wgpu::BufferUsages::INDEX);

        // ---- the effects' squares and the flash, written each frame
        let flat_vb = device.create_buffer(&wgpu::BufferDescriptor { label: Some("effects"), size: ((fx::VERTICES + 4) * core::mem::size_of::<FxVertex>()) as u64, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let quads: Vec<u16> = (0..(fx::VERTICES / 4 + 1) as u16).flat_map(|q| [q * 4, q * 4 + 1, q * 4 + 2, q * 4, q * 4 + 2, q * 4 + 3]).collect();
        let quad_ib = init("squares", bytes_of(&quads), wgpu::BufferUsages::INDEX);

        Ok(Renderer {
            solid,
            glass,
            shadow,
            sky,
            cap,
            cover,
            glow,
            flash,
            skin: skin_program,
            present,
            post_layout,
            scene_buf,
            scene_group,
            chunk_group,
            figure_buf,
            figure_group,
            post_buf,
            pictures,
            vertices,
            indices,
            skin_vb,
            skin_ib,
            sky_vb,
            sky_ib,
            sky_column: (SKY_SEGMENTS * SKY_ROWS * 6) as u32,
            sky_cap: (around * 3) as u32,
            flat_vb,
            quad_ib,
            targets: None,
            scene: *scene,
            sun,
            glass_order: Vec::new(),
            scratch: Vec::new(),
            bytes: held,
            rich,
            samples,
            sun_depth,
            blocks,
            light_group,
        })
    }

    /// A texel of the ground's shadow picture at a place: how much of the sun is left there, 0…1.
    pub fn sunlit(&self, x: f32, z: f32) -> f32 {
        let (r, (lit, w, h)) = (&self.scene.shadow_rows, &self.sun);
        let (u, v) = (x * r[0][0] + z * r[0][1] + r[0][2], x * r[1][0] + z * r[1][1] + r[1][2]);
        if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) || lit.is_empty() {
            return 1.0;
        }
        lit[((v * *h as f32) as usize).min(h - 1) * w + ((u * *w as f32) as usize).min(w - 1)] as f32 / 255.0
    }

    fn targets(&mut self, gpu: &Gpu, width: u32, height: u32) {
        if self.targets.as_ref().is_some_and(|t| (t.width, t.height) == (width, height)) {
            return;
        }
        let target = |label: &str, format: TextureFormat, sampled: bool, samples: u32| {
            gpu.device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | if sampled { wgpu::TextureUsages::TEXTURE_BINDING } else { wgpu::TextureUsages::empty() },
                    view_formats: &[],
                })
                .create_view(&wgpu::TextureViewDescriptor::default())
        };
        // With several samples a pixel the scene's target is resolved into the picture the last pass reads.
        let depth = target("depth", DEPTH, false, self.samples);
        let (colour, resolve) = if self.samples > 1 { (target("samples", TARGET, false, self.samples), Some(target("frame", TARGET, true, 1))) } else { (target("frame", TARGET, true, 1), None) };
        let shown = resolve.as_ref().unwrap_or(&colour);
        let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("post"), layout: &self.post_layout, entries: &[wgpu::BindGroupEntry { binding: 0, resource: self.post_buf.as_entire_binding() }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(shown) }] });
        self.targets = Some(Targets { width, height, colour, resolve, depth, group });
    }

    /// Draws a frame onto `screen`, with the interface's picture over it. Returns the draws and the triangles.
    pub fn frame(&mut self, gpu: &Gpu, screen: &Screen, overlay: &Overlay, chunks: &[Chunk], view: &View) -> Result<(u32, u32), String> {
        let frame = screen.frame(gpu)?;
        self.targets(gpu, screen.width, screen.height);
        let scene = self.scene;
        let hazy = view.look.haze && scene.haze[1] > scene.haze[0];
        let haze = colour(scene.haze_colour);

        // ---- this frame's records
        let mut record = [0f32; SCENE_FLOATS];
        record[0..16].copy_from_slice(&view.world);
        record[16..32].copy_from_slice(&view.eye_turn);
        record[32..48].copy_from_slice(&view.held.unwrap_or(duty_sim::mat::IDENTITY));
        record[48..52].copy_from_slice(&[haze[0], haze[1], haze[2], 1.0]);
        record[52..56].copy_from_slice(&[scene.haze[0], scene.haze[1], hazy as u32 as f32, 0.0]);
        record[56..60].copy_from_slice(&[scene.shadow_rows[0][0], scene.shadow_rows[0][1], scene.shadow_rows[0][2], 0.0]);
        record[60..64].copy_from_slice(&[scene.shadow_rows[1][0], scene.shadow_rows[1][1], scene.shadow_rows[1][2], 0.0]);
        // The PS Vita's street: the sun's depth picture while the shadows are on, and a muzzle's light.
        let sun = self.sun_depth.filter(|_| view.look.shadows);
        if let Some(sun) = &sun {
            for (row, at) in sun.rows.iter().zip([64, 68, 72]) {
                record[at..at + 4].copy_from_slice(row);
            }
        }
        // (lit up to `bias` past the picture's depth, then out over `edge`)
        record[76..80].copy_from_slice(&[sun.map_or(0.0, |s| s.bias + s.edge), sun.map_or(0.0, |s| 1.0 / s.edge.max(1e-9)), view.look.shadows as u32 as f32, sun.is_some() as u32 as f32]);
        if let Some((at, strength)) = view.light.filter(|_| self.rich) {
            record[80..84].copy_from_slice(&[at.x, at.y, at.z, strength]);
            record[84..88].copy_from_slice(&[1.0, 0.8, 0.5, 0.0]);
        }
        gpu.queue.write_buffer(&self.scene_buf, 0, floats(&record));
        gpu.queue.write_buffer(&self.post_buf, 0, floats(&[view.look.sixteen as u32 as f32, 0.0, 0.0, 0.0]));
        let figures = if view.look.figures { &view.figures[..view.figures.len().min(FIGURE_DRAWS as usize)] } else { &[] };
        if !figures.is_empty() {
            self.scratch.clear();
            self.scratch.resize(figures.len() * FIGURE_STRIDE as usize, 0);
            for (i, drawn) in figures.iter().enumerate() {
                let from = drawn.record as usize * RECORD;
                self.scratch[i * FIGURE_STRIDE as usize..][..RECORD * 4].copy_from_slice(floats(&view.records[from..from + RECORD]));
            }
            gpu.queue.write_buffer(&self.figure_buf, 0, &self.scratch);
        }
        let squares = if view.look.effects { view.covering + view.glowing } else { 0 };
        if squares > 0 {
            gpu.queue.write_buffer(&self.flat_vb, 0, bytes_of(&view.effects[..squares]));
        }
        if let Some(flash) = &view.flash {
            gpu.queue.write_buffer(&self.flat_vb, (fx::VERTICES * core::mem::size_of::<FxVertex>()) as u64, bytes_of(flash));
        }
        // The glass goes over everything solid, the farthest first.
        self.glass_order.clear();
        for d in view.draws {
            let c = &chunks[d.chunk as usize];
            if c.flags & chunk::BLEND != 0 {
                self.glass_order.push((duty_sim::mat::box_distance2(view.eye, &c.min, &c.max), *d));
            }
        }
        self.glass_order.sort_by(|a, b| b.0.total_cmp(&a.0));

        let targets = self.targets.as_ref().unwrap();
        let (width, height) = (targets.width as f32, targets.height as f32);
        let (mut draws, mut triangles) = (0u32, 0u32);
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                // (several samples a pixel are kept only as far as their resolve)
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &targets.colour, resolve_target: targets.resolve.as_ref(), ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: haze[0] as f64, g: haze[1] as f64, b: haze[2] as f64, a: 1.0 }), store: if targets.resolve.is_some() { wgpu::StoreOp::Discard } else { wgpu::StoreOp::Store } } })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &targets.depth, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }), stencil_ops: None }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            // Everything but the held weapon shares the far part of the depth range.
            pass.set_viewport(0.0, 0.0, width, height, HELD_DEPTH, 1.0);
            pass.set_bind_group(0, &self.scene_group, &[]);

            // ---- the sky
            if view.look.sky && scene.sky_columns > 0 {
                pass.set_vertex_buffer(0, self.sky_vb.slice(..));
                pass.set_index_buffer(self.sky_ib.slice(..), wgpu::IndexFormat::Uint16);
                pass.set_pipeline(&self.sky);
                for c in 0..scene.sky_columns {
                    pass.set_bind_group(1, &self.pictures[(scene.sky_texture + c) as usize], &[]);
                    pass.draw_indexed(c * self.sky_column..(c + 1) * self.sky_column, 0, 0..1);
                }
                pass.set_pipeline(&self.cap);
                let band = scene.sky_columns * self.sky_column;
                pass.draw_indexed(band..band + self.sky_cap, 0, 0..1);
                draws += scene.sky_columns + 1;
            }

            // ---- the street
            pass.set_vertex_buffer(0, self.vertices.slice(..));
            pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint16);
            if let Some(light) = &self.light_group {
                pass.set_bind_group(3, light, &[]);
            }
            let run = |pass: &mut wgpu::RenderPass, d: &Draw, c: &Chunk| {
                let level = c.lod(d.lod as usize);
                let from = level.index + d.first;
                pass.draw_indexed(from..from + d.count, level.vertex as i32, 0..1);
            };
            let (mut bound, mut record, mut sides) = (u32::MAX, u32::MAX, usize::MAX);
            for d in view.draws {
                let c = &chunks[d.chunk as usize];
                if c.flags & chunk::BLEND != 0 {
                    continue;
                }
                let both = (c.flags & chunk::TWO_SIDED != 0) as usize;
                if both != sides {
                    sides = both;
                    pass.set_pipeline(&self.solid[both]);
                }
                if c.texture as u32 != bound {
                    bound = c.texture as u32;
                    pass.set_bind_group(1, &self.pictures[bound as usize], &[]);
                }
                if d.chunk as u32 != record {
                    record = d.chunk as u32;
                    pass.set_bind_group(2, &self.chunk_group, &[record * ALIGN as u32]);
                }
                run(&mut pass, d, c);
                draws += 1;
                triangles += d.count / 3;
            }
            // The ground once more, under its shadow picture (the PS Vita's street took it in its own pass).
            if view.look.shadows && !self.rich {
                pass.set_pipeline(&self.shadow);
                pass.set_bind_group(1, &self.pictures[scene.shadow_texture as usize], &[]);
                record = u32::MAX;
                for d in view.draws {
                    let c = &chunks[d.chunk as usize];
                    if c.flags & chunk::GROUND == 0 || c.flags & chunk::BLEND != 0 {
                        continue;
                    }
                    if d.chunk as u32 != record {
                        record = d.chunk as u32;
                        pass.set_bind_group(2, &self.chunk_group, &[record * ALIGN as u32]);
                    }
                    run(&mut pass, d, c);
                    draws += 1;
                    triangles += d.count / 3;
                }
            }
            (bound, record, sides) = (u32::MAX, u32::MAX, usize::MAX);
            for (_, d) in &self.glass_order {
                let c = &chunks[d.chunk as usize];
                let both = (c.flags & chunk::TWO_SIDED != 0) as usize;
                if both != sides {
                    sides = both;
                    pass.set_pipeline(&self.glass[both]);
                }
                if c.texture as u32 != bound {
                    bound = c.texture as u32;
                    pass.set_bind_group(1, &self.pictures[bound as usize], &[]);
                }
                if d.chunk as u32 != record {
                    record = d.chunk as u32;
                    pass.set_bind_group(2, &self.chunk_group, &[record * ALIGN as u32]);
                    let a = c.opacity as f64 / 255.0;
                    pass.set_blend_constant(wgpu::Color { r: a, g: a, b: a, a });
                }
                run(&mut pass, d, c);
                draws += 1;
                triangles += d.count / 3;
            }

            // ---- the soldiers
            let batch = |pass: &mut wgpu::RenderPass, number: usize, drawn: &Drawn| {
                pass.set_bind_group(1, &self.figure_group, &[(number as u64 * FIGURE_STRIDE) as u32]);
                pass.draw_indexed(drawn.first..drawn.first + drawn.indices, drawn.vertex as i32, 0..1);
            };
            let standing = view.held_from.min(figures.len());
            if !figures.is_empty() {
                pass.set_pipeline(&self.skin);
                pass.set_vertex_buffer(0, self.skin_vb.slice(..));
                pass.set_index_buffer(self.skin_ib.slice(..), wgpu::IndexFormat::Uint16);
                for (number, drawn) in figures[..standing].iter().enumerate() {
                    batch(&mut pass, number, drawn);
                    draws += 1;
                    triangles += drawn.indices / 3;
                }
            }

            // ---- the effects, over the world and the soldiers
            if squares > 0 {
                pass.set_vertex_buffer(0, self.flat_vb.slice(..));
                pass.set_index_buffer(self.quad_ib.slice(..), wgpu::IndexFormat::Uint16);
                pass.set_bind_group(1, &self.pictures[scene.fx_texture as usize], &[]);
                if view.covering > 0 {
                    pass.set_pipeline(&self.cover);
                    pass.draw_indexed(0..(view.covering / 4 * 6) as u32, 0, 0..1);
                }
                if view.glowing > 0 {
                    pass.set_pipeline(&self.glow);
                    pass.draw_indexed(0..(view.glowing / 4 * 6) as u32, view.covering as i32, 0..1);
                }
                draws += 2;
                triangles += (squares / 2) as u32;
            }

            // ---- the held weapon, in the near part of the depth range, and the flash at its muzzle
            if standing < figures.len() {
                pass.set_viewport(0.0, 0.0, width, height, 0.0, HELD_DEPTH);
                pass.set_pipeline(&self.skin);
                pass.set_vertex_buffer(0, self.skin_vb.slice(..));
                pass.set_index_buffer(self.skin_ib.slice(..), wgpu::IndexFormat::Uint16);
                for (number, drawn) in figures.iter().enumerate().skip(standing) {
                    batch(&mut pass, number, drawn);
                    draws += 1;
                    triangles += drawn.indices / 3;
                }
                if view.flash.is_some() {
                    pass.set_pipeline(&self.flash);
                    pass.set_vertex_buffer(0, self.flat_vb.slice(..));
                    pass.set_index_buffer(self.quad_ib.slice(..), wgpu::IndexFormat::Uint16);
                    pass.set_bind_group(1, &self.pictures[scene.fx_texture as usize], &[]);
                    pass.draw_indexed(0..6, fx::VERTICES as i32, 0..1);
                    draws += 1;
                }
            }
        }
        {
            // The frame in the display's colours.
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("present"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: frame.shown(), resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.present);
            pass.set_bind_group(0, &targets.group, &[]);
            pass.draw(0..3, 0..1);
        }
        overlay.draw(&mut encoder, &frame);
        gpu.queue.submit([encoder.finish()]);
        frame.present();
        Ok((draws, triangles))
    }
}
