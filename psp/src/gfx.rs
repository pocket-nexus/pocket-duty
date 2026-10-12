//! The GE renderer.
//!
//! A frame is one display list:
//!
//! 1. the sky: a band of pictures round the horizon and the zenith's colour
//!    over it, with no depth;
//! 2. the world near the eye, with a short frustum and three quarters of the
//!    16-bit depth buffer, then the world beyond, with a frustum from where the
//!    near one's chunks end and the other quarter. One range cannot tell a
//!    window frame from its wall at 60 m and still start 15 cm from the eye;
//! 3. the ground once more under its shadow picture, the glass, the effects;
//! 4. the soldiers, the effects, and the held weapon in a depth range of its
//!    own (`figures`);
//! 5. the interface, as PocketJS's GE backend draws what the guest last laid out.
//!
//! The GE has no programs. A world texel is its tile at half strength, the
//! vertex's colour is the surface under the baked light, and the texture stage
//! doubles their product. The ground's shadow is a second draw of the same
//! triangles that multiplies what is there by the shadow picture, placed by
//! the texture matrix from the vertex's own position.
//!
//! The GE clips at the near plane and nowhere else: a triangle with a corner
//! outside its guard band is dropped whole. `duty_sim::view` marks the
//! clusters whose large triangles could be; those are tested here and cut on
//! the CPU (`duty_sim::clip`).

use core::ffi::c_void;
use core::ptr;

use alloc::vec::Vec;
use duty_pack::{self as pack, chunk, texture, Chunk, Cluster, Scene, Texture, ViewCell, WorldVertex};
use duty_sim::clip::{ClipVertex, Guard, Verdict, MAX_OUT};
use duty_sim::fx::{self, Fx, FxVertex};
use duty_sim::mat::{self, Mat4};
use duty_sim::math::*;
use duty_sim::view::{self, Draw};
use pocket_psp_ge::DisplayList;
use psp::sys::*;

use crate::interface::Ui;
use crate::store::{self, Kept};

const LIST_WORDS: usize = 131_072;
static mut LIST: DisplayList<LIST_WORDS> = DisplayList::new();

/// One frame buffer: 512 × 272 texels of 16 bits. The colour and depth buffers are all this size.
pub const FB_BYTES: usize = 512 * 272 * 2;
/// Which of the two frame buffers the list in flight draws into.
pub static mut DRAW_BUFFER: usize = 0;
/// The depth buffer's shares: the held weapon's, the near world's, the far world's.
const DEPTH_HELD: i32 = 61440;
const DEPTH_SPLIT: i32 = 14336;
/// Metres from the eye at which a chunk goes to the far pass, and how far a near chunk may reach.
const SPLIT: f32 = 12.0;
/// The far pass's near plane at most, as a share of the least distance its triangles keep from
/// the eye: the cosine of the angle from the view's middle to its corner, with room for the wider
/// view of a sprint.
const FAR_NEAR: f32 = 0.55;
const NEAR_REACH: f32 = 44.0;
pub const NEAR: f32 = 0.15;
const FAR: f32 = 420.0;
/// Vertices of triangles cut on the CPU that a frame holds at most.
const CUT_VERTICES: usize = 1536;

const SKY_SEGMENTS: usize = 8;
const SKY_ROWS: usize = 6;

fn vtype_world() -> VertexType {
    VertexType::TEXTURE_16BIT | VertexType::COLOR_5650 | VertexType::VERTEX_16BIT | VertexType::INDEX_16BIT | VertexType::TRANSFORM_3D
}
/// Texture, colour and position as floats: the cut triangles, the sky and the effects.
fn vtype_float() -> VertexType {
    VertexType::TEXTURE_32BITF | VertexType::COLOR_8888 | VertexType::VERTEX_32BITF | VertexType::TRANSFORM_3D
}

/// Whether the cell at lattice place (`i`, `j`) is drawn in the near pass: its square comes
/// within `SPLIT` metres of the eye, over the ground. A line from the eye crosses cells whose two
/// lattice distances from the eye's own cell only grow, and a square's distance grows with them:
/// every near cell on the line comes before every far one. So what the near pass draws may hide
/// what the far pass drew without their depths being compared, which they cannot be: each pass
/// spreads its own distances over its own part of the 16 bits.
fn near_square(scene: &Scene, eye: V3, i: f32, j: f32) -> bool {
    let (x0, z0) = (scene.min[0] + i * scene.cell, scene.min[2] + j * scene.cell);
    let dx = max(max(x0 - eye.x, eye.x - (x0 + scene.cell)), 0.0);
    let dz = max(max(z0 - eye.z, eye.z - (z0 + scene.cell)), 0.0);
    dx * dx + dz * dz < SPLIT * SPLIT
}

fn near_cell(scene: &Scene, eye: V3, cell: u32) -> bool {
    near_square(scene, eye, (cell % scene.cells_x) as f32, (cell / scene.cells_x) as f32)
}

/// The pass that draws what stands at a place: a soldier, a puff of dust.
pub fn near_place(scene: &Scene, eye: V3, x: f32, z: f32) -> bool {
    near_square(scene, eye, floor((x - scene.min[0]) / scene.cell), floor((z - scene.min[2]) / scene.cell))
}

pub fn abgr(c: [f32; 3]) -> u32 {
    let b = |x: f32| (clamp(x, 0.0, 1.0) * 255.0 + 0.5) as u32;
    0xff00_0000 | (b(c[2]) << 16) | (b(c[1]) << 8) | b(c[0])
}

pub fn fmatrix(m: &Mat4) -> ScePspFMatrix4 {
    let col = |c: usize| ScePspFVector4 { x: m[c], y: m[4 + c], z: m[8 + c], w: m[12 + c] };
    ScePspFMatrix4 { x: col(0), y: col(1), z: col(2), w: col(3) }
}

fn vec4(x: f32, y: f32, z: f32, w: f32) -> ScePspFVector4 {
    ScePspFVector4 { x, y, z, w }
}

/// The eye of a frame.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub eye: V3,
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    /// Degrees, top to bottom.
    pub fov: f32,
}

#[derive(Clone, Copy, Default)]
pub struct Stats {
    pub draws: u32,
    pub triangles: u32,
    pub counts: view::Counts,
    /// Large triangles tested, and of them the ones cut.
    pub tested: u32,
    pub cut: u32,
    /// Microseconds choosing the draws, and writing the list.
    pub select: u32,
    pub list: u32,
    pub words: u32,
}

/// This frame's vertices and indices that are computed on the CPU: one block of ordinary memory,
/// filled from the start each frame. The display list's own memory is addressed past the data
/// cache, where a write costs a bus cycle; here a batch is written through the cache and flushed
/// once, before its draw.
struct Scratch {
    mem: Vec<u8>,
    at: usize,
}

impl Scratch {
    const BYTES: usize = 192 * 1024;
    fn new() -> Scratch {
        Scratch { mem: alloc::vec![0u8; Scratch::BYTES + 64], at: 0 }
    }
    /// `bytes` on a 16-byte boundary; null when the frame has used the block up.
    unsafe fn take(&mut self, bytes: usize) -> *mut u8 {
        let base = self.mem.as_mut_ptr();
        let first = (base as usize + 63) & !63;
        let at = (first + self.at + 15) & !15;
        if at + bytes > base as usize + self.mem.len() {
            return ptr::null_mut();
        }
        self.at = at + bytes - first;
        at as *mut u8
    }
}

unsafe fn flush<T>(p: *const T, bytes: usize) {
    sceKernelDcacheWritebackRange(p as *const c_void, bytes as u32);
}

/// A ground batch as the first pass sent it, to be sent again under the shadow picture.
#[derive(Clone, Copy)]
struct Sent {
    /// The cell its positions count from; `u32::MAX` for cut triangles, whose positions count from the eye.
    cell: u32,
    vertices: *const u8,
    indices: *const u16,
    count: u32,
    near: bool,
}

pub struct Gfx {
    pub scene: Scene,
    textures: Kept<Texture>,
    texels: Kept<u8>,
    chunks: Kept<Chunk>,
    clusters: Kept<Cluster>,
    vertices: Kept<WorldVertex>,
    indices: Kept<u16>,
    cells: Kept<ViewCell>,
    lists: Kept<u16>,
    bits: Kept<u8>,
    /// The far pass's near plane in the frame chosen, metres: under where its chunks begin.
    pub far_near: f32,
    draws: Vec<Draw>,
    /// The pass each of them is drawn in.
    pass_of: Vec<u8>,
    ground: Vec<Sent>,
    sky_vb: Vec<FxVertex>,
    sky_ib: Vec<u16>,
    quad_ib: Vec<u16>,
    scratch: Scratch,
    pub fx: Fx,
    pub stats: Stats,
    /// How far the levels of detail are pulled in to keep a frame's triangles under `budget`.
    pub nearer: f32,
    pub budget: u32,
    pub resident_bytes: usize,
    /// Development switches (`option=` on the control line): 1 no culling of back faces, 2 the
    /// other winding, 4 every chunk and cluster the view holds, 8 no haze, 16 no shadow pass,
    /// 32 no texture, 64 no cutting on the CPU, 128 no sky, 256 no interface in the list,
    /// 512 no turn for the interface, 1024 no effects, 2048 large triangles the GE cannot take
    /// are left out, not cut (what the cutting costs is the difference).
    pub option: u32,
    /// What `choose` kept for `world`, and when the list was begun.
    chosen: Option<(Camera, Mat4, Guard, Guard)>,
    list_from: u32,
}

/// Starts the GE: two 16-bit frame buffers, the depth buffer, and the state every frame assumes.
pub unsafe fn init() {
    sceGuInit();
    sceGuStart(GuContextType::Direct, DisplayList::as_mut_ptr(ptr::addr_of_mut!(LIST)));
    // 16-bit colour with ordered dither: half the memory traffic of 32-bit per pixel written.
    sceGuDrawBuffer(DisplayPixelFormat::Psm5650, ptr::null_mut(), 512);
    sceGuDispBuffer(480, 272, FB_BYTES as *mut c_void, 512);
    sceGuDepthBuffer((FB_BYTES * 2) as *mut c_void, 512);
    sceGuOffset(2048 - 240, 2048 - 136);
    sceGuViewport(2048, 2048, 480, 272);
    sceGuDepthRange(65535, 0);
    sceGuDepthFunc(DepthFunc::GreaterOrEqual);
    sceGuScissor(0, 0, 480, 272);
    sceGuEnable(GuState::ScissorTest);
    sceGuEnable(GuState::ClipPlanes);
    sceGuFrontFace(FrontFaceDirection::CounterClockwise);
    sceGuShadeModel(ShadingModel::Smooth);
    let row = |x, y, z, w| ScePspIVector4 { x, y, z, w };
    sceGuSetDither(&ScePspIMatrix4 { x: row(-4, 0, -3, 1), y: row(2, -2, 3, -1), z: row(-3, 1, -4, 0), w: row(3, -1, 2, -2) });
    sceGuEnable(GuState::Dither);
    sceGuBlendFunc(BlendOp::Add, BlendFactor::SrcAlpha, BlendFactor::OneMinusSrcAlpha, 0, 0);
    sceGuAlphaFunc(AlphaFunc::Greater, 0x80, 0xff);
    sceGuClearColor(BACKDROP);
    sceGuClear(ClearBuffer::COLOR_BUFFER_BIT);
    sceGuFinish();
    sceGuSync(GuSyncMode::Finish, GuSyncBehavior::Wait);
    sceDisplayWaitVblankStart();
    sceGuDisplay(true);
}

/// Presents the frame just drawn; the next list draws into the other buffer.
pub unsafe fn swap() {
    sceGuSwapBuffers();
    DRAW_BUFFER ^= 1;
}

/// A frame out of video memory, 480 × 272 texels of 16 bits: host I/O cannot take a video memory
/// address. `buffer` is `DRAW_BUFFER` for the list just drawn and not yet presented.
pub unsafe fn pixels(buffer: usize) -> Vec<u8> {
    let vram = sceGeEdramGetAddr().add(buffer * FB_BYTES);
    let mut out = alloc::vec![0u8; 480 * 272 * 2];
    for y in 0..272 {
        ptr::copy_nonoverlapping(vram.add(y * 512 * 2), out.as_mut_ptr().add(y * 480 * 2), 480 * 2);
    }
    out
}

/// Behind the interface while there is no world.
const BACKDROP: u32 = 0xff10_0c08;

/// A frame of the interface alone, shown at once: the pack is loading, or it could not be.
pub unsafe fn interlude(ui: &Ui) {
    sceGuStart(GuContextType::Direct, DisplayList::as_mut_ptr(ptr::addr_of_mut!(LIST)));
    sceGuClearColor(BACKDROP);
    sceGuClear(ClearBuffer::COLOR_BUFFER_BIT);
    sceGuDisable(GuState::DepthTest);
    sceGuDisable(GuState::CullFace);
    sceGuTexWrap(GuTexWrapMode::Clamp, GuTexWrapMode::Clamp);
    ui.draw();
    sceGuFinish();
    sceGuSync(GuSyncMode::Finish, GuSyncBehavior::Wait);
    pocketjs_psp::ge::reset_pool();
    sceDisplayWaitVblankStart();
    swap();
}

/// Sends one draw of indexed triangles from the vertices last pointed at. Every 24th one goes
/// through `sceGuDrawArray`, which also tells the GE how far the list has grown.
unsafe fn send(indices: *const u16, count: u32, since_stall: &mut u32) {
    *since_stall += 1;
    if *since_stall >= 24 {
        *since_stall = 0;
        sceGuDrawArray(GuPrimitive::Triangles, VertexType::empty(), count as i32, indices as *const c_void, ptr::null());
    } else {
        sceGuSendCommandi(GeCommand::Base, ((indices as u32 >> 8) & 0xf0000) as i32);
        sceGuSendCommandi(GeCommand::Iaddr, (indices as u32 & 0x00ff_ffff) as i32);
        sceGuSendCommandi(GeCommand::Prim, ((GuPrimitive::Triangles as i32) << 16) | count as i32);
    }
}

unsafe fn point_at(vertices: *const u8) {
    sceGuSendCommandi(GeCommand::Base, ((vertices as u32 >> 8) & 0xf0000) as i32);
    sceGuSendCommandi(GeCommand::Vaddr, (vertices as u32 & 0x00ff_ffff) as i32);
}

impl Gfx {
    /// Reads what the world is drawn from. `init` has set the GE up, or will.
    pub unsafe fn load(file: &store::PackFile, scene: Scene, progress: &mut dyn FnMut(&str)) -> Result<Gfx, &'static str> {
        progress("Reading the pictures");
        let textures: Kept<Texture> = file.resident(pack::TEXT)?;
        let texels: Kept<u8> = file.resident(pack::TEXD)?;
        if texels.as_ptr() as usize & 15 != 0 {
            return Err("the pack's pictures");
        }
        progress("Reading the street");
        let chunks: Kept<Chunk> = file.resident(pack::CHNK)?;
        let clusters: Kept<Cluster> = file.resident(pack::CLUS)?;
        let vertices: Kept<WorldVertex> = file.resident(pack::WVTX)?;
        let indices: Kept<u16> = file.resident(pack::WIDX)?;
        progress("Reading what is seen from where");
        let cells: Kept<ViewCell> = file.resident(pack::VCEL)?;
        let lists: Kept<u16> = file.resident(pack::VLST)?;
        let bits: Kept<u8> = file.resident(pack::VBIT)?;

        // The sky: a band of pictures round the horizon, then a fan to the zenith.
        let columns = scene.sky_columns.max(1) as usize;
        let mut sky_vb = Vec::new();
        let mut sky_ib = Vec::new();
        let around = columns * SKY_SEGMENTS;
        for c in 0..columns {
            let base = sky_vb.len() as u16;
            for row in 0..=SKY_ROWS {
                let v = row as f32 / SKY_ROWS as f32;
                let elevation = scene.sky_top + (scene.sky_floor - scene.sky_top) * v;
                for k in 0..=SKY_SEGMENTS {
                    let u = k as f32 / SKY_SEGMENTS as f32;
                    let azimuth = (c as f32 + u) / columns as f32 * TAU;
                    let d = v3(cos(elevation) * cos(azimuth), sin(elevation), cos(elevation) * sin(azimuth)) * 200.0;
                    // (half a texel in from the picture's edges: its neighbour is another picture)
                    sky_vb.push(FxVertex { uv: [0.002 + u * 0.996, 0.004 + v * 0.992], colour: [255, 255, 255, 255], pos: [d.x, d.y, d.z] });
                }
            }
            let stride = SKY_SEGMENTS as u16 + 1;
            for row in 0..SKY_ROWS as u16 {
                for k in 0..SKY_SEGMENTS as u16 {
                    let (a, b, c2, d) = (base + row * stride + k, base + row * stride + k + 1, base + (row + 1) * stride + k, base + (row + 1) * stride + k + 1);
                    sky_ib.extend_from_slice(&[a, c2, b, b, c2, d]);
                }
            }
        }
        let z = scene.zenith;
        let zenith = [z as u8, (z >> 8) as u8, (z >> 16) as u8, 255];
        let cap = sky_vb.len() as u16;
        for k in 0..around {
            let azimuth = k as f32 / around as f32 * TAU;
            let d = v3(cos(scene.sky_top) * cos(azimuth), sin(scene.sky_top), cos(scene.sky_top) * sin(azimuth)) * 200.0;
            sky_vb.push(FxVertex { uv: [0.0, 0.0], colour: zenith, pos: [d.x, d.y, d.z] });
        }
        sky_vb.push(FxVertex { uv: [0.0, 0.0], colour: zenith, pos: [0.0, 200.0, 0.0] });
        for k in 0..around as u16 {
            sky_ib.extend_from_slice(&[cap + k, cap + (k + 1) % around as u16, cap + around as u16]);
        }
        let quads = fx::VERTICES / 4;
        let mut quad_ib = Vec::with_capacity(quads * 6);
        for q in 0..quads as u16 {
            quad_ib.extend_from_slice(&[q * 4, q * 4 + 1, q * 4 + 2, q * 4, q * 4 + 2, q * 4 + 3]);
        }
        let far_near = SPLIT * FAR_NEAR;
        let resident_bytes = texels.len() + chunks.len() * core::mem::size_of::<Chunk>() + clusters.len() * core::mem::size_of::<Cluster>() + vertices.len() * 12 + indices.len() * 2 + lists.len() * 2 + bits.len();
        sceKernelDcacheWritebackAll();
        Ok(Gfx {
            scene,
            textures,
            texels,
            chunks,
            clusters,
            vertices,
            indices,
            cells,
            lists,
            bits,
            draws: Vec::with_capacity(4096),
            pass_of: Vec::with_capacity(4096),
            ground: Vec::with_capacity(512),
            sky_vb,
            sky_ib,
            quad_ib,
            scratch: Scratch::new(),
            fx: Fx::new(),
            stats: Stats::default(),
            nearer: 1.0,
            budget: 20_000,
            resident_bytes,
            far_near,
            option: 0,
            chosen: None,
            list_from: 0,
        })
    }

    /// Binds a picture: its palette, then its levels.
    unsafe fn bind(&self, index: u32) {
        const LEVELS: [MipmapLevel; 8] = [MipmapLevel::None, MipmapLevel::Level1, MipmapLevel::Level2, MipmapLevel::Level3, MipmapLevel::Level4, MipmapLevel::Level5, MipmapLevel::Level6, MipmapLevel::Level7];
        let t = &self.textures[index as usize];
        let base = self.texels.as_ptr().add(t.offset as usize);
        sceGuClutMode(ClutPixelFormat::Psm8888, 0, 0xff, 0);
        sceGuClutLoad(32, base as *const c_void);
        sceGuTexMode(TexturePixelFormat::PsmT8, t.levels as i32 - 1, 0, 1);
        for l in 0..t.levels as u32 {
            let (at, _) = t.level(l);
            let (w, h) = ((t.width as i32 >> l).max(1), (t.height as i32 >> l).max(1));
            sceGuTexImage(LEVELS[l as usize], w, h, w, base.add(at) as *const c_void);
        }
        if t.flags & texture::CLAMP != 0 {
            sceGuTexWrap(GuTexWrapMode::Clamp, GuTexWrapMode::Clamp);
        } else {
            sceGuTexWrap(GuTexWrapMode::Repeat, GuTexWrapMode::Repeat);
        }
    }

    /// A texel of the ground's shadow picture at a place: how much of the sun is left there, 0…1.
    pub fn sunlit(&self, x: f32, z: f32) -> f32 {
        let (t, r) = (&self.textures[self.scene.shadow_texture as usize], &self.scene.shadow_rows);
        let (u, v) = (x * r[0][0] + z * r[0][1] + r[0][2], x * r[1][0] + z * r[1][1] + r[1][2]);
        if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
            return 1.0;
        }
        let (px, py) = ((u * t.width as f32) as usize, (v * t.height as f32) as usize);
        // Swizzled: blocks of 16 bytes by 8 rows.
        let at = t.offset as usize + pack::texture::PALETTE_BYTES + ((py / 8) * (t.width as usize / 16) + px / 16) * 128 + (py % 8) * 16 + px % 16;
        let index = self.texels[at] as usize;
        // The palette's green: one half is all of the sun.
        saturate(self.texels[t.offset as usize + index * 4 + 1] as f32 / 127.0)
    }

    fn pack_world(&self) -> view::World<'_> {
        view::World { scene: &self.scene, chunks: &self.chunks, clusters: &self.clusters, cells: &self.cells, lists: &self.lists, bits: &self.bits }
    }

    /// Chooses the frame's draws. The GE may still be drawing the frame before.
    pub unsafe fn choose(&mut self, camera: &Camera) {
        self.stats = Stats::default();
        let t0 = sceKernelGetSystemTimeLow();
        let look = forward(camera.yaw, camera.pitch);
        let aspect = 480.0 / 272.0;
        let view = mat::view(camera.eye, look, camera.roll);
        let vp = mat::mul(&mat::perspective(camera.fov, aspect, NEAR, FAR), &view);
        let planes = mat::unit_planes(&mat::planes(&vp));
        let near_guard = Guard::new(&vp, NEAR, 480.0, 272.0, 2048.0).boxes_along(&self.scene.level_x, &self.scene.level_z);
        // The triangles of the frame before say how far in the levels of detail come.
        let mut draws = core::mem::take(&mut self.draws);
        let counts = view::select(&self.pack_world(), &planes, camera.eye, &near_guard, &view::Options { nearer: self.nearer, everything: self.option & 4 != 0 }, &mut draws);
        // Which of the frame's passes takes each draw: the near one for the cells about the eye
        // (bit 0), the glass's (bit 1).
        self.pass_of.clear();
        // The far pass's near plane goes under the nearest of its chunks: a chunk's triangles may
        // reach out of its cell's square, and nearer the eye than the square is.
        let mut least = SPLIT;
        for d in &draws {
            let c = &self.chunks[d.chunk as usize];
            let near = near_cell(&self.scene, camera.eye, c.cell as u32);
            if !near {
                least = min(least, mat::box_distance(camera.eye, &c.min, &c.max));
            }
            self.pass_of.push(near as u8 | ((c.flags & chunk::BLEND != 0) as u8) << 1);
        }
        self.far_near = max(least * FAR_NEAR, 1.0);
        self.draws = draws;
        let over = counts.triangles as f32 / self.budget.max(1) as f32;
        self.nearer = clamp(self.nearer * if over > 1.0 { 1.0 / (1.0 + (over - 1.0) * 0.5) } else { 1.0 + (1.0 - over) * 0.08 }, 0.4, 1.0);
        let far_guard = Guard::new(&vp, self.far_near, 480.0, 272.0, 2048.0).boxes_along(&self.scene.level_x, &self.scene.level_z);
        self.stats.counts = counts;
        self.chosen = Some((*camera, view, near_guard, far_guard));
        self.stats.select = sceKernelGetSystemTimeLow().wrapping_sub(t0);
    }

    /// The model matrix of a cell's stored positions, the eye being the origin.
    unsafe fn cell_matrix(&self, cell: u32, eye: V3) {
        let c = self.scene.cell_centre(cell);
        // A position is a signed 16-bit number of lattice steps: the GE reads it as -1…1.
        let s = 32768.0 * self.scene.quantum;
        let m = ScePspFMatrix4 { x: vec4(s, 0.0, 0.0, 0.0), y: vec4(0.0, s, 0.0, 0.0), z: vec4(0.0, 0.0, s, 0.0), w: vec4(c[0] - eye.x, c[1] - eye.y, c[2] - eye.z, 1.0) };
        sceGuSetMatrix(MatrixMode::Model, &m);
    }

    unsafe fn eye_matrix(&self) {
        let id = ScePspFMatrix4 { x: vec4(1.0, 0.0, 0.0, 0.0), y: vec4(0.0, 1.0, 0.0, 0.0), z: vec4(0.0, 0.0, 1.0, 0.0), w: vec4(0.0, 0.0, 0.0, 1.0) };
        sceGuSetMatrix(MatrixMode::Model, &id);
    }

    /// The solid world of one pass: `near` for the chunks about the eye.
    unsafe fn pass(&mut self, near: bool, eye: V3, guard: &Guard, blend: bool) {
        let scene = self.scene;
        let (mut bound, mut cell, mut chunk_at, mut vertices_at, mut flags) = (u32::MAX, u32::MAX, u32::MAX, ptr::null::<u8>(), u16::MAX);
        let mut since_stall = 0u32;
        // The triangles cut on the CPU, picture by picture.
        let cut = self.scratch.take(CUT_VERTICES * core::mem::size_of::<ClipVertex>()) as *mut ClipVertex;
        let (mut cut_n, mut cut_first, mut cut_ground) = (0usize, 0usize, false);
        let no_cut = self.option & 64 != 0 || cut.is_null();
        // The guard for positions as the cell stores them.
        let (mut stored, mut stored_cell) = (guard.stored([0.0; 3], 1.0), u32::MAX);
        let mut tested = 0u32;
        let want = near as u8 | (blend as u8) << 1;
        for i in 0..self.draws.len() {
            if *self.pass_of.get_unchecked(i) != want {
                continue;
            }
            let d = *self.draws.get_unchecked(i);
            // (the chunks stand still for the program's life)
            let c = &*self.chunks.as_ptr().add(d.chunk as usize);
            if c.texture as u32 != bound {
                // The cut triangles of the picture before go out before it is replaced.
                self.send_cut(cut, cut_first, cut_n, near, cut_ground);
                cut_first = cut_n;
                bound = c.texture as u32;
                self.bind(bound);
                flags = u16::MAX;
                cell = u32::MAX;
                vertices_at = ptr::null();
            }
            let state = c.flags & (chunk::TWO_SIDED | chunk::CUTOUT | chunk::BLEND);
            if state != flags {
                flags = state;
                if state & chunk::TWO_SIDED != 0 || self.option & 1 != 0 {
                    sceGuDisable(GuState::CullFace);
                } else {
                    sceGuEnable(GuState::CullFace);
                }
                if state & chunk::CUTOUT != 0 {
                    sceGuEnable(GuState::AlphaTest);
                    sceGuTexFunc(TextureEffect::Modulate, TextureColorComponent::Rgba);
                } else {
                    sceGuDisable(GuState::AlphaTest);
                    sceGuTexFunc(TextureEffect::Modulate, TextureColorComponent::Rgb);
                }
            }
            if blend {
                // Glass: its colour by its opacity over what is behind it.
                let a = c.opacity as u32;
                sceGuBlendFunc(BlendOp::Add, BlendFactor::Fix, BlendFactor::Fix, a | a << 8 | a << 16, (255 - a) | (255 - a) << 8 | (255 - a) << 16);
            }
            if c.cell as u32 != cell {
                cell = c.cell as u32;
                self.cell_matrix(cell, eye);
            }
            if d.chunk as u32 != chunk_at {
                chunk_at = d.chunk as u32;
                sceGuTexScale(c.uv_scale[0], c.uv_scale[1]);
                sceGuTexOffset(c.uv_offset[0], c.uv_offset[1]);
            }
            let lod = &c.lod[d.lod as usize];
            let vertices = self.vertices.as_ptr().add(lod.vertex as usize) as *const u8;
            if vertices != vertices_at {
                vertices_at = vertices;
                point_at(vertices);
            }
            let ground = c.flags & chunk::GROUND != 0;
            let indices = self.indices.as_ptr().add((lod.index + d.first) as usize);
            let mut first = 0u32;
            if d.cut && !no_cut {
                // Its large triangles one by one: the GE takes the ones inside its guard band, the
                // others are cut here.
                let centre = scene.cell_centre(cell);
                if cell != stored_cell {
                    stored_cell = cell;
                    stored = guard.stored(centre, scene.quantum);
                }
                let keep = self.scratch.take(d.large as usize * 6) as *mut u16;
                let mut kept = 0usize;
                let records = vertices as *const WorldVertex;
                let world = |v: &WorldVertex| v3(centre[0] + v.pos[0] as f32 * scene.quantum, centre[1] + v.pos[1] as f32 * scene.quantum, centre[2] + v.pos[2] as f32 * scene.quantum);
                let steps = |v: &WorldVertex| v3(v.pos[0] as f32, v.pos[1] as f32, v.pos[2] as f32);
                tested += d.large as u32;
                for t in 0..d.large as usize {
                    let corner = [*indices.add(t * 3), *indices.add(t * 3 + 1), *indices.add(t * 3 + 2)];
                    let v = [&*records.add(corner[0] as usize), &*records.add(corner[1] as usize), &*records.add(corner[2] as usize)];
                    let clip = [stored.to_clip(steps(v[0])), stored.to_clip(steps(v[1])), stored.to_clip(steps(v[2]))];
                    match stored.classify(&clip) {
                        Verdict::Culled => {}
                        Verdict::Clip if self.option & 2048 != 0 => {}
                        Verdict::Clip if cut_n + MAX_OUT <= CUT_VERTICES => {
                            let p = [world(v[0]), world(v[1]), world(v[2])];
                            let full = |k: usize| {
                                let (r, g, b) = ((v[k].colour & 31) as u8, ((v[k].colour >> 5) & 63) as u8, (v[k].colour >> 11) as u8);
                                ClipVertex { uv: [v[k].uv[0] as f32 * (c.uv_scale[0] / 32768.0) + c.uv_offset[0], v[k].uv[1] as f32 * (c.uv_scale[1] / 32768.0) + c.uv_offset[1]], color: [(r << 3) | (r >> 2), (g << 2) | (g >> 4), (b << 3) | (b >> 2), 255], pos: [p[k].x, p[k].y, p[k].z] }
                            };
                            let n = guard.clip(&[full(0), full(1), full(2)], &clip, core::slice::from_raw_parts_mut(cut.add(cut_n), MAX_OUT));
                            // The pieces are sent with the eye as the origin.
                            for piece in core::slice::from_raw_parts_mut(cut.add(cut_n), n) {
                                piece.pos = [piece.pos[0] - eye.x, piece.pos[1] - eye.y, piece.pos[2] - eye.z];
                            }
                            cut_n += n;
                            cut_ground |= ground;
                            self.stats.cut += 1;
                        }
                        _ if !keep.is_null() => {
                            ptr::copy_nonoverlapping(corner.as_ptr(), keep.add(kept), 3);
                            kept += 3;
                        }
                        _ => {}
                    }
                }
                if kept > 0 {
                    flush(keep, kept * 2);
                    send(keep, kept as u32, &mut since_stall);
                    if ground {
                        self.ground.push(Sent { cell, vertices, indices: keep, count: kept as u32, near });
                    }
                    self.stats.draws += 1;
                    self.stats.triangles += kept as u32 / 3;
                }
                first = d.large as u32 * 3;
            }
            // The rest of the run, as it is: at most 65 535 indices in one draw.
            let mut at = first;
            while at < d.count {
                let count = (d.count - at).min(65_532);
                send(indices.add(at as usize), count, &mut since_stall);
                if ground {
                    self.ground.push(Sent { cell, vertices, indices: indices.add(at as usize), count, near });
                }
                at += count;
                self.stats.draws += 1;
                self.stats.triangles += count / 3;
            }
        }
        self.stats.tested += tested;
        self.send_cut(cut, cut_first, cut_n, near, cut_ground);
    }

    /// Sends the triangles cut on the CPU since `first`: float vertices with the eye as the origin.
    unsafe fn send_cut(&mut self, cut: *mut ClipVertex, first: usize, end: usize, near: bool, ground: bool) {
        if end <= first {
            return;
        }
        let at = cut.add(first);
        flush(at, (end - first) * core::mem::size_of::<ClipVertex>());
        self.eye_matrix();
        sceGuTexScale(1.0, 1.0);
        sceGuTexOffset(0.0, 0.0);
        sceGuDrawArray(GuPrimitive::Triangles, vtype_float(), (end - first) as i32, ptr::null(), at as *const c_void);
        sceGuSendCommandi(GeCommand::VertexType, vtype_world().bits());
        if ground {
            self.ground.push(Sent { cell: u32::MAX, vertices: at as *const u8, indices: ptr::null(), count: (end - first) as u32, near });
        }
        self.stats.draws += 1;
        self.stats.triangles += (end - first) as u32 / 3;
    }

    /// The ground of one pass once more, under the shadow picture.
    unsafe fn shadows(&mut self, near: bool, eye: V3) {
        let (r, s) = (self.scene.shadow_rows, 32768.0 * self.scene.quantum);
        let mut cell = u32::MAX - 1;
        let mut vertices_at = ptr::null::<u8>();
        let mut since_stall = 0u32;
        for i in 0..self.ground.len() {
            let g = self.ground[i];
            if g.near != near {
                continue;
            }
            if g.cell != cell {
                cell = g.cell;
                // Where a stored position stands in the world, and from that in the picture.
                let (scale, at) = if cell == u32::MAX {
                    self.eye_matrix();
                    (1.0, [eye.x, eye.y, eye.z])
                } else {
                    self.cell_matrix(cell, eye);
                    (s, self.scene.cell_centre(cell))
                };
                let m = ScePspFMatrix4 {
                    x: vec4(r[0][0] * scale, r[1][0] * scale, 0.0, 0.0),
                    y: vec4(0.0, 0.0, 0.0, 0.0),
                    z: vec4(r[0][1] * scale, r[1][1] * scale, 0.0, 0.0),
                    w: vec4(r[0][0] * at[0] + r[0][1] * at[2] + r[0][2], r[1][0] * at[0] + r[1][1] * at[2] + r[1][2], 1.0, 0.0),
                };
                sceGuSetMatrix(MatrixMode::Texture, &m);
                vertices_at = ptr::null();
            }
            if g.indices.is_null() {
                sceGuDrawArray(GuPrimitive::Triangles, vtype_float(), g.count as i32, ptr::null(), g.vertices as *const c_void);
                sceGuSendCommandi(GeCommand::VertexType, vtype_world().bits());
                vertices_at = ptr::null();
            } else {
                if g.vertices != vertices_at {
                    vertices_at = g.vertices;
                    point_at(g.vertices);
                }
                send(g.indices, g.count, &mut since_stall);
            }
            self.stats.draws += 1;
        }
    }

    /// Starts the list of the frame chosen and writes the sky and the world into it. No list is in
    /// flight when this is called. Returns the frame's eye and its view matrix, for what the caller
    /// draws next: the soldiers, then `effects`, the held weapon, and `finish`.
    pub unsafe fn world(&mut self) -> Option<(Camera, Mat4)> {
        let (camera, view, near_guard, far_guard) = self.chosen.take()?;
        self.list_from = sceKernelGetSystemTimeLow();
        self.scratch.at = 0;
        self.ground.clear();
        let (eye, aspect) = (camera.eye, 480.0 / 272.0);
        let look = forward(camera.yaw, camera.pitch);
        sceGuStart(GuContextType::Direct, DisplayList::as_mut_ptr(ptr::addr_of_mut!(LIST)));
        sceGuClearColor(self.scene.haze_colour);
        sceGuClearDepth(0);
        sceGuClear(ClearBuffer::COLOR_BUFFER_BIT | ClearBuffer::DEPTH_BUFFER_BIT);
        // The eye is the origin: a cell's matrix carries where it is from there.
        sceGuSetMatrix(MatrixMode::View, &fmatrix(&mat::view(V3::ZERO, look, camera.roll)));
        let near_proj = fmatrix(&mat::perspective_gl(camera.fov, aspect, NEAR, NEAR_REACH));
        let far_proj = fmatrix(&mat::perspective_gl(camera.fov, aspect, self.far_near, FAR));
        sceGuTexFilter(TextureFilter::Linear, TextureFilter::Linear);
        sceGuTexLevelMode(TextureLevelMode::Auto, 0.0);
        sceGuTexMapMode(TextureMapMode::TextureCoords, 0, 0);
        sceGuTexScale(1.0, 1.0);
        sceGuTexOffset(0.0, 0.0);
        sceGuDisable(GuState::Blend);
        sceGuDisable(GuState::AlphaTest);
        sceGuDisable(GuState::Fog);
        sceGuDisable(GuState::Lighting);

        // ------------------------------------------------------------ the sky
        if self.option & 128 == 0 && self.scene.sky_columns > 0 {
            sceGuSetMatrix(MatrixMode::Projection, &far_proj);
            sceGuDepthRange(DEPTH_SPLIT, 0);
            sceGuDisable(GuState::DepthTest);
            sceGuDisable(GuState::CullFace);
            sceGuDisable(GuState::Fragment2X);
            sceGuDepthMask(1);
            self.eye_matrix();
            sceGuEnable(GuState::Texture2D);
            sceGuTexFunc(TextureEffect::Modulate, TextureColorComponent::Rgb);
            let per_column = SKY_SEGMENTS * SKY_ROWS * 6;
            for c in 0..self.scene.sky_columns as usize {
                self.bind(self.scene.sky_texture + c as u32);
                sceGuDrawArray(GuPrimitive::Triangles, vtype_float() | VertexType::INDEX_16BIT, per_column as i32, self.sky_ib.as_ptr().add(c * per_column) as *const c_void, self.sky_vb.as_ptr() as *const c_void);
            }
            sceGuDisable(GuState::Texture2D);
            let cap = self.scene.sky_columns as usize * per_column;
            sceGuDrawArray(GuPrimitive::Triangles, vtype_float() | VertexType::INDEX_16BIT, (self.sky_ib.len() - cap) as i32, self.sky_ib.as_ptr().add(cap) as *const c_void, self.sky_vb.as_ptr() as *const c_void);
            self.stats.draws += self.scene.sky_columns + 1;
            sceGuDepthMask(0);
        }

        // ------------------------------------------------------------ the world
        sceGuEnable(GuState::DepthTest);
        sceGuFrontFace(if self.option & 2 == 0 { FrontFaceDirection::CounterClockwise } else { FrontFaceDirection::Clockwise });
        if self.option & 32 == 0 {
            sceGuEnable(GuState::Texture2D);
        }
        sceGuEnable(GuState::Fragment2X);
        sceGuTexFilter(TextureFilter::LinearMipmapNearest, TextureFilter::Linear);
        if self.option & 8 == 0 && self.scene.haze[1] > self.scene.haze[0] {
            sceGuEnable(GuState::Fog);
            sceGuFog(self.scene.haze[0], self.scene.haze[1], self.scene.haze_colour);
        }
        sceGuSendCommandi(GeCommand::VertexType, vtype_world().bits());
        for near in [true, false] {
            if near {
                sceGuSetMatrix(MatrixMode::Projection, &near_proj);
                sceGuDepthRange(DEPTH_HELD, DEPTH_SPLIT);
            } else {
                sceGuSetMatrix(MatrixMode::Projection, &far_proj);
                sceGuDepthRange(DEPTH_SPLIT, 0);
            }
            sceGuTexFilter(TextureFilter::LinearMipmapNearest, TextureFilter::Linear);
            self.pass(near, eye, if near { &near_guard } else { &far_guard }, false);
            // The ground once more: what is there times the shadow picture, where the depth is the ground's own.
            if self.option & 16 == 0 && !self.ground.is_empty() {
                sceGuEnable(GuState::Blend);
                sceGuBlendFunc(BlendOp::Add, BlendFactor::Color, BlendFactor::Fix, 0, 0);
                sceGuDepthFunc(DepthFunc::Equal);
                sceGuDepthMask(1);
                sceGuDisable(GuState::AlphaTest);
                sceGuEnable(GuState::CullFace);
                sceGuDisable(GuState::Fog);
                sceGuTexFunc(TextureEffect::Replace, TextureColorComponent::Rgb);
                sceGuTexFilter(TextureFilter::Linear, TextureFilter::Linear);
                self.bind(self.scene.shadow_texture);
                sceGuTexMapMode(TextureMapMode::TextureMatrix, 0, 0);
                sceGuTexProjMapMode(TextureProjectionMapMode::Position);
                self.shadows(near, eye);
                sceGuTexMapMode(TextureMapMode::TextureCoords, 0, 0);
                sceGuDepthFunc(DepthFunc::GreaterOrEqual);
                sceGuDepthMask(0);
                sceGuDisable(GuState::Blend);
                if self.option & 8 == 0 && self.scene.haze[1] > self.scene.haze[0] {
                    sceGuEnable(GuState::Fog);
                }
            }
        }
        // The glass, over everything solid, farthest pass first.
        sceGuEnable(GuState::Blend);
        sceGuDepthMask(1);
        for near in [false, true] {
            if near {
                sceGuSetMatrix(MatrixMode::Projection, &near_proj);
                sceGuDepthRange(DEPTH_HELD, DEPTH_SPLIT);
            } else {
                sceGuSetMatrix(MatrixMode::Projection, &far_proj);
                sceGuDepthRange(DEPTH_SPLIT, 0);
            }
            self.pass(near, eye, if near { &near_guard } else { &far_guard }, true);
        }
        sceGuDepthMask(0);
        sceGuDisable(GuState::Blend);
        sceGuDisable(GuState::AlphaTest);
        sceGuDisable(GuState::Fragment2X);
        sceGuDisable(GuState::Fog);
        sceGuTexScale(1.0, 1.0);
        sceGuTexOffset(0.0, 0.0);

        sceGuEnable(GuState::CullFace);
        Some((camera, view))
    }

    /// The projection and the part of the depth buffer of one of the world's two passes.
    pub unsafe fn pass_of_place(&self, camera: &Camera, near: bool) {
        let aspect = 480.0 / 272.0;
        if near {
            sceGuSetMatrix(MatrixMode::Projection, &fmatrix(&mat::perspective_gl(camera.fov, aspect, NEAR, NEAR_REACH)));
            sceGuDepthRange(DEPTH_HELD, DEPTH_SPLIT);
        } else {
            sceGuSetMatrix(MatrixMode::Projection, &fmatrix(&mat::perspective_gl(camera.fov, aspect, self.far_near, FAR)));
            sceGuDepthRange(DEPTH_SPLIT, 0);
        }
    }

    /// The effects, over the world and the soldiers.
    pub unsafe fn effects(&mut self, camera: &Camera) {
        let (eye, aspect) = (camera.eye, 480.0 / 272.0);
        let look = forward(camera.yaw, camera.pitch);
        sceGuSetMatrix(MatrixMode::View, &fmatrix(&mat::view(V3::ZERO, look, camera.roll)));
        let near_proj = fmatrix(&mat::perspective_gl(camera.fov, aspect, NEAR, NEAR_REACH));
        let far_proj = fmatrix(&mat::perspective_gl(camera.fov, aspect, self.far_near, FAR));
        sceGuEnable(GuState::DepthTest);
        // ------------------------------------------------------------ the effects, over the world and the soldiers
        // Each of the world's two passes wrote its own scale of depth, so the effects are drawn
        // twice too: what is near against the near pass's depth, the rest against the far one's.
        if self.option & 1024 == 0 {
            let right = look.cross(V3::UP).norm_or(v3(1.0, 0.0, 0.0));
            let up = right.cross(look);
            sceGuEnable(GuState::Texture2D);
            sceGuEnable(GuState::Blend);
            sceGuDisable(GuState::CullFace);
            sceGuDepthMask(1);
            sceGuTexFilter(TextureFilter::Linear, TextureFilter::Linear);
            sceGuTexFunc(TextureEffect::Modulate, TextureColorComponent::Rgba);
            self.bind(self.scene.fx_texture);
            self.eye_matrix();
            for near in [true, false] {
                let out = self.scratch.take(fx::VERTICES * core::mem::size_of::<FxVertex>()) as *mut FxVertex;
                if out.is_null() {
                    break;
                }
                let scene = self.scene;
                let (covering, glowing) = self.fx.build(eye, right, up, |p| near_place(&scene, eye, p.x, p.z) == near, core::slice::from_raw_parts_mut(out, fx::VERTICES));
                if covering + glowing == 0 {
                    continue;
                }
                flush(out, (covering + glowing) * core::mem::size_of::<FxVertex>());
                if near {
                    sceGuSetMatrix(MatrixMode::Projection, &near_proj);
                    sceGuDepthRange(DEPTH_HELD, DEPTH_SPLIT);
                } else {
                    sceGuSetMatrix(MatrixMode::Projection, &far_proj);
                    sceGuDepthRange(DEPTH_SPLIT, 0);
                }
                if covering > 0 {
                    sceGuBlendFunc(BlendOp::Add, BlendFactor::SrcAlpha, BlendFactor::OneMinusSrcAlpha, 0, 0);
                    sceGuDrawArray(GuPrimitive::Triangles, vtype_float() | VertexType::INDEX_16BIT, (covering / 4 * 6) as i32, self.quad_ib.as_ptr() as *const c_void, out as *const c_void);
                }
                if glowing > 0 {
                    sceGuBlendFunc(BlendOp::Add, BlendFactor::SrcAlpha, BlendFactor::Fix, 0, 0x00ff_ffff);
                    sceGuDrawArray(GuPrimitive::Triangles, vtype_float() | VertexType::INDEX_16BIT, (glowing / 4 * 6) as i32, self.quad_ib.as_ptr() as *const c_void, out.add(covering) as *const c_void);
                }
                self.stats.draws += 2;
                self.stats.triangles += ((covering + glowing) / 2) as u32;
            }
            sceGuBlendFunc(BlendOp::Add, BlendFactor::SrcAlpha, BlendFactor::OneMinusSrcAlpha, 0, 0);
            sceGuDepthMask(0);
            sceGuDisable(GuState::Blend);
        }

    }

    /// A muzzle's flash at a place of the held weapon's own space, in the frustum the weapon was
    /// drawn with.
    pub unsafe fn flash(&mut self, at: V3, half: f32) {
        let out = self.scratch.take(4 * core::mem::size_of::<FxVertex>()) as *mut FxVertex;
        if out.is_null() {
            return;
        }
        for (k, (sx, sy)) in [(-1.0, 1.0), (1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)].into_iter().enumerate() {
            *out.add(k) = FxVertex { uv: [0.02 + (sx * 0.5 + 0.5) * 0.46, 0.52 + (0.5 - sy * 0.5) * 0.46], colour: [255, 220, 160, 240], pos: [at.x + sx * half, at.y + sy * half, at.z] };
        }
        flush(out, 4 * core::mem::size_of::<FxVertex>());
        sceGuEnable(GuState::Texture2D);
        sceGuEnable(GuState::Blend);
        sceGuDisable(GuState::CullFace);
        sceGuDisable(GuState::DepthTest);
        sceGuTexFilter(TextureFilter::Linear, TextureFilter::Linear);
        sceGuTexFunc(TextureEffect::Modulate, TextureColorComponent::Rgba);
        self.bind(self.scene.fx_texture);
        sceGuBlendFunc(BlendOp::Add, BlendFactor::SrcAlpha, BlendFactor::Fix, 0, 0x00ff_ffff);
        sceGuDrawArray(GuPrimitive::Triangles, vtype_float() | VertexType::INDEX_16BIT, 6, self.quad_ib.as_ptr() as *const c_void, out as *const c_void);
        sceGuBlendFunc(BlendOp::Add, BlendFactor::SrcAlpha, BlendFactor::OneMinusSrcAlpha, 0, 0);
        sceGuDisable(GuState::Blend);
        self.stats.draws += 1;
    }

    /// The interface over everything, and the list's end: the GE draws it from here on.
    pub unsafe fn finish(&mut self, ui: &Ui) {
        // PocketJS's GE backend sets blending and texturing for itself and leaves both off; it draws
        // in screen coordinates, over everything.
        if self.option & 256 == 0 {
            sceGuDisable(GuState::DepthTest);
            sceGuDisable(GuState::CullFace);
            sceGuDisable(GuState::Texture2D);
            sceGuDisable(GuState::Lighting);
            sceGuTexWrap(GuTexWrapMode::Clamp, GuTexWrapMode::Clamp);
            ui.draw();
        }
        self.stats.words = (sceGuFinish() / 4) as u32;
        self.stats.list = sceKernelGetSystemTimeLow().wrapping_sub(self.list_from);
    }
}
