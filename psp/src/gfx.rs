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
//! clusters whose large triangles could be; of those the ones near enough for
//! their size (`duty_pack::Large`) are tested here and cut on the CPU
//! (`duty_sim::clip`).
//!
//! What a console takes for this, measured on one with 64 MB (`probe=1` in
//! `boot.txt`, and the frame's own timers):
//!
//! - **A multiply and an add are 20 cycles** of the 333 MHz CPU's floating
//!   point unit, a divide 45. What is asked of every cluster and triangle is
//!   asked with as few of them as it can be.
//! - **A line of the data cache fetched while the GE draws is about a
//!   microsecond**, three times what it is with the GE idle: the two share
//!   the bus. The frame reads the pack in order where it can, and keeps what
//!   it asks again (`Near`, `Seen`).
//! - **A word of `sceGu`'s list is a bus cycle of its own**, written past the
//!   cache. The world's commands go through the cache into runs the list
//!   calls (`Sub`).
//! - **The GE takes 0.7 ms for a thousand triangles** it is sent, drawn or
//!   not, and is slower while this CPU reads memory. Under 20 000 triangles
//!   and 33 ms a frame, the levels of detail come in (`Gfx::nearer`).

use core::ffi::c_void;
use core::ptr;

use alloc::vec::Vec;
use duty_pack::{self as pack, chunk, large, texture, Chunk, Cluster, Large, Scene, Texture, ViewCell, WorldVertex};
use duty_sim::clip::{ClipVertex, Corner, Guard, Verdict, MAX_OUT, MAX_POLY};
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
/// The nearest the levels of detail are pulled in: the finest within 3 m, the second within 10 m.
const NEARER_LEAST: f32 = 0.35;
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
    /// Large triangles of the clusters that reach out of the band; of them the ones near enough
    /// for their size to have their corners looked at; the corners the guard worked out (the
    /// others it had seen); and the triangles cut.
    pub tested: u32,
    pub near: u32,
    pub corners: u32,
    pub cut: u32,
    /// Microseconds choosing the draws, and writing the list.
    pub select: u32,
    pub list: u32,
    pub words: u32,
    /// Microseconds of `select` in the choice itself, and of `list` in its parts: the sky, the
    /// near pass, its shadows, the far pass, its shadows, the glass, the soldiers, the effects,
    /// the held weapon, the interface.
    pub chosen: u32,
    pub parts: [u32; 10],
    marked: u32,
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

/// Commands for the GE that are written through the data cache and called from the frame's list.
///
/// The list `sceGu` writes is addressed past the cache, and each of its words is a bus cycle of
/// its own. On the console measured that is 18 ns a word while the GE is idle and four times that
/// while it draws, which it does for most of the time the list is written: 250 draws of the far pass took
/// 1.45 ms, and 0.36 ms with the GE held. Words written here reach memory a cache line at a
/// time, when `close` flushes a run of them and the frame's list calls it.
pub struct Sub {
    _mem: Vec<u32>,
    first: *mut u32,
    at: *mut u32,
    /// Where the run being written starts.
    open: *mut u32,
    end: *mut u32,
    draws: u32,
    /// Draws in a run: the GE is handed the run when it is this long.
    pub run: u32,
}

impl Default for Sub {
    fn default() -> Sub {
        Sub { _mem: Vec::new(), first: ptr::null_mut(), at: ptr::null_mut(), open: ptr::null_mut(), end: ptr::null_mut(), draws: 0, run: 48 }
    }
}

impl Sub {
    const WORDS: usize = 48 * 1024;
    /// Words a draw takes at most with everything that can go before it: a picture with eight
    /// levels, the surface's state, a cell's matrix, the picture's scale.
    const UNIT: usize = 96;
    fn new() -> Sub {
        let mut mem = alloc::vec![0u32; Sub::WORDS + 32];
        let first = ((mem.as_mut_ptr() as usize + 63) & !63) as *mut u32;
        Sub { _mem: mem, first, at: first, open: first, end: unsafe { first.add(Sub::WORDS) }, draws: 0, run: 48 }
    }

    /// A new frame: the GE has read everything the last one wrote.
    fn rewind(&mut self) {
        self.at = self.first;
        self.open = self.first;
        self.draws = 0;
    }

    /// Whether another draw fits.
    #[inline(always)]
    pub fn room(&self) -> bool {
        (self.end as usize - self.at as usize) / 4 > Sub::UNIT
    }

    #[inline(always)]
    pub unsafe fn i(&mut self, command: GeCommand, argument: u32) {
        self.at.write((command as u32) << 24 | (argument & 0x00ff_ffff));
        self.at = self.at.add(1);
    }

    #[inline(always)]
    pub unsafe fn f(&mut self, command: GeCommand, value: f32) {
        self.i(command, value.to_bits() >> 8);
    }

    /// A command that takes an address: its high bits go in `Base` first.
    #[inline(always)]
    pub unsafe fn address(&mut self, command: GeCommand, at: usize) {
        self.i(GeCommand::Base, (at as u32 >> 8) & 0xf0000);
        self.i(command, at as u32);
    }

    /// A matrix of three rows: its four columns, as `sceGuSetMatrix` sends them.
    #[inline(always)]
    pub unsafe fn matrix(&mut self, number: GeCommand, data: GeCommand, first: u32, columns: &[[f32; 3]; 4]) {
        self.i(number, first);
        for column in columns {
            self.f(data, column[0]);
            self.f(data, column[1]);
            self.f(data, column[2]);
        }
    }

    /// One draw of triangles from the vertices last pointed at: indexed, or the vertices in order
    /// when `indices` is null.
    #[inline(always)]
    pub unsafe fn triangles(&mut self, indices: *const u16, count: u32) {
        if !indices.is_null() {
            self.address(GeCommand::Iaddr, indices as usize);
        }
        self.i(GeCommand::Prim, (GuPrimitive::Triangles as u32) << 16 | count);
        self.draws += 1;
        if self.draws >= self.run {
            self.close();
        }
    }

    /// A fan of the next `count` vertices at `vertices` (float vertices cut on the CPU).
    #[inline(always)]
    pub unsafe fn fan(&mut self, vertices: *const u8, count: u32) {
        self.address(GeCommand::Vaddr, vertices as usize);
        self.i(GeCommand::Prim, (GuPrimitive::TriangleFan as u32) << 16 | count);
    }

    /// Ends the run: flushes it and has the frame's list call it. What `sceGu` writes next comes
    /// after it.
    pub unsafe fn close(&mut self) {
        if self.at == self.open {
            return;
        }
        self.i(GeCommand::Ret, 0);
        flush(self.open, self.at as usize - self.open as usize);
        sceGuCallList(self.open as *const c_void);
        // The next run starts on a cache line of its own.
        self.at = ((self.at as usize + 63) & !63) as *mut u32;
        self.open = self.at;
        self.draws = 0;
    }
}

/// A ground batch as the first pass sent it, to be sent again under the shadow picture.
#[derive(Clone, Copy)]
struct Sent {
    /// The cell its positions count from; `u32::MAX` for cut triangles, whose positions count
    /// from the eye: `count` fans, their sizes at `indices` as bytes.
    cell: u32,
    vertices: *const u8,
    indices: *const u16,
    count: u32,
    near: bool,
}

/// A vertex the guard has looked at, kept for the other triangles that have it.
#[derive(Clone, Copy)]
struct Seen {
    /// The vertex's index, and which run of triangles it was looked at for (0: none).
    key: u32,
    corner: Corner,
}

pub struct Gfx {
    pub scene: Scene,
    textures: Kept<Texture>,
    texels: Kept<u8>,
    chunks: Kept<Chunk>,
    clusters: Kept<Cluster>,
    larges: Kept<Large>,
    larges_at: Kept<u32>,
    by_cell: view::Cells,
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
    pub sub: Sub,
    /// Draws of the world in a run of `sub`.
    pub run: u32,
    pub fx: Fx,
    pub stats: Stats,
    /// How far the levels of detail are pulled in to keep a frame's triangles under `budget`,
    /// and its time under what the caller says a frame may take (`load`: the share of that the
    /// frame before took).
    pub nearer: f32,
    pub budget: u32,
    pub load: f32,
    /// What the soldiers and the held weapon of the frame before count for in the budget: the GE
    /// blends four bones and two lights into each of their corners, so a triangle of theirs
    /// counts for one and a half of the world's.
    pub figures: u32,
    pub resident_bytes: usize,
    /// Development switches (`option=` on the control line): 1 no culling of back faces, 2 the
    /// other winding, 4 every chunk and cluster the view holds, 8 no haze, 16 no shadow pass,
    /// 32 no texture, 64 no cutting on the CPU, 128 no sky, 256 no interface in the list,
    /// 512 no turn for the interface, 1024 no effects, 2048 large triangles the GE cannot take
    /// are left out, not cut (what the cutting costs is the difference), 4096 faces seen from
    /// behind are cut like the others.
    pub option: u32,
    /// What `choose` kept for `world`, and when the list was begun.
    chosen: Option<(Camera, Mat4, Guard, Guard)>,
    pub list_from: u32,
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
    let mut out = alloc::vec![0u8; 480 * 272 * 2];
    read_pixels(buffer, &mut out);
    out
}

/// The same frame into `out`, which holds 480 × 272 texels of 16 bits.
pub unsafe fn read_pixels(buffer: usize, out: &mut [u8]) {
    assert!(out.len() >= 480 * 272 * 2);
    let vram = sceGeEdramGetAddr().add(buffer * FB_BYTES);
    for y in 0..272 {
        ptr::copy_nonoverlapping(vram.add(y * 512 * 2), out.as_mut_ptr().add(y * 480 * 2), 480 * 2);
    }
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

impl Gfx {
    /// Reads what the world is drawn from. `init` has set the GE up, or will.
    pub unsafe fn load(file: &store::PackFile, scene: Scene, progress: &mut dyn FnMut(&str)) -> Result<Gfx, &'static str> {
        progress("Reading the pictures");
        let textures: Kept<Texture> = file.resident(pack::TEXT)?;
        let texels: Kept<u8> = file.resident(pack::TEXD)?;
        if texels.as_ptr() as usize & 15 != 0 {
            return Err("the pack's pictures");
        }
        progress("Reading the map");
        let chunks: Kept<Chunk> = file.resident(pack::CHNK)?;
        let clusters: Kept<Cluster> = file.resident(pack::CLUS)?;
        let larges: Kept<Large> = file.resident(pack::LTRI)?;
        let larges_at: Kept<u32> = file.resident(pack::LTRO)?;
        if chunks.as_ptr() as usize & 63 != 0 || larges_at.len() != clusters.len() {
            return Err("the pack's chunks");
        }
        let by_cell = view::Cells::new(&scene, &chunks);
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
        let resident_bytes = texels.len() + chunks.len() * core::mem::size_of::<Chunk>() + clusters.len() * core::mem::size_of::<Cluster>() + larges.len() * 8 + larges_at.len() * 4 + vertices.len() * 12 + indices.len() * 2 + lists.len() * 2 + bits.len();
        sceKernelDcacheWritebackAll();
        Ok(Gfx {
            scene,
            textures,
            texels,
            chunks,
            clusters,
            larges,
            larges_at,
            by_cell,
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
            sub: Sub::new(),
            run: 48,
            fx: Fx::new(),
            stats: Stats::default(),
            nearer: 1.0,
            budget: 22_500,
            load: 0.0,
            figures: 0,
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

    /// The same, into a run of `sub`.
    unsafe fn bind_in(&self, sub: &mut Sub, index: u32) {
        const ADDRESS: [GeCommand; 8] = [GeCommand::TexAddr0, GeCommand::TexAddr1, GeCommand::TexAddr2, GeCommand::TexAddr3, GeCommand::TexAddr4, GeCommand::TexAddr5, GeCommand::TexAddr6, GeCommand::TexAddr7];
        const WIDTH: [GeCommand; 8] = [GeCommand::TexBufWidth0, GeCommand::TexBufWidth1, GeCommand::TexBufWidth2, GeCommand::TexBufWidth3, GeCommand::TexBufWidth4, GeCommand::TexBufWidth5, GeCommand::TexBufWidth6, GeCommand::TexBufWidth7];
        const SIZE: [GeCommand; 8] = [GeCommand::TexSize0, GeCommand::TexSize1, GeCommand::TexSize2, GeCommand::TexSize3, GeCommand::TexSize4, GeCommand::TexSize5, GeCommand::TexSize6, GeCommand::TexSize7];
        let t = &self.textures[index as usize];
        let base = self.texels.as_ptr().add(t.offset as usize) as u32;
        sub.i(GeCommand::ClutFormat, ClutPixelFormat::Psm8888 as u32 | 0xff << 8);
        sub.i(GeCommand::ClutAddr, base);
        sub.i(GeCommand::ClutAddrUpper, (base >> 8) & 0xf0000);
        sub.i(GeCommand::LoadClut, 32);
        sub.i(GeCommand::TexMode, (t.levels as u32 - 1) << 16 | 1);
        sub.i(GeCommand::TexFormat, TexturePixelFormat::PsmT8 as u32);
        sub.i(GeCommand::TexFlush, 0);
        for l in 0..(t.levels as usize).min(8) {
            let (at, _) = t.level(l as u32);
            let (w, h) = ((t.width as u32 >> l).max(1), (t.height as u32 >> l).max(1));
            let texels = base + at as u32;
            sub.i(ADDRESS[l], texels);
            sub.i(WIDTH[l], (texels >> 8) & 0x0f0000 | w);
            sub.i(SIZE[l], (31 - (h & 0x3ff).leading_zeros()) << 8 | (31 - (w & 0x3ff).leading_zeros()));
            sub.i(GeCommand::TexFlush, 0);
        }
        let wrap = if t.flags & texture::CLAMP != 0 { GuTexWrapMode::Clamp as u32 } else { GuTexWrapMode::Repeat as u32 };
        sub.i(GeCommand::TexWrap, wrap << 8 | wrap);
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
        view::World { scene: &self.scene, chunks: &self.chunks, clusters: &self.clusters, larges_at: &self.larges_at, cells: &self.cells, lists: &self.lists, bits: &self.bits }
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
        let mut by_cell = core::mem::replace(&mut self.by_cell, view::Cells::none());
        let counts = view::select(&self.pack_world(), &mut by_cell, &planes, camera.eye, &near_guard, &view::Options { nearer: self.nearer, everything: self.option & 4 != 0 }, &mut draws);
        self.by_cell = by_cell;
        self.stats.chosen = sceKernelGetSystemTimeLow().wrapping_sub(t0);
        // Which of the frame's passes takes each draw: the near one for the cells about the eye
        // (bit 0), the glass's (bit 1).
        self.pass_of.clear();
        // The far pass's near plane goes under the nearest of its chunks: a chunk's triangles may
        // reach out of its cell's square, and nearer the eye than the square is.
        let mut least = SPLIT * SPLIT;
        let (mut chunk, mut pass) = (u32::MAX, 0u8);
        for d in &draws {
            // (a chunk's draws follow each other)
            if d.chunk as u32 != chunk {
                chunk = d.chunk as u32;
                let c = &self.chunks[d.chunk as usize];
                let near = near_cell(&self.scene, camera.eye, c.cell as u32);
                if !near {
                    least = min(least, mat::box_distance2(camera.eye, &c.min, &c.max));
                }
                pass = near as u8 | ((c.flags & chunk::BLEND != 0) as u8) << 1;
            }
            self.pass_of.push(pass);
        }
        let least = sqrt(least);
        self.far_near = max(least * FAR_NEAR, 1.0);
        self.draws = draws;
        // Over the budget of triangles, or of time: the frame before took `load` of what a frame may.
        let over = max((counts.triangles + self.figures) as f32 / self.budget.max(1) as f32, self.load);
        self.nearer = clamp(self.nearer * if over > 1.0 { 1.0 / (1.0 + (over - 1.0) * 0.5) } else { 1.0 + (1.0 - over) * 0.08 }, NEARER_LEAST, 1.0);
        let far_guard = Guard::new(&vp, self.far_near, 480.0, 272.0, 2048.0).boxes_along(&self.scene.level_x, &self.scene.level_z);
        self.stats.counts = counts;
        self.chosen = Some((*camera, view, near_guard, far_guard));
        self.stats.select = sceKernelGetSystemTimeLow().wrapping_sub(t0);
    }

    /// Ends a part of the list's work: the time since the last mark is that part's.
    pub unsafe fn mark(&mut self, part: usize) {
        let now = sceKernelGetSystemTimeLow();
        self.stats.parts[part] += now.wrapping_sub(self.stats.marked);
        self.stats.marked = now;
    }

    /// The model matrix of a cell's stored positions, the eye being the origin.
    unsafe fn cell_matrix(&self, sub: &mut Sub, cell: u32, eye: V3) {
        let c = self.scene.cell_centre(cell);
        // A position is a signed 16-bit number of lattice steps: the GE reads it as -1…1.
        let s = 32768.0 * self.scene.quantum;
        sub.matrix(GeCommand::WorldMatrixNumber, GeCommand::WorldMatrixData, 0, &[[s, 0.0, 0.0], [0.0, s, 0.0], [0.0, 0.0, s], [c[0] - eye.x, c[1] - eye.y, c[2] - eye.z]]);
    }

    /// The model matrix of positions counted from the eye.
    unsafe fn eye_matrix_in(sub: &mut Sub) {
        sub.matrix(GeCommand::WorldMatrixNumber, GeCommand::WorldMatrixData, 0, &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, 0.0]]);
    }

    unsafe fn eye_matrix(&self) {
        let id = ScePspFMatrix4 { x: vec4(1.0, 0.0, 0.0, 0.0), y: vec4(0.0, 1.0, 0.0, 0.0), z: vec4(0.0, 0.0, 1.0, 0.0), w: vec4(0.0, 0.0, 0.0, 1.0) };
        sceGuSetMatrix(MatrixMode::Model, &id);
    }

    /// The solid world of one pass: `near` for the chunks about the eye. Its commands go through
    /// `sub`; the last run is closed when this returns.
    unsafe fn pass(&mut self, near: bool, eye: V3, guard: &Guard, blend: bool) {
        let scene = self.scene;
        let mut sub = core::mem::take(&mut self.sub);
        sub.run = self.run;
        let (mut bound, mut cell, mut chunk_at, mut vertices_at, mut flags) = (u32::MAX, u32::MAX, u32::MAX, ptr::null::<u8>(), u16::MAX);
        // The triangles cut on the CPU, picture by picture: the corners of what is left of each,
        // and how many corners that is.
        let cut = self.scratch.take(CUT_VERTICES * core::mem::size_of::<ClipVertex>()) as *mut ClipVertex;
        let fans = self.scratch.take(CUT_VERTICES / 3);
        let (mut cut_n, mut fan_n, mut cut_first, mut fan_first, mut cut_ground) = (0usize, 0usize, 0usize, 0usize, false);
        let no_cut = self.option & 64 != 0 || cut.is_null() || fans.is_null();
        // The corners the guard has looked at: a cluster's large triangles share most of theirs,
        // and a vertex read from the pack is a cache line the GE's own reading has to make way for.
        let mut seen = [Seen { key: 0, corner: Corner { clip: [0.0; 3], out: 0 } }; 64];
        let (mut run, mut run_vertices, mut run_cell) = (0u32, ptr::null::<u8>(), u32::MAX);
        // The eye among the cell's stored positions, in sixteenths of a unit of the large
        // triangles' records, and how many of those sixteenths of distance a unit of edge asks
        // for (times 256): a triangle whose sphere is farther than its longest edge over
        // `Guard::chord` cannot reach from the view to outside the band.
        const FINE: i32 = large::UNIT as i32 / 8;
        let mut eye_in = [0i32; 3];
        let per_edge = (8.0 / guard.chord().max(0.05) * 256.0) as u32 + 1;
        // The guard for positions as the cell stores them.
        let (mut stored, mut stored_cell) = (guard.stored([0.0; 3], 1.0), u32::MAX);
        let (mut tested, mut looked_at, mut looked, mut draws, mut triangles) = (0u32, 0u32, 0u32, 0u32, 0u32);
        let want = near as u8 | (blend as u8) << 1;
        // (the texture stage doubles what it makes while the world is drawn)
        let doubled = 0x10000u32;
        for i in 0..self.draws.len() {
            if *self.pass_of.get_unchecked(i) != want {
                continue;
            }
            if !sub.room() {
                break;
            }
            let d = *self.draws.get_unchecked(i);
            // (the chunks stand still for the program's life)
            let c = &*self.chunks.as_ptr().add(d.chunk as usize);
            if c.texture as u32 != bound {
                // The cut triangles of the picture before go out before it is replaced.
                self.send_cut(&mut sub, cut.add(cut_first), fans.add(fan_first), fan_n - fan_first, near, cut_ground);
                (cut_first, fan_first, cut_ground) = (cut_n, fan_n, false);
                bound = c.texture as u32;
                self.bind_in(&mut sub, bound);
                flags = u16::MAX;
                cell = u32::MAX;
                chunk_at = u32::MAX;
                vertices_at = ptr::null();
            }
            let state = c.flags & (chunk::TWO_SIDED | chunk::CUTOUT | chunk::BLEND);
            if state != flags {
                flags = state;
                sub.i(GeCommand::CullFaceEnable, (state & chunk::TWO_SIDED == 0 && self.option & 1 == 0) as u32);
                let cutout = state & chunk::CUTOUT != 0;
                sub.i(GeCommand::AlphaTestEnable, cutout as u32);
                let component = if cutout { TextureColorComponent::Rgba } else { TextureColorComponent::Rgb };
                sub.i(GeCommand::TexFunc, (component as u32) << 8 | TextureEffect::Modulate as u32 | doubled);
            }
            if blend {
                // Glass: its colour by its opacity over what is behind it.
                let a = c.opacity as u32;
                sub.i(GeCommand::BlendMode, BlendFactor::Fix as u32 | (BlendFactor::Fix as u32) << 4 | (BlendOp::Add as u32) << 8);
                sub.i(GeCommand::BlendFixedA, a | a << 8 | a << 16);
                sub.i(GeCommand::BlendFixedB, (255 - a) | (255 - a) << 8 | (255 - a) << 16);
            }
            if c.cell as u32 != cell {
                cell = c.cell as u32;
                self.cell_matrix(&mut sub, cell, eye);
            }
            if d.chunk as u32 != chunk_at {
                chunk_at = d.chunk as u32;
                sub.f(GeCommand::TexScaleU, c.uv_scale[0]);
                sub.f(GeCommand::TexScaleV, c.uv_scale[1]);
                sub.f(GeCommand::TexOffsetU, c.uv_offset[0]);
                sub.f(GeCommand::TexOffsetV, c.uv_offset[1]);
            }
            let lod = c.lod(d.lod as usize);
            let vertices = self.vertices.as_ptr().add(lod.vertex as usize) as *const u8;
            if vertices != vertices_at {
                vertices_at = vertices;
                sub.address(GeCommand::Vaddr, vertices as usize);
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
                    eye_in = [((eye.x - centre[0]) / scene.quantum) as i32, ((eye.y - centre[1]) / scene.quantum) as i32, ((eye.z - centre[2]) / scene.quantum) as i32];
                }
                let records = vertices as *const WorldVertex;
                let spheres = self.larges.as_ptr().add(d.tri as usize);
                if vertices != run_vertices || cell != run_cell {
                    (run, run_vertices, run_cell) = (run + 1, vertices, cell);
                }
                let large = d.large as usize;
                // The triangles the GE takes as they are: copied only from the first one it cannot.
                let (mut keep, mut kept) = (ptr::null_mut::<u16>(), 0usize);
                tested += large as u32;
                for t in 0..large {
                    // Far enough for its size: the GE has it without a look at its corners.
                    let sphere = &*spheres.add(t);
                    let (dx, dy, dz) = ((sphere.centre[0] as i32 - eye_in[0]) / FINE, (sphere.centre[1] as i32 - eye_in[1]) / FINE, (sphere.centre[2] as i32 - eye_in[2]) / FINE);
                    let reach = sphere.radius as u32 * 8 + ((sphere.edge as u32 * per_edge) >> 8) + 3;
                    if sphere.radius != 255 && sphere.edge != 255 && (dx * dx) as u32 + (dy * dy) as u32 + (dz * dz) as u32 >= reach * reach {
                        if !keep.is_null() {
                            ptr::copy_nonoverlapping(indices.add(t * 3), keep.add(kept), 3);
                            kept += 3;
                        }
                        continue;
                    }
                    looked_at += 1;
                    let index = [*indices.add(t * 3), *indices.add(t * 3 + 1), *indices.add(t * 3 + 2)];
                    let mut corner = [Corner { clip: [0.0; 3], out: 0 }; 3];
                    for k in 0..3 {
                        let slot = seen.get_unchecked_mut(index[k] as usize & 63);
                        let key = run << 16 | index[k] as u32;
                        if slot.key != key {
                            let v = &*records.add(index[k] as usize);
                            slot.key = key;
                            looked += 1;
                            slot.corner = stored.corner(v3(v.pos[0] as f32, v.pos[1] as f32, v.pos[2] as f32));
                        }
                        corner[k] = slot.corner;
                    }
                    let verdict = stored.judge(&corner[0], &corner[1], &corner[2]);
                    if verdict == Verdict::Safe {
                        if !keep.is_null() {
                            ptr::copy_nonoverlapping(index.as_ptr(), keep.add(kept), 3);
                            kept += 3;
                        }
                        continue;
                    }
                    if keep.is_null() {
                        keep = self.scratch.take(large * 6) as *mut u16;
                        if keep.is_null() {
                            break;
                        }
                        ptr::copy_nonoverlapping(indices, keep, t * 3);
                        kept = t * 3;
                    }
                    if verdict == Verdict::Culled || self.option & 2048 != 0 || cut_n + MAX_POLY > CUT_VERTICES {
                        continue;
                    }
                    let clip = [corner[0].clip, corner[1].clip, corner[2].clip];
                    // A face seen from behind that the GE would not draw is not worth the cut.
                    if c.flags & chunk::TWO_SIDED == 0 && self.option & (1 | 4096) == 0 && Guard::turned(&clip) != (self.option & 2 != 0) {
                        continue;
                    }
                    // The pieces are sent with the eye as the origin.
                    let from = [centre[0] - eye.x, centre[1] - eye.y, centre[2] - eye.z];
                    let full = |k: usize| {
                        let v = &*records.add(index[k] as usize);
                        let (r, g, b) = ((v.colour & 31) as u8, ((v.colour >> 5) & 63) as u8, (v.colour >> 11) as u8);
                        ClipVertex {
                            uv: [v.uv[0] as f32 * (c.uv_scale[0] / 32768.0) + c.uv_offset[0], v.uv[1] as f32 * (c.uv_scale[1] / 32768.0) + c.uv_offset[1]],
                            color: [(r << 3) | (r >> 2), (g << 2) | (g >> 4), (b << 3) | (b >> 2), 255],
                            pos: [from[0] + v.pos[0] as f32 * scene.quantum, from[1] + v.pos[1] as f32 * scene.quantum, from[2] + v.pos[2] as f32 * scene.quantum],
                        }
                    };
                    let n = stored.cut(&[full(0), full(1), full(2)], &clip, &mut *(cut.add(cut_n) as *mut [ClipVertex; MAX_POLY]));
                    if n >= 3 {
                        *fans.add(fan_n) = n as u8;
                        fan_n += 1;
                        cut_n += n;
                        cut_ground |= ground;
                        triangles += n as u32 - 2;
                        self.stats.cut += 1;
                    }
                }
                if keep.is_null() {
                    // Every one of them is the GE's: the run goes out whole.
                } else {
                    if kept > 0 {
                        flush(keep, kept * 2);
                        sub.triangles(keep, kept as u32);
                        if ground {
                            self.ground.push(Sent { cell, vertices, indices: keep, count: kept as u32, near });
                        }
                        draws += 1;
                        triangles += kept as u32 / 3;
                    }
                    first = large as u32 * 3;
                }
            }
            // The rest of the run, as it is: at most 65 535 indices in one draw.
            let mut at = first;
            while at < d.count {
                let count = (d.count - at).min(65_532);
                sub.triangles(indices.add(at as usize), count);
                if ground {
                    self.ground.push(Sent { cell, vertices, indices: indices.add(at as usize), count, near });
                }
                at += count;
                draws += 1;
                triangles += count / 3;
            }
        }
        self.stats.tested += tested;
        self.stats.near += looked_at;
        self.stats.corners += looked;
        self.stats.draws += draws;
        self.stats.triangles += triangles;
        if !no_cut {
            self.send_cut(&mut sub, cut.add(cut_first), fans.add(fan_first), fan_n - fan_first, near, cut_ground);
        }
        sub.close();
        self.sub = sub;
    }

    /// Sends `count` pieces cut on the CPU, each a fan of as many corners as its byte at `fans`
    /// says: float vertices from `at` on, with the eye as the origin.
    unsafe fn send_cut(&mut self, sub: &mut Sub, at: *const ClipVertex, fans: *const u8, count: usize, near: bool, ground: bool) {
        if count == 0 {
            return;
        }
        Gfx::eye_matrix_in(sub);
        sub.f(GeCommand::TexScaleU, 1.0);
        sub.f(GeCommand::TexScaleV, 1.0);
        sub.f(GeCommand::TexOffsetU, 0.0);
        sub.f(GeCommand::TexOffsetV, 0.0);
        sub.i(GeCommand::VertexType, vtype_float().bits() as u32);
        let corners = Gfx::fans_in(sub, at, fans, count);
        sub.i(GeCommand::VertexType, vtype_world().bits() as u32);
        flush(at, corners * core::mem::size_of::<ClipVertex>());
        if ground {
            self.ground.push(Sent { cell: u32::MAX, vertices: at as *const u8, indices: fans as *const u16, count: count as u32, near });
        }
        self.stats.draws += count as u32;
    }

    /// The fans themselves; returns how many corners they have between them.
    unsafe fn fans_in(sub: &mut Sub, at: *const ClipVertex, fans: *const u8, count: usize) -> usize {
        let mut corner = 0usize;
        for k in 0..count {
            if !sub.room() {
                break;
            }
            let n = *fans.add(k) as usize;
            sub.fan(at.add(corner) as *const u8, n as u32);
            corner += n;
        }
        corner
    }

    /// The ground of one pass once more, under the shadow picture.
    unsafe fn shadows(&mut self, near: bool, eye: V3) {
        let (r, s) = (self.scene.shadow_rows, 32768.0 * self.scene.quantum);
        let mut sub = core::mem::take(&mut self.sub);
        let mut cell = u32::MAX - 1;
        let mut vertices_at = ptr::null::<u8>();
        let mut draws = 0;
        for i in 0..self.ground.len() {
            let g = *self.ground.get_unchecked(i);
            if g.near != near {
                continue;
            }
            if !sub.room() {
                break;
            }
            if g.cell != cell {
                cell = g.cell;
                // Where a stored position stands in the world, and from that in the picture.
                let (scale, at) = if cell == u32::MAX {
                    Gfx::eye_matrix_in(&mut sub);
                    (1.0, [eye.x, eye.y, eye.z])
                } else {
                    self.cell_matrix(&mut sub, cell, eye);
                    (s, self.scene.cell_centre(cell))
                };
                sub.matrix(
                    GeCommand::TGenMatrixNumber,
                    GeCommand::TGenMatrixData,
                    0,
                    &[[r[0][0] * scale, r[1][0] * scale, 0.0], [0.0, 0.0, 0.0], [r[0][1] * scale, r[1][1] * scale, 0.0], [r[0][0] * at[0] + r[0][1] * at[2] + r[0][2], r[1][0] * at[0] + r[1][1] * at[2] + r[1][2], 1.0]],
                );
                vertices_at = ptr::null();
            }
            if g.cell == u32::MAX {
                sub.i(GeCommand::VertexType, vtype_float().bits() as u32);
                Gfx::fans_in(&mut sub, g.vertices as *const ClipVertex, g.indices as *const u8, g.count as usize);
                sub.i(GeCommand::VertexType, vtype_world().bits() as u32);
                vertices_at = ptr::null();
            } else {
                if g.vertices != vertices_at {
                    vertices_at = g.vertices;
                    sub.address(GeCommand::Vaddr, g.vertices as usize);
                }
                sub.triangles(g.indices, g.count);
            }
            draws += 1;
        }
        self.stats.draws += draws;
        sub.close();
        self.sub = sub;
    }

    /// Starts the list of the frame chosen and writes the sky and the world into it. No list is in
    /// flight when this is called. Returns the frame's eye and its view matrix, for what the caller
    /// draws next: the soldiers, then `effects`, the held weapon, and `finish`.
    pub unsafe fn world(&mut self) -> Option<(Camera, Mat4)> {
        let (camera, view, near_guard, far_guard) = self.chosen.take()?;
        self.list_from = sceKernelGetSystemTimeLow();
        self.stats.marked = self.list_from;
        self.sub.rewind();
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

        self.mark(0);

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
            self.mark(if near { 1 } else { 3 });
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
            self.mark(if near { 2 } else { 4 });
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
        self.mark(5);
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
        self.mark(9);
        self.stats.words = (sceGuFinish() / 4) as u32;
        self.stats.list = sceKernelGetSystemTimeLow().wrapping_sub(self.list_from);
    }

    /// What this console takes for the frame's smallest pieces of work, in nanoseconds each
    /// (`probe=1` in `boot.txt`): a command through `sceGuSendCommandi`; a word written past the
    /// cache; a word written through it and flushed; a flush of 64 bytes; a model matrix; a large
    /// triangle's three corners carried to clip space and judged; a triangle cut; a multiply and
    /// add; a divide; a read from a far place of the pack; a call of the kernel's clock.
    pub unsafe fn probe(&mut self, camera: &Camera) -> [u32; 11] {
        const N: usize = 16384;
        static mut WORDS: [u32; N] = [0; N];
        let words = ptr::addr_of_mut!(WORDS) as *mut u32;
        let mut out = [0u32; 11];
        let per = |from: u32, n: usize| (sceKernelGetSystemTimeLow().wrapping_sub(from) as u64 * 1000 / n as u64) as u32;
        // 0: commands through the library.
        sceGuStart(GuContextType::Direct, DisplayList::as_mut_ptr(ptr::addr_of_mut!(LIST)));
        let t = sceKernelGetSystemTimeLow();
        for _ in 0..N {
            sceGuSendCommandi(GeCommand::Nop, 0);
        }
        out[0] = per(t, N);
        // 4: model matrices.
        let m = ScePspFMatrix4 { x: vec4(1.0, 0.0, 0.0, 0.0), y: vec4(0.0, 1.0, 0.0, 0.0), z: vec4(0.0, 0.0, 1.0, 0.0), w: vec4(0.0, 0.0, 0.0, 1.0) };
        let t = sceKernelGetSystemTimeLow();
        for _ in 0..1024 {
            sceGuSetMatrix(MatrixMode::Model, &m);
        }
        out[4] = per(t, 1024);
        sceGuFinish();
        sceGuSync(GuSyncMode::Finish, GuSyncBehavior::Wait);
        // 1: words past the cache.
        sceKernelDcacheWritebackInvalidateRange(words as *const c_void, (N * 4) as u32);
        let past = (words as usize | 0x4000_0000) as *mut u32;
        let t = sceKernelGetSystemTimeLow();
        for i in 0..N {
            past.add(i).write_volatile(i as u32);
        }
        out[1] = per(t, N);
        // 2: words through the cache, flushed once.
        let t = sceKernelGetSystemTimeLow();
        for i in 0..N {
            words.add(i).write_volatile(i as u32 ^ 0x5555);
        }
        flush(words, N * 4);
        out[2] = per(t, N);
        // 3: flushes of 64 bytes.
        let t = sceKernelGetSystemTimeLow();
        for i in 0..1024 {
            words.add((i & 255) * 16).write_volatile(i as u32);
            flush(words.add((i & 255) * 16), 64);
        }
        out[3] = per(t, 1024);
        // 5 and 6: the guard's work on the pack's own triangles.
        let look = forward(camera.yaw, camera.pitch);
        let view = mat::view(camera.eye, look, camera.roll);
        let vp = mat::mul(&mat::perspective(camera.fov, 480.0 / 272.0, NEAR, FAR), &view);
        let guard = Guard::new(&vp, NEAR, 480.0, 272.0, 2048.0);
        let cell = self.scene.cell_at(camera.eye.x, camera.eye.z).unwrap_or(0);
        let stored = guard.stored(self.scene.cell_centre(cell), self.scene.quantum);
        let (vn, mut seed, mut sum) = (self.vertices.len().min(60_000), 12345u32, 0u32);
        let mut next = || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed >> 8) as usize
        };
        let t = sceKernelGetSystemTimeLow();
        for _ in 0..8192 {
            let at = next() % (vn - 3);
            let v = [&self.vertices[at], &self.vertices[at + 1], &self.vertices[at + 2]];
            let steps = |v: &WorldVertex| v3(v.pos[0] as f32, v.pos[1] as f32, v.pos[2] as f32);
            let clip = [stored.to_clip(steps(v[0])), stored.to_clip(steps(v[1])), stored.to_clip(steps(v[2]))];
            sum += (stored.classify(&clip) == Verdict::Safe) as u32;
        }
        out[5] = per(t, 8192);
        let corner = |x: f32, y: f32, z: f32| {
            let p = camera.eye + v3(x, y, z);
            ClipVertex { uv: [x, z], color: [200, 150, 100, 255], pos: [p.x, p.y, p.z] }
        };
        let right = look.cross(V3::UP).norm_or(v3(1.0, 0.0, 0.0));
        let (a, b, c) = (look * 3.0 - right * 40.0, look * 3.0 + right * 2.0, look * -2.0 + right * 1.0 - V3::UP * 1.5);
        let tri = [corner(a.x, a.y, a.z), corner(b.x, b.y, b.z), corner(c.x, c.y, c.z)];
        let clip = tri.map(|v| guard.to_clip(v3(v.pos[0], v.pos[1], v.pos[2])));
        let mut pieces = [ClipVertex::default(); MAX_OUT];
        let t = sceKernelGetSystemTimeLow();
        for _ in 0..1024 {
            sum += guard.clip(&tri, &clip, &mut pieces) as u32;
        }
        out[6] = per(t, 1024);
        // 7 and 8: the floating point unit.
        let (mut acc, a, b) = (1.0f32, core::hint::black_box(0.999f32), core::hint::black_box(0.001f32));
        let t = sceKernelGetSystemTimeLow();
        for _ in 0..65536 {
            acc = core::hint::black_box(acc * a + b);
        }
        out[7] = per(t, 65536);
        let t = sceKernelGetSystemTimeLow();
        for _ in 0..65536 {
            acc = core::hint::black_box(b / (acc + a));
        }
        out[8] = per(t, 65536);
        // 9: reads from far places.
        let all = self.vertices.len();
        let t = sceKernelGetSystemTimeLow();
        for _ in 0..N {
            sum = sum.wrapping_add(self.vertices[next() % all].colour as u32);
        }
        out[9] = per(t, N);
        // 10: the clock itself.
        let t = sceKernelGetSystemTimeLow();
        for _ in 0..1024 {
            sum = sum.wrapping_add(sceKernelGetSystemTimeLow());
        }
        out[10] = per(t, 1024);
        core::hint::black_box((sum, acc));
        out
    }
}
