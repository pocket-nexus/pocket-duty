//! C interface of the 3DS host (`../src/core.h` declares the same functions).
//!
//! The host owns the GPU, the pad, the sound output and storage; it hands the
//! pack's sections in once and then asks, each frame, where the eye is, what
//! to draw of the street, where every soldier's bones are, and what the
//! effects are. The match (`duty_sim::game`), the choice of the frame's draws
//! (`duty_sim::view`), the posing (`duty_sim::pose`) and the mixing
//! (`duty_sim::mix`) are the same code as the PSP's.
//!
//! The interface is a PocketJS guest the host runs beside this library: its
//! service channel is answered here (`wire.rs` exports the `svcwire_*`
//! functions PocketJS's guest driver calls), and the flow around the match is
//! `duty_interface::Session`.

#![no_std]

extern crate alloc;

#[path = "alloc.rs"]
mod allocator;
mod wire;

use alloc::string::String;
use alloc::vec::Vec;
use core::ffi::c_char;
use core::fmt::Write;
use core::sync::atomic::{AtomicU32, Ordering};

use duty_interface::{channel, pad, parse_f32, Command, Mode, Pace, Pad, Session};
use duty_pack::figure::{bone, Pose};
use duty_pack::sound::{Id, Sound};
use duty_pack::{Chunk, Cluster, Scene, Texture, ViewCell};
use duty_sim::bvh::{Bvh, Node, Tri};
use duty_sim::clip::Guard;
use duty_sim::fx::{self, Fx, FxVertex};
use duty_sim::game::Game;
use duty_sim::hear;
use duty_sim::level::Level;
use duty_sim::mat::{self, Mat4};
use duty_sim::math::*;
use duty_sim::mix::{self, Mixer, UNITY};
use duty_sim::pose::{self, Figures};
use duty_sim::tune::{WeaponDef, DT};
use duty_sim::view::{self, Draw};
use duty_sim::weapon::{self, WEAPONS};

/// Degrees, top to bottom, of the widest view.
const FOV: f32 = 65.0;
const ASPECT: f32 = 400.0 / 240.0;
pub const NEAR: f32 = 0.12;
pub const FAR: f32 = 420.0;
/// Batches a figure has at most (the held rifle has nine).
const BATCHES: usize = 12;
/// Soldiers and the held weapon a frame draws at most.
const FIGURES: usize = duty_sim::soldier::MAX + 1;
/// Sounds heard at once, and the bed's priority.
const VOICES: usize = 16;
const BED: u8 = u8::MAX;

/// The pack's sections as the host read them. Each stays where it is for the run.
#[repr(C)]
pub struct PackIn {
    scene: *const Scene,
    chunks: *const Chunk,
    chunk_count: u32,
    clusters: *const Cluster,
    cluster_count: u32,
    cells: *const ViewCell,
    cell_count: u32,
    lists: *const u16,
    list_count: u32,
    bits: *const u8,
    bit_bytes: u32,
    textures: *const Texture,
    texture_count: u32,
    texels: *const u8,
    texel_bytes: u32,
    solid_verts: *const [f32; 3],
    solid_vert_count: u32,
    solid_tris: *const Tri,
    solid_tri_count: u32,
    solid_nodes: *const Node,
    solid_node_count: u32,
    nav: *const u8,
    nav_bytes: u32,
    weapons: *const WeaponDef,
    weapon_bytes: u32,
    figures: *const u8,
    figure_bytes: u32,
    sounds: *const Sound,
    sound_bytes: u32,
    samples: *const u8,
    sample_bytes: u32,
}

/// The pad as the host reads it: `duty_interface::pad` bits, the Circle Pad, and the second stick
/// of a New 3DS (zeros without one), each -1…1.
#[repr(C)]
pub struct PadIn {
    held: u32,
    sx: f32,
    sy: f32,
    rx: f32,
    ry: f32,
}

/// One draw of the street: a run of a level's indices, with what the host sets before it.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct DrawOut {
    /// The level's first vertex and the run's first index, in the pack's sections.
    vertex: u32,
    index: u32,
    count: u32,
    texture: u16,
    cell: u16,
    /// `duty_pack::chunk` flags.
    flags: u16,
    opacity: u8,
    pad: u8,
    /// u = stored × `uv[0]` + `uv[1]`, v = stored × `uv[2]` + `uv[3]`.
    uv: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct BatchOut {
    /// Four bones, three rows of four each.
    bones: [[f32; 12]; 4],
    vertices: *const u8,
    indices: *const u16,
    count: u32,
}

/// A soldier, or the weapon in the player's hands.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FigureOut {
    /// Model to view, three rows of four (the eye is the view's origin).
    place: [f32; 12],
    /// In the figure's own space: toward the sun, and up.
    sun: [f32; 3],
    up: [f32; 3],
    /// How much of the sun reaches where it stands, 0…1.
    lit: f32,
    /// 1 for the held weapon, drawn in a frustum of `fov` degrees and a depth range of its own.
    held: u32,
    fov: f32,
    batches: u32,
    batch: [BatchOut; BATCHES],
}

/// The frame as the host draws it.
#[repr(C)]
#[derive(Default)]
pub struct ViewOut {
    eye: [f32; 3],
    /// World to view with the eye as the origin, row by row.
    view: [f32; 16],
    fov: f32,
    /// The haze's colour, where it starts and where it is whole, metres.
    haze: [f32; 3],
    haze_from: f32,
    haze_to: f32,
    sun_colour: [f32; 3],
    sky_colour: [f32; 3],
    /// A match is on the screen: its soldiers, its effects and the held weapon are drawn.
    stood: u32,
    /// The muzzle's flash: 0 or its half side, and where it is in the weapon's own view.
    flash: f32,
    flash_at: [f32; 3],
    /// Display refreshes a frame is shown for.
    pace: u32,
    option: u32,
    nearer: f32,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct Perf {
    frame: f32,
    worst: f32,
    late: u32,
    frames: u32,
    cpu: f32,
    gpu: f32,
    draws: u32,
    triangles: u32,
}

/// What the sound thread is told, through a queue neither side waits on.
#[derive(Clone, Copy)]
enum Heard {
    Play(hear::Play),
    Bed { id: u16, gain: u16 },
    Silence,
}

const QUEUE: usize = 64;

struct Core {
    scene: Scene,
    world: view::World<'static>,
    by_cell: view::Cells,
    textures: &'static [Texture],
    texels: &'static [u8],
    figures: Option<Figures<'static>>,
    figure_base: *const u8,
    game: Game<'static>,
    session: Session,
    pace: Pace,
    fx: Fx,
    draws: Vec<Draw>,
    out: Vec<DrawOut>,
    posed: Vec<FigureOut>,
    triangles: u32,
    figure_triangles: u32,
    nearer: f32,
    budget: u32,
    option: u32,
    /// The bench's player, and a player nothing hurts.
    auto: bool,
    god: bool,
    held: (Pad, u32),
    orbit: f32,
    sounds_played: u32,
    was_playing: bool,
    prefs_out: Option<String>,
    perf: Perf,
    frame: u32,
    sim_us: f32,
    select_us: f32,
    clock: Option<unsafe extern "C" fn() -> u32>,
}

static mut CORE: Option<Core> = None;
static mut HEARD: [Heard; QUEUE] = [Heard::Silence; QUEUE];
static WRITTEN: AtomicU32 = AtomicU32::new(0);
static TAKEN: AtomicU32 = AtomicU32::new(0);
static mut MIXER: Mixer<VOICES> = Mixer::new();
static mut SOUNDS: (&[Sound], &[u8]) = (&[], &[]);
static mut BED_VOICE: Option<(usize, u16)> = None;

unsafe fn core() -> Option<&'static mut Core> {
    (*core::ptr::addr_of_mut!(CORE)).as_mut()
}

fn hear(what: Heard) {
    let written = WRITTEN.load(Ordering::Relaxed);
    if written.wrapping_sub(TAKEN.load(Ordering::Acquire)) as usize >= QUEUE {
        return;
    }
    // SAFETY: the frame's thread alone writes the queue, and the sound thread reads a place only
    // after `WRITTEN` has moved past it.
    unsafe { (core::ptr::addr_of_mut!(HEARD) as *mut Heard).add(written as usize % QUEUE).write(what) };
    WRITTEN.store(written.wrapping_add(1), Ordering::Release);
}

unsafe fn slice<T>(p: *const T, n: u32) -> &'static [T] {
    if p.is_null() {
        &[]
    } else {
        core::slice::from_raw_parts(p, n as usize)
    }
}

/// The version of the pack this program reads (`duty_pack::VERSION`): the host checks the pack's head
/// against it, so the number is written once.
#[no_mangle]
pub extern "C" fn dt_pack_version() -> u32 {
    duty_pack::VERSION
}

/// Sizes of the records both sides lay out: the host checks them against `core.h` at start.
#[no_mangle]
pub extern "C" fn dt_sizes(out: &mut [u32; 8]) {
    *out = [
        core::mem::size_of::<PackIn>() as u32,
        core::mem::size_of::<DrawOut>() as u32,
        core::mem::size_of::<FigureOut>() as u32,
        core::mem::size_of::<ViewOut>() as u32,
        core::mem::size_of::<Scene>() as u32,
        core::mem::size_of::<Texture>() as u32,
        core::mem::size_of::<FxVertex>() as u32,
        fx::VERTICES as u32,
    ];
}

/// Starts a match on the pack. Returns why it could not, or null.
///
/// # Safety
/// Every section of `pack` stays where it is, unchanged, for the rest of the run.
#[no_mangle]
pub unsafe extern "C" fn dt_init(pack: &PackIn, clock: Option<unsafe extern "C" fn() -> u32>) -> *const c_char {
    if pack.scene.is_null() || pack.chunks.is_null() || pack.clusters.is_null() || pack.cells.is_null() {
        return c"the pack lacks a section".as_ptr();
    }
    let scene = *pack.scene;
    let chunks = slice(pack.chunks, pack.chunk_count);
    let world = view::World {
        scene: &*pack.scene,
        chunks,
        clusters: slice(pack.clusters, pack.cluster_count),
        // (the PICA200 cuts a triangle at every side of its view: no cluster is handed over to be tested)
        larges_at: &[],
        cells: slice(pack.cells, pack.cell_count),
        lists: slice(pack.lists, pack.list_count),
        bits: slice(pack.bits, pack.bit_bytes),
    };
    let solid = Bvh { nodes: slice(pack.solid_nodes, pack.solid_node_count), tris: slice(pack.solid_tris, pack.solid_tri_count), verts: slice(pack.solid_verts, pack.solid_vert_count) };
    let Some(level) = Level::parse_nav(solid, slice(pack.nav, pack.nav_bytes)) else {
        return c"the pack's walk grid".as_ptr();
    };
    let weapons = slice(pack.weapons, pack.weapon_bytes / core::mem::size_of::<WeaponDef>() as u32);
    let defs: [WeaponDef; WEAPONS] = if weapons.len() == WEAPONS { [weapons[0], weapons[1], weapons[2]] } else { weapon::reference() };
    let mut game = Game::new(level, defs, 0x5eed_1234);
    let mut session = Session::new();
    // The map picture's corner, as the compiler laid it out about the level's origin.
    session.map = (scene.level_origin[0] - 80.0, scene.level_origin[2] - 80.0);
    session.focal = 120.0 / tan(FOV * 0.5 * (PI / 180.0));
    session.apply(&mut game);
    let sounds = slice(pack.sounds, pack.sound_bytes / core::mem::size_of::<Sound>() as u32);
    *core::ptr::addr_of_mut!(SOUNDS) = if sounds.len() == Id::ALL.len() { (sounds, slice(pack.samples, pack.sample_bytes)) } else { (&[], &[]) };
    *core::ptr::addr_of_mut!(CORE) = Some(Core {
        scene,
        world,
        by_cell: view::Cells::new(&scene, chunks),
        textures: slice(pack.textures, pack.texture_count),
        texels: slice(pack.texels, pack.texel_bytes),
        figures: Figures::parse(slice(pack.figures, pack.figure_bytes)),
        figure_base: pack.figures,
        game,
        session,
        pace: Pace::new(),
        fx: Fx::new(),
        draws: Vec::with_capacity(4096),
        out: Vec::with_capacity(4096),
        posed: Vec::with_capacity(FIGURES),
        triangles: 0,
        figure_triangles: 0,
        nearer: 1.0,
        budget: 36_000,
        option: 0,
        auto: false,
        god: false,
        held: (Pad::default(), 0),
        orbit: 0.0,
        sounds_played: 0,
        was_playing: false,
        prefs_out: None,
        perf: Perf::default(),
        frame: 0,
        sim_us: 0.0,
        select_us: 0.0,
        clock,
    });
    hear(Heard::Bed { id: Id::Ambience as u16, gain: 70 });
    core::ptr::null()
}

/// Before the match exists: the loading step, or why the start failed.
///
/// # Safety
/// `message` points at `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn dt_stage(error: u32, message: *const u8, len: u32) {
    let state = &mut channel().state;
    state.mode = if error != 0 { Mode::Error } else { Mode::Loading };
    state.message.clear();
    state.message.push_str(core::str::from_utf8(core::slice::from_raw_parts(message, len as usize)).unwrap_or(""));
}

/// The preferences read from storage at the start.
///
/// # Safety
/// `text` points at `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn dt_prefs_stored(text: *const u8, len: u32) {
    let state = &mut channel().state;
    state.prefs.clear();
    state.prefs.push_str(core::str::from_utf8(core::slice::from_raw_parts(text, len as usize)).unwrap_or(""));
}

/// What the interface asked to have stored since the last call: its length, 0 for nothing.
///
/// # Safety
/// `out` has room for `cap` bytes.
#[no_mangle]
pub unsafe extern "C" fn dt_prefs_take(out: *mut u8, cap: u32) -> u32 {
    let Some(text) = core().and_then(|c| c.prefs_out.take()) else { return 0 };
    let n = text.len().min(cap as usize);
    core::ptr::copy_nonoverlapping(text.as_ptr(), out, n);
    n as u32
}

/// A guest holds the interface's channel.
#[no_mangle]
pub extern "C" fn dt_interface_open() -> u32 {
    unsafe { channel() }.is_open() as u32
}

/// Whether the guest's next turn is worth taking (`duty_interface::Pace`). `buttons`: PocketJS's
/// bits as held since the last offer; `touching`: a contact on the lower screen.
#[no_mangle]
pub extern "C" fn dt_guest_due(buttons: u32, touching: u32, every: u32) -> u32 {
    let Some(c) = (unsafe { core() }) else { return 1 };
    // A contact is news of its own: the lower screen's keys answer it.
    (touching != 0 || c.pace.due(&c.session, buttons, every as u8)) as u32
}

/// The words of a control text: `mode=`, `ui=`, `pad=held,x,y,frames`, `at=x,y,z,yaw,pitch`,
/// `near=soldier,metres`, `auto=`, `god=`, `hurt=`, `clock=`, `skill=`, `budget=`, `option=`.
///
/// # Safety
/// `text` points at `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn dt_control(text: *const u8, len: u32) {
    let (Some(c), Ok(text)) = (core(), core::str::from_utf8(core::slice::from_raw_parts(text, len as usize))) else { return };
    for word in text.split_ascii_whitespace() {
        let Some((key, value)) = word.split_once('=') else { continue };
        let mut numbers = value.split(',').map(|n| parse_f32(n).unwrap_or(0.0));
        let mut next = || numbers.next().unwrap_or(0.0);
        match key {
            "mode" => {
                if let Some(mode @ (Mode::Title | Mode::Play | Mode::Menu)) = Mode::parse(value) {
                    if mode == Mode::Play && c.session.mode == Mode::Title {
                        c.session.command(&mut c.game, Command::Start);
                    }
                    c.session.mode = mode;
                }
            }
            "ui" => channel().receive(match value {
                "start" => r#"{"type":"start"}"#,
                "menu" => r#"{"type":"menu","on":true}"#,
                "resume" => r#"{"type":"menu","on":false}"#,
                "restart" => r#"{"type":"restart"}"#,
                "title" => r#"{"type":"title"}"#,
                _ => continue,
            }),
            "pad" => c.held = (Pad { held: next() as u32, sx: next() * 0.01, sy: next() * 0.01 }, next() as u32),
            "at" => {
                let (x, y, z, yaw, pitch) = (next(), next(), next(), next(), next());
                c.game.player.place(v3(x, y, z), yaw * (PI / 180.0));
                c.game.player.pitch = pitch * (PI / 180.0);
                c.session.apply(&mut c.game);
            }
            "near" => {
                let (i, d) = (next() as usize, next());
                if let Some(s) = c.game.soldiers.get(i) {
                    let ahead = forward(s.yaw, 0.0);
                    let at = s.pos + ahead * if d > 0.0 { d } else { 4.0 };
                    c.game.player.place(at, yaw_of(ahead * -1.0));
                    c.game.player.pitch = -0.08;
                }
            }
            "auto" => c.auto = next() != 0.0,
            "god" => c.god = next() != 0.0,
            "hurt" => {
                let from = c.game.player.head() + forward(c.game.player.yaw, 0.0) * 8.0;
                c.game.player.hurt(next(), from, &mut c.game.events);
            }
            "clock" => c.game.clock = next(),
            "skill" => c.game.skill = next() * 0.01,
            "budget" => c.budget = next() as u32,
            "option" => c.option = next() as u32,
            _ => {}
        }
    }
}

/// One frame of the match: what the interface asked on its last turn is done, the pad is read,
/// `ticks` steps are taken, and the interface is shown the match as it now stands.
#[no_mangle]
pub extern "C" fn dt_step(input: &PadIn, ticks: u32) {
    let Some(c) = (unsafe { core() }) else { return };
    let now = |c: &Core| c.clock.map_or(0, |f| unsafe { f() });
    let from = now(c);
    if let Some(text) = c.session.obey(&mut c.game) {
        c.prefs_out = Some(text);
    }
    // The second stick of a New 3DS looks as the four buttons do, by how far it is pushed.
    let mut pad = Pad { held: input.held, sx: input.sx, sy: input.sy };
    for (push, bit) in [(input.rx, pad::EAST), (-input.rx, pad::WEST), (input.ry, pad::NORTH), (-input.ry, pad::SOUTH)] {
        if push > 0.35 {
            pad.held |= bit;
        }
    }
    if c.held.1 > 0 {
        c.held.1 -= 1;
        pad = c.held.0;
    }
    if c.auto && c.session.mode == Mode::Play {
        let (fire, reload) = hear::bench_aim(&mut c.game, ticks as f32 * DT);
        pad.held &= !(pad::R | pad::L | pad::LEFT);
        pad.held |= if reload { pad::LEFT } else { 0 } | if fire { pad::R } else { 0 };
    }
    {
        let (fx, count) = (&mut c.fx, &mut c.sounds_played);
        c.session.run(&mut c.game, &pad, ticks, |game, event| {
            let (eye, yaw, pitch, _, _) = game.player.eye();
            let ahead = forward(yaw, pitch);
            let right = ahead.cross(V3::UP).norm_or(v3(1.0, 0.0, 0.0));
            fx.on_event(event, eye, ahead, right, right.cross(ahead));
            hear::sound(game, event, count, |p| hear(Heard::Play(p)));
        });
    }
    if c.god {
        c.game.player.health = 100.0;
    }
    let playing = matches!(c.session.mode, Mode::Play | Mode::Down);
    if playing {
        c.fx.step(ticks as f32 * DT);
    }
    if playing != c.was_playing {
        // A stopped match is silent but for the street.
        if !playing {
            hear(Heard::Silence);
        }
        hear(Heard::Bed { id: Id::Ambience as u16, gain: if playing { 110 } else { 70 } });
    }
    c.was_playing = playing;
    c.orbit += ticks as f32 * DT;
    let state = unsafe { &mut channel().state };
    c.session.publish(&c.game, state);
    if !c.session.stats {
        state.stats.clear();
    } else if c.frame % 15 == 0 {
        let p = &c.perf;
        let (avg, fps) = ((p.frame * 10.0) as u32, (10_000.0 / max(p.frame, 1.0)) as u32);
        state.stats.clear();
        let _ = write!(state.stats, "{}.{} fps · {}.{} ms · late {} · {} draws · {}.{}k tris", fps / 10, fps % 10, avg / 10, avg % 10, p.late, p.draws, p.triangles / 1000, p.triangles % 1000 / 100);
    }
    c.frame = c.frame.wrapping_add(1);
    c.sim_us += (now(c).wrapping_sub(from) as f32 - c.sim_us) * 0.1;
}

impl Core {
    /// The eye of the frame: behind the title it stands where a match starts, a little above a
    /// head, and looks slowly up and down the street.
    fn eye(&self) -> (V3, f32, f32, f32, f32) {
        if matches!(self.session.mode, Mode::Title | Mode::Loading) {
            let s = &self.scene;
            (v3(s.start[0], s.start[1] + 2.3, s.start[2]), s.start[3] + sin(self.orbit * 0.11) * 0.42, 0.03 + sin(self.orbit * 0.07) * 0.03, 0.0, 1.0)
        } else {
            self.game.player.eye()
        }
    }
}

/// The frame's eye, light and switches.
#[no_mangle]
pub extern "C" fn dt_view(out: &mut ViewOut) {
    let Some(c) = (unsafe { core() }) else { return };
    let (eye, yaw, pitch, roll, zoom) = c.eye();
    let look = forward(yaw, pitch);
    let s = &c.scene;
    let colour = |abgr: u32| [(abgr & 255) as f32 / 255.0, (abgr >> 8 & 255) as f32 / 255.0, (abgr >> 16 & 255) as f32 / 255.0];
    let stood = !matches!(c.session.mode, Mode::Title | Mode::Loading);
    let flash = c.game.arms.flash;
    *out = ViewOut {
        eye: [eye.x, eye.y, eye.z],
        view: mat::view(V3::ZERO, look, roll),
        fov: FOV * zoom,
        haze: colour(s.haze_colour),
        haze_from: s.haze[0],
        haze_to: s.haze[1],
        sun_colour: s.sun_colour,
        sky_colour: s.sky_colour,
        stood: stood as u32,
        flash: if stood && flash > 0 { 0.035 + 0.012 * flash as f32 } else { 0.0 },
        flash_at: [0.0; 3],
        pace: 2,
        option: c.option,
        nearer: c.nearer,
    };
}

/// Chooses the frame's draws of the street, in the pack's order: one picture after another, the
/// glass last. Returns how many, and where they are until the next call.
#[no_mangle]
pub extern "C" fn dt_choose(draws: &mut *const DrawOut) -> u32 {
    let Some(c) = (unsafe { core() }) else { return 0 };
    let from = c.clock.map_or(0, |f| unsafe { f() });
    let (eye, yaw, pitch, roll, zoom) = c.eye();
    let vp = mat::mul(&mat::perspective(FOV * zoom, ASPECT, NEAR, FAR), &mat::view(eye, forward(yaw, pitch), roll));
    let planes = mat::unit_planes(&mat::planes(&vp));
    let counts = view::select(&c.world, &mut c.by_cell, &planes, eye, &Guard::open(), &view::Options { nearer: c.nearer, everything: c.option & 4 != 0 }, &mut c.draws);
    // Over the budget of triangles: the levels of detail come in. A soldier's triangle counts for one and a half.
    let over = (counts.once + c.figure_triangles * 3 / 2) as f32 / c.budget.max(1) as f32;
    c.nearer = clamp(c.nearer * if over > 1.0 { 1.0 / (1.0 + (over - 1.0) * 0.5) } else { 1.0 + (1.0 - over) * 0.08 }, 0.35, 1.0);
    c.triangles = counts.once;
    c.out.clear();
    for d in &c.draws {
        let chunk = &c.world.chunks[d.chunk as usize];
        let lod = chunk.lod(d.lod as usize);
        c.out.push(DrawOut {
            vertex: lod.vertex,
            index: lod.index + d.first,
            count: d.count,
            texture: chunk.texture,
            cell: chunk.cell,
            flags: chunk.flags,
            opacity: chunk.opacity,
            pad: 0,
            uv: [chunk.uv_scale[0] / 32768.0, chunk.uv_offset[0], chunk.uv_scale[1] / 32768.0, chunk.uv_offset[1]],
        });
    }
    *draws = c.out.as_ptr();
    c.select_us += (c.clock.map_or(0, |f| unsafe { f() }).wrapping_sub(from) as f32 - c.select_us) * 0.1;
    c.out.len() as u32
}

fn rows(p: &Pose) -> [f32; 12] {
    [p[0][0], p[0][1], p[0][2], p[0][3], p[1][0], p[1][1], p[1][2], p[1][3], p[2][0], p[2][1], p[2][2], p[2][3]]
}

/// The soldiers and the held weapon of the frame, posed. Returns how many, and where they are
/// until the next call. The held weapon is the last.
#[no_mangle]
pub extern "C" fn dt_figures(figures: &mut *const FigureOut, view: &mut ViewOut) -> u32 {
    let Some(c) = (unsafe { core() }) else { return 0 };
    c.posed.clear();
    c.figure_triangles = 0;
    let Some(posed) = c.figures.as_ref() else { return 0 };
    if matches!(c.session.mode, Mode::Title | Mode::Loading) {
        return 0;
    }
    let (eye, yaw, pitch, roll, _) = c.game.player.eye();
    let look = forward(yaw, pitch);
    let to_view: Mat4 = mat::view(V3::ZERO, look, roll);
    let turn = |v: V3| v3(to_view[0] * v.x + to_view[1] * v.y + to_view[2] * v.z, to_view[4] * v.x + to_view[5] * v.y + to_view[6] * v.z, to_view[8] * v.x + to_view[9] * v.y + to_view[10] * v.z);
    let sun = v3(c.scene.sun[0], c.scene.sun[1], c.scene.sun[2]);
    let add = |c_posed: &mut Vec<FigureOut>, triangles: &mut u32, id: u32, poses: &[Pose], place: [f32; 12], sun: V3, up: V3, lit: f32, held: Option<f32>| {
        let Some(model) = posed.model(id) else { return };
        let mut out = FigureOut { place, sun: [sun.x, sun.y, sun.z], up: [up.x, up.y, up.z], lit, held: held.is_some() as u32, fov: held.unwrap_or(0.0), batches: 0, batch: [BatchOut { bones: [[0.0; 12]; 4], vertices: core::ptr::null(), indices: core::ptr::null(), count: 0 }; BATCHES] };
        for b in model.batches.iter().take(BATCHES) {
            let slot = &mut out.batch[out.batches as usize];
            for k in 0..4 {
                slot.bones[k] = rows(&poses[(b.bones[k.min(b.count.max(1) as usize - 1)] as usize).min(poses.len() - 1)]);
            }
            // SAFETY: the offsets are inside the figures' section, which `parse` walked.
            slot.vertices = unsafe { c.figure_base.add(b.vertices) };
            slot.indices = unsafe { c.figure_base.add(b.indices) } as *const u16;
            slot.count = b.index_count;
            out.batches += 1;
            *triangles += b.index_count / 3;
        }
        c_posed.push(out);
    };
    for s in c.game.soldiers.iter().filter(|s| s.used) {
        let to = s.pos + v3(0.0, 0.9, 0.0) - eye;
        let distance = to.len();
        // Behind the eye, or too far to be more than a pixel.
        if distance > 110.0 || to.dot(look) < -1.5 {
            continue;
        }
        let mut poses = [pose::IDENTITY; bone::SOLDIER];
        posed.soldier(s, &mut poses);
        // The model faces +Z; a soldier's heading at yaw 0 is -Z.
        let (sn, cs) = (sin(s.yaw + PI), cos(s.yaw + PI));
        let at = s.pos - eye;
        let root: Mat4 = [cs, 0.0, sn, at.x, 0.0, 1.0, 0.0, at.y, -sn, 0.0, cs, at.z, 0.0, 0.0, 0.0, 1.0];
        let m = mat::mul(&to_view, &root);
        let place = [m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7], m[8], m[9], m[10], m[11]];
        // The sun in his own space: the root's turn taken back.
        let local = v3(cs * sun.x - sn * sun.z, sun.y, sn * sun.x + cs * sun.z);
        let lit = pose::sunlit(&c.scene, c.textures, c.texels, s.pos.x, s.pos.z);
        add(&mut c.posed, &mut c.figure_triangles, posed.soldier_model(s, distance), &poses, place, local, V3::UP, lit, None);
    }
    if let Some(held) = posed.held(&c.game) {
        let p = c.game.player.body.pos;
        let lit = pose::sunlit(&c.scene, c.textures, c.texels, p.x, p.z);
        // Its space is the eye's: the sun and the sky's up as the eye sees them.
        add(&mut c.posed, &mut c.figure_triangles, held.model, &held.poses, [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0], turn(sun), turn(V3::UP), lit, Some(held.fov));
        let at = held.muzzle + v3(0.0, 0.0, -0.06);
        view.flash_at = [at.x, at.y, at.z];
    } else {
        view.flash = 0.0;
    }
    *figures = c.posed.as_ptr();
    c.posed.len() as u32
}

/// The effects of the frame as squares facing the eye, their places counted from the eye: the
/// ones that cover what is behind them, then the ones that glow.
///
/// # Safety
/// `out` has room for `dt_sizes`'s last number of vertices.
#[no_mangle]
pub unsafe extern "C" fn dt_effects(out: *mut FxVertex, covering: &mut u32, glowing: &mut u32) {
    (*covering, *glowing) = (0, 0);
    let Some(c) = core() else { return };
    if c.option & 1024 != 0 || matches!(c.session.mode, Mode::Title | Mode::Loading) {
        return;
    }
    let (eye, yaw, pitch, _, _) = c.game.player.eye();
    let look = forward(yaw, pitch);
    let right = look.cross(V3::UP).norm_or(v3(1.0, 0.0, 0.0));
    let (a, b) = c.fx.build(eye, right, right.cross(look), |_| true, core::slice::from_raw_parts_mut(out, fx::VERTICES));
    (*covering, *glowing) = (a as u32, b as u32);
}

/// The sky: a band of pictures round the horizon and a fan to the zenith, as float vertices about
/// the eye. Returns the vertices written; `indices` takes `columns × 288` for the band and the
/// rest for the fan, whose count is written to `fan`.
///
/// # Safety
/// `vertices` has room for 1 024 and `indices` for 4 096.
#[no_mangle]
pub unsafe extern "C" fn dt_sky(vertices: *mut FxVertex, indices: *mut u16, fan: &mut u32) -> u32 {
    const SEGMENTS: usize = 8;
    const ROWS: usize = 6;
    let Some(c) = core() else { return 0 };
    let scene = &c.scene;
    let columns = scene.sky_columns.max(1) as usize;
    let around = columns * SEGMENTS;
    let (mut nv, mut ni) = (0usize, 0usize);
    let mut vertex = |v: FxVertex| {
        vertices.add(nv).write(v);
        nv += 1;
        nv as u16 - 1
    };
    let mut index = |list: &[u16]| {
        for i in list {
            indices.add(ni).write(*i);
            ni += 1;
        }
    };
    for col in 0..columns {
        let mut base = 0u16;
        for row in 0..=ROWS {
            let v = row as f32 / ROWS as f32;
            let elevation = scene.sky_top + (scene.sky_floor - scene.sky_top) * v;
            for k in 0..=SEGMENTS {
                let u = k as f32 / SEGMENTS as f32;
                let azimuth = (col as f32 + u) / columns as f32 * TAU;
                let d = v3(cos(elevation) * cos(azimuth), sin(elevation), cos(elevation) * sin(azimuth)) * 200.0;
                // (half a texel in from the picture's edges: its neighbour is another picture)
                let at = vertex(FxVertex { uv: [0.002 + u * 0.996, 0.004 + v * 0.992], colour: [255, 255, 255, 255], pos: [d.x, d.y, d.z] });
                if row == 0 && k == 0 {
                    base = at;
                }
            }
        }
        let stride = SEGMENTS as u16 + 1;
        for row in 0..ROWS as u16 {
            for k in 0..SEGMENTS as u16 {
                let (a, b, c2, d) = (base + row * stride + k, base + row * stride + k + 1, base + (row + 1) * stride + k, base + (row + 1) * stride + k + 1);
                index(&[a, c2, b, b, c2, d]);
            }
        }
    }
    let z = scene.zenith;
    let zenith = [z as u8, (z >> 8) as u8, (z >> 16) as u8, 255];
    let mut cap = 0u16;
    for k in 0..around {
        let azimuth = k as f32 / around as f32 * TAU;
        let d = v3(cos(scene.sky_top) * cos(azimuth), sin(scene.sky_top), cos(scene.sky_top) * sin(azimuth)) * 200.0;
        let at = vertex(FxVertex { uv: [0.0, 0.0], colour: zenith, pos: [d.x, d.y, d.z] });
        if k == 0 {
            cap = at;
        }
    }
    let top = vertex(FxVertex { uv: [0.0, 0.0], colour: zenith, pos: [0.0, 200.0, 0.0] });
    for k in 0..around as u16 {
        index(&[cap + k, cap + (k + 1) % around as u16, top]);
    }
    *fan = around as u32 * 3;
    nv as u32
}

/// The next `frames` of sound, left and right in turn, at 22 050 a second. The host's sound
/// thread calls this and nothing else of the library.
///
/// # Safety
/// `out` has room for `frames × 2` samples; one thread calls it.
#[no_mangle]
pub unsafe extern "C" fn dt_mix(out: *mut i16, frames: u32) {
    let block = core::slice::from_raw_parts_mut(out, frames as usize * 2);
    block.fill(0);
    let (sounds, samples) = *core::ptr::addr_of!(SOUNDS);
    let mixer = &mut *core::ptr::addr_of_mut!(MIXER);
    let bed = &mut *core::ptr::addr_of_mut!(BED_VOICE);
    let written = WRITTEN.load(Ordering::Acquire);
    let mut taken = TAKEN.load(Ordering::Relaxed);
    while taken != written {
        let what = (core::ptr::addr_of!(HEARD) as *const Heard).add(taken as usize % QUEUE).read();
        taken = taken.wrapping_add(1);
        match what {
            Heard::Play(p) => {
                mixer.play(sounds, p.id as u16, p.left, p.right, p.pitch, p.priority.min(BED - 1));
            }
            Heard::Bed { id, gain } => {
                let held = bed.filter(|&(slot, sound)| mixer.voice(slot).is_some_and(|v| v.sound == sound));
                match held {
                    Some((slot, sound)) if sound == id && gain > 0 => mixer.set_gain(slot, gain, gain),
                    _ => {
                        if let Some((slot, _)) = held {
                            mixer.stop(slot);
                        }
                        *bed = if gain > 0 { mixer.play(sounds, id, gain, gain, UNITY, BED).map(|slot| (slot, id)) } else { None };
                    }
                }
            }
            Heard::Silence => {
                mixer.stop_all();
                *bed = None;
            }
        }
    }
    TAKEN.store(taken, Ordering::Release);
    if !sounds.is_empty() {
        for part in block.chunks_mut(mix::BLOCK * 2) {
            mixer.render(sounds, samples, part);
        }
    }
}

/// What the frame took, for the statistics line and the status.
#[no_mangle]
pub extern "C" fn dt_report(perf: &Perf) {
    if let Some(c) = unsafe { core() } {
        c.perf = *perf;
    }
}

/// The status record as JSON, with `extra` (members without braces) before its end.
///
/// # Safety
/// `out` has room for `cap` bytes; `extra` points at `extra_len`.
#[no_mangle]
pub unsafe extern "C" fn dt_status(out: *mut c_char, cap: u32, extra: *const u8, extra_len: u32) -> u32 {
    let Some(c) = core() else { return 0 };
    let mut s = String::with_capacity(1024);
    let (eye, yaw, pitch, _, _) = c.eye();
    let (p, g) = (&c.perf, &c.game);
    let _ = write!(
        s,
        "{{\"target\":\"3ds\",\"stage\":\"running\",\"frames\":{},\"frameMs\":{:.2},\"worstMs\":{:.2},\"late\":{},\"mode\":\"{}\",\"draws\":{},\"tris\":{},\"worldTris\":{},\"figureTris\":{},\"nearer\":{:.2},\"budget\":{},\"cpuMs\":{:.2},\"gpuMs\":{:.2},\"simMs\":{:.2},\"selectMs\":{:.2},\"eye\":[{:.2},{:.2},{:.2}],\"yaw\":{:.1},\"pitch\":{:.1},\"match\":{{\"health\":{:.0},\"us\":{},\"them\":{},\"clock\":{:.0},\"standing\":{},\"kills\":{},\"deaths\":{},\"shots\":{},\"hits\":{},\"weapon\":{},\"magazine\":{}}},",
        p.frames,
        p.frame,
        p.worst,
        p.late,
        c.session.mode.name(),
        p.draws,
        p.triangles,
        c.triangles,
        c.figure_triangles,
        c.nearer,
        c.budget,
        p.cpu,
        p.gpu,
        c.sim_us / 1000.0,
        c.select_us / 1000.0,
        eye.x,
        eye.y,
        eye.z,
        yaw * (180.0 / PI),
        pitch * (180.0 / PI),
        g.player.health,
        g.score[0],
        g.score[1],
        g.clock,
        g.standing(),
        g.tally.kills,
        g.tally.deaths,
        g.tally.shots,
        g.tally.hits,
        g.arms.active,
        g.arms.magazine[g.arms.active],
    );
    s.push_str(core::str::from_utf8(core::slice::from_raw_parts(extra, extra_len as usize)).unwrap_or("\"extra\":null"));
    s.push('}');
    let n = s.len().min(cap as usize - 1);
    core::ptr::copy_nonoverlapping(s.as_ptr(), out as *mut u8, n);
    *out.add(n) = 0;
    n as u32
}
