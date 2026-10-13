//! Pocket Duty on PS Vita: the street through GXM at 960 × 544 with four
//! samples a pixel, 60 frames a second, drawn straight into the display's
//! surface.
//!
//! The match is `duty_sim::game::Game` and the flow around it is
//! `duty_interface::Session`, as on every device. The interface is the
//! PocketJS guest of `ui/` on PocketJS's Vita host library (`interface.rs`).
//! A map's pack is read through `pocket3d-packs` (`store.rs`): its pieces lie
//! in the package, or on the memory card where a development build keeps
//! what it fetched over the cable. Another map is read in place: this map's
//! memory goes back first.
//!
//! A frame is the browser renderer's (`wgpu/src/render.rs`) on this console's
//! GPU (`world.rs`, `figures.rs`); what the Vita's pack holds more than the
//! PSP's is `profiles/vita60.json`.
//!
//! A development build runs in Pocket Devkit's native slot and is steered
//! over the cable: `host0:duty/boot.txt` holds words for the start,
//! `host0:duty/control.txt` words for the frame that reads them (`remote`).

// (the status is one `json!` of a few dozen fields)
#![recursion_limit = "256"]

mod figures;
mod gpu;
mod hostfs;
mod interface;
mod paths;
mod store;
mod world;

use std::sync::mpsc;
use std::time::{Duration, Instant};

use duty_interface::maps::{self, Map};
use duty_interface::{channel, pad, parse_f32, Chart, Command, Mode, Pad, Session};
use duty_pack::sound::{Id, Sound};
use duty_pack::{self as pack, Chunk, Cluster, Scene, ViewCell};
use duty_sim::bvh::{Bvh, Node, Tri};
use duty_sim::clip::Guard;
use duty_sim::fx::{self, Fx, FxVertex};
use duty_sim::game::Game;
use duty_sim::hear;
use duty_sim::level::Level;
use duty_sim::mat;
use duty_sim::math::*;
use duty_sim::mix::{self, Mixer, UNITY};
use duty_sim::tune::{WeaponDef, DT};
use duty_sim::view::{self, Draw};
use duty_sim::weapon::{self, WEAPONS};
use figures::{Figures, Skins};
use gpu::Gpu;
use pocket_vita_gxm::mem::{Arena, Kind, Ring};
use pocket_vita_gxm::target::Fence;
use pocketjs_vita::{audio, dev, dev_protocol::Op, devmenu::Action, graphics, input};
use serde_json::{json, Value};
use store::Packs;
use vita2d_sys as g;
use world::{Drawn, Gfx, Look, Programs, View, HEIGHT, WIDTH};

#[no_mangle]
#[used]
pub static sceUserMainThreadStackSize: u32 = 1024 * 1024;

/// The heap: the guest, the match's own sections of a pack (the collision world, the walk grid, the sight
/// lists, the sounds) and a picture on its way to video memory. The street's vertices and indices are in
/// memory the GPU reads, outside it.
#[no_mangle]
#[used]
pub static _newlib_heap_size_user: u32 = 72 * 1024 * 1024;

extern "C" {
    fn scePowerSetArmClockFrequency(freq: i32) -> i32;
    fn scePowerSetBusClockFrequency(freq: i32) -> i32;
    fn scePowerSetGpuClockFrequency(freq: i32) -> i32;
    fn scePowerSetGpuXbarClockFrequency(freq: i32) -> i32;
    fn scePowerGetArmClockFrequency() -> i32;
    fn scePowerGetGpuClockFrequency() -> i32;
    fn sceDisplayGetVcount() -> i32;
    fn sceDisplayWaitVblankStart() -> i32;
    fn sceKernelPowerTick(kind: i32) -> i32;
}

/// Bytes of the parameter buffer: where GXM keeps a scene's transformed vertices until its tiles are drawn.
/// When a scene fills it, GXM draws what it has and starts again, at several times the cost. vita2d asks for
/// 16 MB, which Pocket Tokyo's vertices filled near 150 000 triangles in view; a view along the street at its
/// nearest level holds 120 000 and the figures.
const PARAMETER_BUFFER: u32 = 32 * 1024 * 1024;

extern "C" {
    fn __real_sceGxmInitialize(params: *const g::SceGxmInitializeParams) -> i32;
}

/// vita2d's start of GXM, with this game's parameter buffer (see `build.rs`).
///
/// # Safety
/// Called by vita2d in place of `sceGxmInitialize`, with its parameters.
#[no_mangle]
pub unsafe extern "C" fn __wrap_sceGxmInitialize(params: *const g::SceGxmInitializeParams) -> i32 {
    let mut p = *params;
    p.parameterBufferSize = PARAMETER_BUFFER;
    __real_sceGxmInitialize(&p)
}

/// Degrees the view takes top to bottom at its widest, and its near and far planes, as on every device.
const FOV: f32 = 65.0;
const NEAR: f32 = 0.15;
const FAR: f32 = 420.0;
/// Samples a pixel of the display's surface.
const DEFAULT_MSAA: u32 = 4;
/// vita2d's pool of temporary vertices, which the interface and the Devkit menu draw from. A frame takes half
/// of it, in turn (see the display scene).
const POOL_BYTES: u32 = 2 * 1024 * 1024;
/// ARM, bus, GPU and GPU crossbar clocks (MHz) the frame budget assumes.
const CLOCKS: [i32; 4] = [444, 222, 222, 166];
/// Sounds heard at once, as on the PSP, and the bed's priority: no other sound takes the bed's voice.
const VOICES: usize = 16;
const BED: u8 = u8::MAX;
/// Frames of sound (at the mixer's rate) kept ahead of the output: three of its blocks, 70 ms.
const AHEAD: usize = 3 * mix::BLOCK;
/// Triangles of the street a frame may draw before the levels of detail come in.
const BUDGET: u32 = 170_000;
/// Seconds a frame may wait for the GPU to finish the frame before last, before the levels come in.
const BEHIND: f32 = 0.006;

unsafe fn set_clocks() {
    scePowerSetArmClockFrequency(CLOCKS[0]);
    scePowerSetBusClockFrequency(CLOCKS[1]);
    scePowerSetGpuClockFrequency(CLOCKS[2]);
    scePowerSetGpuXbarClockFrequency(CLOCKS[3]);
}

// The Vita's controller bits, which are PocketJS's.
const P_SELECT: u32 = 0x1;
const P_START: u32 = 0x8;
const P_UP: u32 = 0x10;
const P_RIGHT: u32 = 0x20;
const P_DOWN: u32 = 0x40;
const P_LEFT: u32 = 0x80;
const P_L: u32 = 0x100 | 0x400;
const P_R: u32 = 0x200 | 0x800;
const P_TRIANGLE: u32 = 0x1000;
const P_CIRCLE: u32 = 0x2000;
const P_CROSS: u32 = 0x4000;
const P_SQUARE: u32 = 0x8000;

unsafe fn text(font: *mut g::vita2d_pgf, x: i32, y: i32, color: u32, scale: f32, s: &str) {
    let c = std::ffi::CString::new(s.replace('\0', " ")).unwrap();
    g::vita2d_pgf_draw_text(font, x, y, color, scale, c.as_ptr());
}

/// Display refreshes from the count `then` to the count `now`. The display's counter has 16 bits: it starts
/// again at 0 after 65 536 refreshes (18 minutes), and a plain difference across that point is -65 535, which
/// a loop that waits for one refresh takes 18 more minutes to see pass.
fn refreshes(now: i32, then: i32) -> i32 {
    now.wrapping_sub(then) & 0xffff
}

/// What a frame needs before there is a street to draw: the interface, the wired-debug host and a count of
/// frames shown.
struct Shell {
    ui: interface::Ui,
    dev: dev::Host,
    /// The system font, loaded when a frame has no interface to draw.
    font: *mut g::vita2d_pgf,
    frame: u32,
    /// The status goes to the data folder too (`report=1`): where a run with no cable is read from.
    report: bool,
}

impl Shell {
    /// A frame of the interface alone, while a pack is read (`Mode::Loading`, `message` the step) or after
    /// the start failed (`Mode::Error`, `message` the reason): the guest turns, then draws. It also publishes
    /// status, so the computer sees the new process come up.
    unsafe fn frame(&mut self, mode: Mode, message: &str) {
        let state = &mut channel().state;
        state.mode = mode;
        if state.message != message {
            state.message.clear();
            state.message.push_str(message);
        }
        self.ui.turn(interface::TURN, &interface::NEUTRAL, true, None);
        if !self.ui.up() && self.font.is_null() {
            self.font = g::vita2d_load_default_pgf();
        }
        // (the ground of the Pocket3D title card)
        graphics::begin_frame(0xff26_1217);
        if self.ui.up() {
            self.ui.draw();
        } else {
            // No interface: the system font says what is going on.
            text(self.font, 48, 80, 0xffff_ffff, 1.4, "Pocket Duty");
            let chars: Vec<char> = message.chars().collect();
            for (i, line) in chars.chunks(90).take(4).enumerate() {
                text(self.font, 48, 130 + i as i32 * 28, 0xffd0_d0d0, 1.0, &line.iter().collect::<String>());
            }
        }
        self.dev.overlay();
        graphics::present();
        self.dev.engine = json!({"target": "vita", "stage": if mode == Mode::Error { "error" } else { "loading" }, "message": message, "interface": {"up": self.ui.up(), "open": channel().is_open(), "error": self.ui.error}});
        self.dev.publish(self.frame, "duty");
        if self.report && self.frame % 15 == 0 {
            paths::write_text("status.json", &self.dev.engine.to_string());
        }
        serve(&mut self.dev, self.frame, Action::None);
        self.frame += 1;
    }

    /// The start failed: the reason stays on the screen and in the status receipt.
    unsafe fn fail(&mut self, error: String) -> ! {
        pocketjs_vita::vita_log(format_args!("duty: {error}"));
        loop {
            self.frame(Mode::Error, &error);
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

/// Words from the computer: `host0:duty/control.txt`, polled off the render thread. The tool ends the file
/// with a line that differs each time, so the same words can be said twice.
fn control_watcher() -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel();
    let _ = std::thread::Builder::new().name("duty-control".into()).stack_size(256 * 1024).spawn(move || {
        // What is there at launch is left over from an earlier run: only changes after it count.
        let path = format!("{}/control.txt", paths::HOST);
        let mut last = hostfs::read(&path, 64 * 1024).unwrap_or_default();
        loop {
            std::thread::sleep(Duration::from_millis(250));
            if let Some(bytes) = hostfs::read(&path, 64 * 1024) {
                if bytes != last {
                    last = bytes.clone();
                    if tx.send(String::from_utf8_lossy(&bytes).into_owned()).is_err() {
                        return;
                    }
                }
            }
        }
    });
    rx
}

/// The pad as the flow reads it: the buttons, and the left stick. The right stick is the flow's second one.
fn session_pad(p: &input::Pad) -> (Pad, (f32, f32)) {
    let mut held = 0;
    for (bit, to) in [
        (P_UP, pad::UP),
        (P_DOWN, pad::DOWN),
        (P_LEFT, pad::LEFT),
        (P_RIGHT, pad::RIGHT),
        (P_TRIANGLE, pad::NORTH),
        (P_CIRCLE, pad::EAST),
        (P_CROSS, pad::SOUTH),
        (P_SQUARE, pad::WEST),
        (P_L, pad::L),
        (P_R, pad::R),
        (P_START, pad::START),
        (P_SELECT, pad::SELECT),
    ] {
        if p.buttons & bit != 0 {
            held |= to;
        }
    }
    // The sticks rest off centre by up to a fifth of their travel: nothing inside that counts, and the rest
    // is rescaled.
    let axis = |v: u8| {
        let x = (v as f32 - 127.5) / 127.5;
        const DEAD: f32 = 0.24;
        if abs(x) < DEAD {
            0.0
        } else {
            (x - DEAD * if x < 0.0 { -1.0 } else { 1.0 }) / (1.0 - DEAD)
        }
    };
    (Pad { held, sx: axis(p.lx), sy: -axis(p.ly) }, (axis(p.rx), -axis(p.ry)))
}

/// Rolling frame statistics over the last `N` frames.
struct Timing {
    ms: [f32; Timing::N],
    at: usize,
    late: u32,
    /// Of the late frames, how many this thread's work made late, and how many the GPU's.
    late_cpu: u32,
    late_gpu: u32,
    frames: u32,
}

impl Timing {
    const N: usize = 240;
    fn push(&mut self, ms: f32, late: bool) {
        self.ms[self.at] = ms;
        self.at = (self.at + 1) % Self::N;
        self.frames += 1;
        if late {
            self.late += 1;
        }
    }
    fn avg(&self) -> f32 {
        self.ms.iter().sum::<f32>() / Self::N as f32
    }
    fn worst(&self) -> f32 {
        self.ms.iter().fold(0.0, |a, &b| a.max(b))
    }
    fn clear(&mut self) {
        *self = Timing::new();
    }
    fn new() -> Timing {
        Timing { ms: [16.7; Timing::N], at: 0, late: 0, late_cpu: 0, late_gpu: 0, frames: 0 }
    }
}

/// The sounds that play: the mixer every device has (`duty_sim::mix`), rendered on this thread a block at a
/// time into PocketJS's output ring, which a thread of its own hands to the console.
struct Sounding {
    mixer: Mixer<VOICES>,
    /// The voice the bed plays on, and its sound.
    bed: Option<(usize, u16)>,
    /// Sounds started so far: every other one of a kind is its second take.
    played: u32,
    /// The output is open.
    on: bool,
}

impl Sounding {
    /// Starts the looping bed `id` at `gain` (0…256), or sets the gain of the bed when it plays `id` already.
    fn ambience(&mut self, sounds: &[Sound], id: Id, gain: u16) {
        let id = id as u16;
        let held = self.bed.filter(|&(slot, sound)| self.mixer.voice(slot).is_some_and(|v| v.sound == sound));
        match held {
            Some((slot, sound)) if sound == id && gain > 0 => self.mixer.set_gain(slot, gain, gain),
            _ => {
                if let Some((slot, _)) = held {
                    self.mixer.stop(slot);
                }
                self.bed = if gain > 0 { self.mixer.play(sounds, id, gain, gain, UNITY, BED).map(|slot| (slot, id)) } else { None };
            }
        }
    }

    fn silence(&mut self) {
        self.mixer.stop_all();
        self.bed = None;
    }

    /// Mixes blocks until the output has `AHEAD` frames waiting.
    unsafe fn pump(&mut self, sounds: &[Sound], samples: &[u8]) {
        if !self.on {
            return;
        }
        // (the ring's whole room, less what is free of it)
        const RING: usize = 32 * 1024;
        while RING - audio::free_frames().min(RING) < AHEAD {
            let mut block = [0i16; mix::BLOCK * 2];
            self.mixer.render(sounds, samples, &mut block);
            audio::push(&block, 2);
        }
    }
}

/// The eye of a frame.
#[derive(Clone, Copy)]
struct Camera {
    eye: V3,
    yaw: f32,
    pitch: f32,
    roll: f32,
    /// Degrees, top to bottom.
    fov: f32,
}

/// What a development host sets beside the flow.
struct Settings {
    look: Look,
    /// Moves the levels' distances (1 is the pack's own), and whether every chunk the view holds is drawn.
    nearer: f32,
    everything: bool,
    /// A fixed eye: where it is, its yaw and pitch in radians, degrees of view.
    view: Option<(V3, f32, f32, f32)>,
    /// The player's aim and trigger are a bench's: at the nearest soldier, whenever one is in sight.
    auto: bool,
    /// Nothing hurts the player.
    god: bool,
    /// A pad to use instead of the console's, and for how many more frames.
    pad: (Pad, u32),
    /// Display refreshes a frame: 1 is sixty frames a second.
    pace: i32,
    /// Wait for the GPU after each frame and time it.
    profile: bool,
    /// Draw the interface (off for a measurement of what it costs).
    interface: bool,
    /// Triangles of the street a frame may draw, and whether the levels' distances follow it.
    budget: u32,
    govern: bool,
}

/// One map: its pack on the GPU and in memory, and the match on it.
struct World {
    map: &'static Map,
    gfx: Gfx,
    figures: Option<Figures>,
    /// The pictures' video memory.
    vram: Arena,
    scene: Scene,
    chunks: Vec<Chunk>,
    clusters: Vec<Cluster>,
    larges_at: Vec<u32>,
    cells: Vec<ViewCell>,
    lists: Vec<u16>,
    bits: Vec<u8>,
    by_cell: view::Cells,
    game: Game<'static>,
    fx: Fx,
    sounds: Vec<Sound>,
    samples: Vec<u8>,
    meta: String,
    bytes: usize,
    fetched: usize,
    load_ms: u64,
    /// The map's picture from above, in the interface's core.
    chart: Option<Chart>,
    /// The collision world and the walk grid the match borrows. Last: they go after the match.
    _solid: (Vec<Node>, Vec<Tri>, Vec<[f32; 3]>),
    _nav: Vec<u32>,
}

impl World {
    /// Reads a map's pack: the street to the GPU, the match's sections to memory. Each step is a frame of
    /// the loading screen.
    unsafe fn read(packs: &Packs, map: &'static Map, linear: bool, shell: &mut Shell) -> Result<World, String> {
        let start = Instant::now();
        let mut file = packs.pack(map.id)?;
        let mut stage = |_step: &str, file: &store::PackFile| {
            let (total, read, fetched) = file.progress();
            let over = if fetched > 0 { format!(", {:.1} MB over the cable", fetched as f32 / 1e6) } else { String::new() };
            shell.frame(Mode::Loading, &format!("Reading {} · {:.1} of {:.1} MB{over}", map.title, read as f32 / 1e6, total as f32 / 1e6));
        };
        stage("", &file);
        let scene: Scene = *file.records::<Scene>(pack::SCEN)?.first().ok_or("the pack's scene section is empty")?;
        let chunks: Vec<Chunk> = file.records(pack::CHNK)?;
        let clusters: Vec<Cluster> = file.records(pack::CLUS)?;
        // A pack cooked without the records of its large triangles (a device whose rasterizer cuts every
        // triangle at the view's sides, as this one's does) lists none.
        let larges_at: Vec<u32> = if file.has(pack::LTRO) { file.records(pack::LTRO)? } else { vec![0; clusters.len()] };
        if larges_at.len() != clusters.len() {
            return Err("the pack's chunks and clusters disagree: cook the pack again".into());
        }
        // Video memory in blocks of 4 MiB, so that the last block leaves at most that much unused.
        let mut vram = Arena::new(Kind::Cdram, 4 * 1024 * 1024);
        let gfx = Gfx::load(&mut file, &mut vram, scene, linear, &mut stage)?;
        stage("", &file);
        let solid: (Vec<Node>, Vec<Tri>, Vec<[f32; 3]>) = (file.records(pack::COLN)?, file.records(pack::COLT)?, file.records(pack::COLV)?);
        let nav: Vec<u32> = file.records(pack::NAVG)?;
        // SAFETY: the vectors are held by the world for as long as the match, and nothing writes them.
        let bvh = Bvh { nodes: &*(solid.0.as_slice() as *const [Node]), tris: &*(solid.1.as_slice() as *const [Tri]), verts: &*(solid.2.as_slice() as *const [[f32; 3]]) };
        let level: Level<'static> = Level::parse_nav(bvh, core::slice::from_raw_parts(nav.as_ptr() as *const u8, nav.len() * 4)).ok_or("the pack's walk grid")?;
        let weapons: Vec<WeaponDef> = if file.has(pack::WEAP) { file.records(pack::WEAP)? } else { Vec::new() };
        let defs: [WeaponDef; WEAPONS] = if weapons.len() == WEAPONS { [weapons[0], weapons[1], weapons[2]] } else { weapon::reference() };
        let figures = Figures::load(&mut file)?;
        stage("", &file);
        let sounds: Vec<Sound> = if file.has(pack::SNDS) { file.records(pack::SNDS)? } else { Vec::new() };
        let samples: Vec<u8> = if file.has(pack::SNDD) { file.read(pack::SNDD)? } else { Vec::new() };
        let meta = if file.has(pack::META) { String::from_utf8_lossy(&file.read(pack::META)?).trim_end_matches('\0').to_string() } else { String::new() };
        let (cells, lists, bits) = (file.records(pack::VCEL)?, file.records(pack::VLST)?, file.records(pack::VBIT)?);
        stage("", &file);
        let (bytes, _, fetched) = file.progress();
        drop(file);
        // The map from above travels beside its pack: it goes into the interface's core, and the interface is
        // told where.
        let chart = packs.whole(&format!("{}-map", map.id)).and_then(|bytes| {
            let (mut chart, entry) = Chart::parse(&bytes)?;
            chart.handle = shell.ui.picture(entry);
            (chart.handle >= 0).then_some(chart)
        });
        let by_cell = view::Cells::new(&scene, &chunks);
        Ok(World {
            map,
            gfx,
            figures,
            vram,
            scene,
            chunks,
            clusters,
            larges_at,
            cells,
            lists,
            bits,
            by_cell,
            game: Game::new(level, defs, 0x5eed_1234),
            fx: Fx::new(),
            sounds: if sounds.len() == Id::ALL.len() { sounds } else { Vec::new() },
            samples,
            meta,
            bytes,
            fetched,
            load_ms: start.elapsed().as_millis() as u64,
            chart,
            _solid: solid,
            _nav: nav,
        })
    }

    /// Gives the map's memory back. The GPU has drawn the last frame that read it.
    unsafe fn leave(self, shell: &mut Shell) {
        g::vita2d_wait_rendering_done();
        g::sceGxmFinish(g::vita2d_get_context());
        if let Some(chart) = &self.chart {
            shell.ui.forget(chart.handle);
        }
        let World { gfx, figures, vram, .. } = self;
        gfx.free();
        if let Some(figures) = figures {
            figures.free();
        }
        vram.free();
    }
}

/// Words for the flow and the renderer, as a development host sends them to every device of the game
/// (`remote` in `psp/src/main.rs`, `control` in `wgpu/src/app.rs`): `mode=title|play|menu` sets the flow
/// outright, `ui=start|menu|resume|restart|title` asks what the interface's lists ask, `map=<id>` asks for a
/// match on a map, `press=<mask>` presses buttons on the interface (PocketJS's bits) and `tap=<x>,<y>` taps
/// its panel (480 × 272), `pad=<held>,<x>,<y>,<frames>` holds the player's pad (the flow's own bits, the stick
/// in hundredths), `at=<x>,<y>,<z>,<yaw>,<pitch>` puts the player somewhere (degrees), `near=<soldier>,
/// <metres>` puts him in front of a soldier, `auto=1` hands his aim and trigger to the bench, `god=1` keeps
/// him whole, `hurt=<points>` hits him from ahead, `clock=<seconds>` sets what is left of the match,
/// `skill=<hundredths>` sets how hard the soldiers hit, `stats=1` asks for the statistics line.
///
/// And this renderer's own: `view=<x>,<y>,<z>,<yaw>,<pitch>,<fov>` or `view=off` (a held eye, degrees),
/// `nearer=<hundredths>` (the levels' distances), `everything=1`, `budget=<triangles>`, `govern=0|1`,
/// `pace=<refreshes a frame>`, `profile=1` (wait for the GPU each frame and time it), `interface=0`, `merge=
/// <indices>`, `cull=0|1|2`, and `sky= haze= shadows= effects= figures= shades= flash=` (0 or 1): each a part
/// of the frame that can be left out to learn what it costs. `walls=1` has every fragment of the street read
/// the ground's shadow picture, as the ground's do: what a second picture's read costs where walls fill the
/// screen (the picture it gives is wrong: the ground's shadow lies on the walls).
unsafe fn remote(words: &str, set: &mut Settings, session: &mut Session, ui: &mut interface::Ui, mut world: Option<&mut World>) {
    for word in words.split_whitespace() {
        let Some((key, value)) = word.split_once('=') else { continue };
        let mut numbers = value.split(',').map(|n| parse_f32(n).unwrap_or(0.0));
        let mut next = || numbers.next().unwrap_or(0.0);
        let on = value != "0";
        match key {
            "sky" => set.look.sky = on,
            "haze" => set.look.haze = on,
            "shadows" => set.look.shadows = on,
            "effects" => set.look.effects = on,
            "figures" => set.look.figures = on,
            "shades" => set.look.shades = on,
            "flash" => set.look.flash = on,
            "walls" => set.look.walls = on,
            "sun" => set.look.sun = on,
            "merge" => set.look.merge = next() as u32,
            "cull" => set.look.cull = next() as u8,
            "everything" => set.everything = on,
            "nearer" => set.nearer = clamp(next() * 0.01, 0.1, 4.0),
            "budget" => set.budget = (next() as u32).max(10_000),
            "govern" => set.govern = on,
            "pace" => set.pace = (next() as i32).clamp(1, 4),
            "profile" => set.profile = on,
            "interface" => set.interface = on,
            "stats" => session.stats = on,
            "view" => {
                let v: Vec<f32> = value.split(',').filter_map(parse_f32).collect();
                set.view = (v.len() >= 5).then(|| (v3(v[0], v[1], v[2]), v[3] * (PI / 180.0), v[4] * (PI / 180.0), v.get(5).copied().unwrap_or(FOV)));
            }
            "auto" => set.auto = on,
            "god" => set.god = on,
            "pad" => {
                let pad = Pad { held: next() as u32, sx: next() * 0.01, sy: next() * 0.01 };
                set.pad = (pad, next() as u32);
            }
            "press" => ui.press(next() as u32),
            "tap" => {
                let (x, y) = (next(), next());
                ui.tap(x as u16, y as u16);
            }
            "ui" => channel().receive(match value {
                "start" => r#"{"type":"start"}"#,
                "menu" => r#"{"type":"menu","on":true}"#,
                "resume" => r#"{"type":"menu","on":false}"#,
                "restart" => r#"{"type":"restart"}"#,
                "title" => r#"{"type":"title"}"#,
                _ => continue,
            }),
            // (what a row of the interface's map list asks)
            "map" => {
                if let Some(map) = maps::named(value) {
                    channel().receive(&format!(r#"{{"type":"map","id":"{}"}}"#, map.id));
                }
            }
            _ => {}
        }
        let Some(w) = world.as_deref_mut() else { continue };
        let game = &mut w.game;
        match key {
            "mode" => {
                if let Some(mode @ (Mode::Title | Mode::Play | Mode::Menu)) = Mode::parse(value) {
                    if mode == Mode::Play && session.mode == Mode::Title {
                        session.command(game, Command::Start);
                    }
                    session.mode = mode;
                }
            }
            "at" => {
                let (x, y, z, yaw, pitch) = (next(), next(), next(), next(), next());
                game.player.place(v3(x, y, z), yaw * (PI / 180.0));
                game.player.pitch = pitch * (PI / 180.0);
                session.apply(game);
            }
            "clock" => game.clock = next(),
            "hurt" => {
                let from = game.player.head() + forward(game.player.yaw, 0.0) * 8.0;
                game.player.hurt(next(), from, &mut game.events);
            }
            "near" => {
                // In front of a soldier, a few metres off, looking at him.
                let (i, d) = (next() as usize, next());
                if let Some(s) = game.soldiers.get(i) {
                    let ahead = forward(s.yaw, 0.0);
                    let at = s.pos + ahead * if d > 0.0 { d } else { 4.0 };
                    game.player.place(at, yaw_of(ahead * -1.0));
                    game.player.pitch = -0.08;
                }
            }
            "skill" => game.skill = next() * 0.01,
            _ => {}
        }
    }
}

/// A map has been read: it stands behind the title, or a match starts on it.
unsafe fn enter(world: &mut World, session: &mut Session, sounding: &mut Sounding, packs: &Packs, play: bool) {
    let scene = world.scene;
    // The map picture's corner, as the compiler laid it out about the level's origin.
    session.map = (scene.level_origin[0] - 80.0, scene.level_origin[2] - 80.0);
    session.map_id = world.map.id;
    session.maps = packs.maps().iter().map(|map| map.id).collect();
    session.chart = world.chart.clone();
    session.apply(&mut world.game);
    sounding.silence();
    sounding.ambience(&world.sounds, Id::Ambience, 70);
    channel().state.message.clear();
    session.mode = Mode::Title;
    if play {
        session.play(&mut world.game);
    }
}

fn main() {
    unsafe {
        // Words for the start: a development build's from the USB share (`host0:duty/boot.txt`), a package's from
        // its data folder when a test run put a `boot.txt` there. The control words, and `title=0` (no title
        // card), `msaa=0|2|4`, `linear=1` (pictures as rows in video memory), `map=<id>` (the map read first),
        // `report=1` (the status goes to `status.json` in the data folder as well).
        let live = cfg!(feature = "usb-debug");
        let boot: String = hostfs::read(&format!("{}/boot.txt", if live { paths::HOST } else { paths::DATA }), 8192).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
        let word = |key: &str| boot.split_whitespace().find_map(|w| w.strip_prefix(key).and_then(|w| w.strip_prefix('=')));
        let samples = word("msaa").and_then(|v| v.parse().ok()).unwrap_or(DEFAULT_MSAA);
        // The Pocket3D title card plays before the renderer starts. A development build skips it with `title=0`.
        if word("title") != Some("0") {
            pocket3d_title::vita::play();
        }
        // The display's own surface is multisampled: the scene is drawn straight into it. (The host's own start
        // of vita2d finds it started.)
        let msaa = match samples {
            4 => g::SceGxmMultisampleMode_SCE_GXM_MULTISAMPLE_4X,
            2 => g::SceGxmMultisampleMode_SCE_GXM_MULTISAMPLE_2X,
            _ => g::SceGxmMultisampleMode_SCE_GXM_MULTISAMPLE_NONE,
        };
        if g::vita2d_init_advanced_with_msaa(POOL_BYTES, msaa) < 0 {
            pocketjs_vita::vita_log(format_args!("duty: vita2d did not start"));
            return;
        }
        if let Err(error) = graphics::init_with_pool(POOL_BYTES) {
            pocketjs_vita::vita_log(format_args!("duty: graphics {error}"));
            return;
        }
        set_clocks();
        input::init();
        // The interface comes up first: it shows the load, and a failure.
        let mut shell = Shell { ui: interface::Ui::boot(), dev: dev::Host::new(), font: core::ptr::null_mut(), frame: 0, report: word("report") == Some("1") };
        channel().state.prefs = std::fs::read_to_string(format!("{}/{}", paths::DATA, paths::INTERFACE_FILE)).unwrap_or_default();

        // ------------------------------------------------------------------ the programs and the first map
        let first = word("map").and_then(maps::named).unwrap_or(maps::first());
        shell.frame(Mode::Loading, &format!("Reading {}", first.title));
        let started = (|| -> Result<_, String> {
            let packs = Packs::open(live)?;
            let mut gpu = Gpu::new(live)?;
            shell.frame(Mode::Loading, "Preparing the programs");
            let programs = Programs::new(&mut gpu, msaa as u32)?;
            let skins = Skins::new(&mut gpu, msaa as u32)?;
            gpu.finish();
            Ok((packs, gpu, programs, skins))
        })();
        let (packs, gpu, programs, skins) = match started {
            Ok(x) => x,
            Err(e) => shell.fail(e),
        };
        let linear = word("linear") == Some("1");
        let first = if packs.maps().contains(&first) { first } else { packs.maps().first().copied().unwrap_or(first) };
        let mut world = match World::read(&packs, first, linear, &mut shell) {
            Ok(world) => world,
            Err(e) => shell.fail(e),
        };

        let mut session = Session::new();
        // The interface is laid out for 480 × 272: pixels from the middle of its screen to its upper edge, over
        // the tangent of half the view.
        session.focal = 136.0 / tan(FOV * 0.5 * (PI / 180.0));
        let mut sounding = Sounding { mixer: Mixer::new(), bed: None, played: 0, on: audio::start(mix::OUTPUT_RATE) };
        enter(&mut world, &mut session, &mut sounding, &packs, false);
        let mut set = Settings { look: Look::DEFAULT, nearer: 1.0, everything: false, view: None, auto: false, god: false, pad: (Pad::default(), 0), pace: 1, profile: false, interface: true, budget: BUDGET, govern: true };
        remote(&boot, &mut set, &mut session, &mut shell.ui, Some(&mut world));

        let mut ring = match Ring::new((fx::VERTICES + 4) * core::mem::size_of::<FxVertex>() + 4096, 2) {
            Ok(r) => r,
            Err(e) => shell.fail(e),
        };
        let mut fence = Fence::new(0, 2);
        let control = if live { control_watcher() } else { mpsc::channel().1 };
        let ctx = g::vita2d_get_context();
        let mut timing = Timing::new();
        let mut last = Instant::now();
        let mut last_vcount = sceDisplayGetVcount();
        let (mut sim_ms, mut select_ms, mut draw_ms, mut gpu_ms, mut ui_ms, mut ui_draw_ms) = (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32);
        let mut was_profiling = false;
        let mut frame_no = shell.frame;
        let mut clock_tick = 0u32;
        let mut was_playing = false;
        let mut orbit = 0.0f32;
        // How far the levels of detail are in for the triangle budget, and the frames they stay in after the
        // GPU fell behind.
        let (mut scale, mut hold) = (1.0f32, 0u32);
        let mut draws: Vec<Draw> = Vec::with_capacity(4096);
        let mut squares = vec![FxVertex::default(); fx::VERTICES];
        let mut counts;
        let (mut street, mut people) = (Drawn::default(), Drawn::default());
        let mut mode_was = session.mode;
        let mut lists = 0u32;
        // The last loading frame drew from the start of vita2d's pool: it leaves the GPU before the first frame
        // of the loop writes there. What mounting the screens left behind is collected before that frame.
        g::vita2d_wait_rendering_done();
        shell.ui.collect();

        loop {
            // -------------------------------------------------------------- input and the match
            let t_frame = Instant::now();
            let raw = input::read();
            let (buttons, action) = shell.dev.menu.input(raw.buttons);
            // While the Devkit menu is up the pad is its own.
            let menu = shell.dev.menu.visible;
            let device = if menu { interface::NEUTRAL } else { input::Pad { buttons, ..raw } };
            while let Ok(words) = control.try_recv() {
                remote(&words, &mut set, &mut session, &mut shell.ui, Some(&mut world));
            }
            // What the interface asked for on its last turn; what it wants kept goes to the card.
            if let Some(text) = session.obey(&mut world.game) {
                paths::write_text(paths::INTERFACE_FILE, &text);
                channel().state.prefs = text;
            }
            // Another map: this one's memory goes back, the loading screen stands while the other's pack is
            // read, and a match starts on it.
            if let Some(to) = session.wanted.take().and_then(maps::named) {
                sounding.silence();
                session.mode = Mode::Loading;
                shell.frame = frame_no;
                shell.frame(Mode::Loading, &format!("Reading {}", to.title));
                let from = world.map;
                world.leave(&mut shell);
                world = match World::read(&packs, to, linear, &mut shell) {
                    Ok(world) => world,
                    // The map that was held comes back, with why the other did not.
                    Err(e) => {
                        pocketjs_vita::vita_log(format_args!("duty: {e}"));
                        match World::read(&packs, from, linear, &mut shell) {
                            Ok(world) => world,
                            Err(e) => shell.fail(e),
                        }
                    }
                };
                let play = world.map.id == to.id;
                enter(&mut world, &mut session, &mut sounding, &packs, play);
                frame_no = shell.frame;
                g::vita2d_wait_rendering_done();
                shell.ui.collect();
                timing.clear();
                last = Instant::now();
                last_vcount = sceDisplayGetVcount();
                was_playing = false;
                orbit = 0.0;
                continue;
            }
            let w = &mut world;
            let vcount = sceDisplayGetVcount();
            // A frame is a whole number of refreshes, and a refresh is one step of the match.
            let ticks = refreshes(vcount, last_vcount).clamp(1, 4) as u32;
            last_vcount = vcount;
            let dt = ticks as f32 * DT;

            let (mut held, second) = session_pad(&device);
            if set.pad.1 > 0 {
                set.pad.1 -= 1;
                held = set.pad.0;
            }
            if set.auto && session.mode == Mode::Play {
                // The bench's player: he turns to the nearest soldier in sight and holds the trigger.
                let (fire, reload) = hear::bench_aim(&mut w.game, dt);
                held.held &= !(pad::R | pad::L | pad::LEFT);
                if reload {
                    held.held |= pad::LEFT;
                }
                if fire {
                    held.held |= pad::R;
                }
            }
            // The right stick looks, whatever the layout the person chose for the buttons.
            session.second = Some(second);
            {
                let (fx, sounding, sounds) = (&mut w.fx, &mut sounding, &w.sounds);
                session.run(&mut w.game, &held, ticks, |game, event| {
                    let (eye, yaw, pitch, _, _) = game.player.eye();
                    let ahead = forward(yaw, pitch);
                    let right = ahead.cross(V3::UP).norm_or(v3(1.0, 0.0, 0.0));
                    fx.on_event(event, eye, ahead, right, right.cross(ahead));
                    let mixer = &mut sounding.mixer;
                    hear::sound(game, event, &mut sounding.played, |p| {
                        mixer.play(sounds, p.id as u16, p.left, p.right, p.pitch, p.priority.min(BED - 1));
                    });
                });
            }
            if set.god {
                w.game.player.health = 100.0;
            }
            let playing = matches!(session.mode, Mode::Play | Mode::Down);
            if playing {
                w.fx.step(dt);
            }
            if playing != was_playing {
                // A stopped match is silent but for the street.
                if !playing {
                    sounding.silence();
                } else {
                    // (late frames are counted from where a match starts or goes on)
                    timing.clear();
                }
                sounding.ambience(&w.sounds, Id::Ambience, if playing { 110 } else { 70 });
            }
            was_playing = playing;
            sounding.pump(&w.sounds, &w.samples);

            // The eye: behind the title it stands where a match starts, a little above a head, and looks slowly
            // up and down the street.
            let camera = if let Some((eye, yaw, pitch, fov)) = set.view {
                Camera { eye, yaw, pitch, roll: 0.0, fov }
            } else if matches!(session.mode, Mode::Title | Mode::Loading) {
                orbit += dt;
                let scene = &w.scene;
                let eye = v3(scene.start[0], scene.start[1] + 2.3, scene.start[2]);
                Camera { eye, yaw: scene.start[3] + sin(orbit * 0.11) * 0.42, pitch: 0.03 + sin(orbit * 0.07) * 0.03, roll: 0.0, fov: FOV }
            } else {
                let (eye, yaw, pitch, roll, zoom) = w.game.player.eye();
                Camera { eye, yaw, pitch, roll, fov: FOV * zoom }
            };
            sim_ms = sim_ms * 0.9 + t_frame.elapsed().as_secs_f32() * 100.0;

            // -------------------------------------------------------------- the interface
            // It is shown the match as it now is, then takes its turn.
            let tu = Instant::now();
            {
                let state = &mut channel().state;
                session.publish(&w.game, state);
                if !session.stats {
                    state.stats.clear();
                } else if frame_no % 30 == 0 {
                    state.stats = format!("{:.1} fps · {:.1} ms · late {} · {} draws · {:.1}k tris", 1000.0 / timing.avg().max(0.1), timing.avg(), timing.late, street.draws + people.draws, (street.triangles + people.triangles) as f32 / 1000.0);
                }
            }
            if session.mode != mode_was {
                // The guest's collector runs where the screen changes whole: the title comes back, and at every
                // 64th list besides.
                if !matches!(session.mode, Mode::Play | Mode::Down) {
                    lists += 1;
                }
                if session.mode == Mode::Title || (session.mode == Mode::Menu && lists % 64 == 0) {
                    shell.ui.collect();
                }
                mode_was = session.mode;
            }
            shell.ui.turn(dt, &device, menu, Some(&session));
            ui_ms = ui_ms * 0.9 + tu.elapsed().as_secs_f32() * 100.0;

            // -------------------------------------------------------------- what the eye sees
            let ts = Instant::now();
            let aspect = WIDTH / HEIGHT;
            let look = forward(camera.yaw, camera.pitch);
            let projection = mat::perspective(camera.fov, aspect, NEAR, FAR);
            let seen = mat::view(camera.eye, look, camera.roll);
            let vp = mat::mul(&projection, &seen);
            let eye_turn = mat::mul(&projection, &mat::view(V3::ZERO, look, camera.roll));
            // What the eye's cell sees, inside the view and not turned away, at the level of detail its distance
            // asks for: every device's own choice (`duty_sim::view`).
            let planes = mat::unit_planes(&mat::planes(&vp));
            let guard = Guard::new(&vp, NEAR, WIDTH, HEIGHT, 2048.0).boxes_along(&w.scene.level_x, &w.scene.level_z);
            let pack = view::World { scene: &w.scene, chunks: &w.chunks, clusters: &w.clusters, larges_at: &w.larges_at, cells: &w.cells, lists: &w.lists, bits: &w.bits };
            counts = view::select(&pack, &mut w.by_cell, &planes, camera.eye, &guard, &view::Options { nearer: set.nearer * scale, everything: set.everything }, &mut draws);
            if set.look.merge > 0 {
                world::merge(&mut draws, set.look.merge);
            }
            // The soldiers, the effects and the held weapon stand once a match has the street.
            let stood = !matches!(session.mode, Mode::Title | Mode::Loading) && set.view.is_none();
            let right = look.cross(V3::UP).norm_or(v3(1.0, 0.0, 0.0));
            let up = right.cross(look);
            let (covering, glowing) = if set.look.effects && (stood || set.view.is_some()) { w.fx.build(camera.eye, right, up, |_| true, &mut squares) } else { (0, 0) };
            select_ms = select_ms * 0.9 + ts.elapsed().as_secs_f32() * 100.0;

            // -------------------------------------------------------------- the scene
            let t2 = Instant::now();
            let slot = (frame_no % 2) as usize;
            // This slot's vertices were last used two frames ago: the GPU must be done with them.
            fence.wait(slot);
            let waited = t2.elapsed();
            ring.next_frame();
            // The interface and the Devkit menu draw from vita2d's one pool of temporary vertices. This slot's
            // frame takes its own half, last written two frames ago, which `fence.wait(slot)` above saw out of
            // the GPU: no frame overwrites vertices still being read.
            g::vita2d_pool_reset();
            if slot == 1 {
                g::vita2d_pool_malloc(POOL_BYTES / 2);
            }
            g::vita2d_start_drawing_advanced(core::ptr::null_mut(), 0);

            // A muzzle's light: the player's own flash, where his muzzle is in the world.
            let held_now = if stood && set.look.figures { w.figures.as_ref().and_then(|f| f.posed_held(&w.game)) } else { None };
            let flash = held_now.as_ref().filter(|_| w.game.arms.flash > 0).map(|h| {
                let m = h.muzzle;
                (camera.eye + right * m.x + up * m.y - look * m.z, 1.4 * w.game.arms.flash as f32)
            });
            street = w.gfx.street(ctx, &programs, &w.chunks, &View { world: vp, eye_turn, eye: camera.eye, draws: &draws, look: set.look, flash });
            people = Drawn::default();
            if set.look.figures && (stood || set.view.is_some()) {
                if let Some(figures) = &w.figures {
                    people = figures.soldiers(ctx, &skins, &w.gfx, &eye_turn, camera.eye, look, &w.game, &set.look);
                }
            }
            if covering + glowing > 0 {
                // The squares, and the flash after them, in memory the GPU reads until this frame is drawn.
                if let Some(at) = ring.alloc((fx::VERTICES + 4) * core::mem::size_of::<FxVertex>(), 16) {
                    let at = at as *mut FxVertex;
                    core::ptr::copy_nonoverlapping(squares.as_ptr(), at, covering + glowing);
                    let drawn = w.gfx.effects(ctx, &programs, &eye_turn, at, covering, glowing);
                    street.draws += drawn.draws;
                    street.triangles += drawn.triangles;
                }
            }
            if let (Some(figures), Some(held)) = (&w.figures, &held_now) {
                {
                    let (frustum, muzzle) = figures.held(ctx, &skins, &w.gfx, &seen, &w.game, &set.look, held, &mut people);
                    if w.game.arms.flash > 0 && set.look.effects {
                        if let Some(at) = ring.alloc(4 * core::mem::size_of::<FxVertex>(), 16) {
                            let (centre, half) = (muzzle + v3(0.0, 0.0, -0.06), 0.035 + 0.012 * w.game.arms.flash as f32);
                            let square = [(-1.0, 1.0), (1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)].map(|(sx, sy): (f32, f32)| FxVertex { uv: [0.02 + (sx * 0.5 + 0.5) * 0.46, 0.52 + (0.5 - sy * 0.5) * 0.46], colour: [255, 220, 160, 240], pos: [centre.x + sx * half, centre.y + sy * half, centre.z] });
                            core::ptr::copy_nonoverlapping(square.as_ptr(), at as *mut FxVertex, 4);
                            w.gfx.flash(ctx, &programs, &frustum, at as *const FxVertex);
                        }
                    }
                }
            }
            // vita2d's draws (the interface, the menu) expect its own viewport and no depth.
            g::sceGxmSetViewport(ctx, 480.0, 480.0, 272.0, -272.0, 0.5, 0.5);
            gpu::state_overlay(ctx, false);
            let td = Instant::now();
            if set.interface {
                shell.ui.draw();
            }
            ui_draw_ms = ui_draw_ms * 0.9 + td.elapsed().as_secs_f32() * 100.0;
            shell.dev.overlay();
            g::sceGxmEndScene(ctx, core::ptr::null(), fence.signal(slot));
            draw_ms = draw_ms * 0.9 + t2.elapsed().saturating_sub(waited).as_secs_f32() * 100.0;
            if set.profile {
                let tg = Instant::now();
                fence.wait(slot);
                gpu_ms = gpu_ms * 0.9 + tg.elapsed().as_secs_f32() * 100.0;
            } else if was_profiling {
                // (a profile run waits for the GPU every frame: its late frames are not the game's)
                timing.clear();
            }
            was_profiling = set.profile;
            // The distances of the levels of detail follow the triangle budget: in quickly when a frame draws too
            // many, out slowly when there is room. They also come in when this frame had to wait for the GPU to
            // finish the frame before last: the GPU is then a refresh behind and the next frames would be late.
            // After such a wait the distances stay in for two seconds.
            if set.govern {
                let behind = !set.profile && waited.as_secs_f32() > BEHIND;
                if behind {
                    hold = 120;
                }
                if street.triangles > set.budget {
                    scale *= 0.97;
                } else if behind {
                    scale *= 0.985;
                } else if (street.triangles as f32) < set.budget as f32 * 0.88 && hold == 0 {
                    scale *= 1.008;
                }
                hold = hold.saturating_sub(1);
                scale = clamp(scale, 0.3, 1.0);
            } else {
                scale = 1.0;
            }
            let worked = t_frame.elapsed().saturating_sub(waited);
            g::vita2d_swap_buffers();
            // Hold the pace: a frame is shown for `pace` refreshes.
            while refreshes(sceDisplayGetVcount(), last_vcount) < set.pace {
                sceDisplayWaitVblankStart();
            }
            let now = Instant::now();
            let shown = refreshes(sceDisplayGetVcount(), last_vcount);
            timing.push((now - last).as_secs_f32() * 1000.0, shown > set.pace);
            if shown > set.pace {
                if worked.as_secs_f32() > 0.012 {
                    timing.late_cpu += 1;
                } else {
                    timing.late_gpu += 1;
                }
            }
            last = now;

            // -------------------------------------------------------------- status
            clock_tick += 1;
            // A match nobody touches for a while is still being watched: the console's own timers (dimming,
            // the display off, standby) start again.
            if clock_tick % 120 == 0 {
                sceKernelPowerTick(0);
            }
            // The system lowers the clocks after a suspend; set them again.
            if clock_tick % 60 == 0 && scePowerGetArmClockFrequency() < CLOCKS[0] - 20 {
                set_clocks();
            }
            if frame_no % 10 == 0 {
                let (game, c) = (&w.game, &counts);
                let p = &game.player;
                shell.dev.engine = json!({
                    "target": "vita",
                    "stage": "running",
                    "mode": session.mode.name(),
                    "map": w.map.id,
                    "maps": session.maps,
                    "interface": {"up": shell.ui.up(), "open": channel().is_open(), "error": shell.ui.error, "turns": shell.ui.turns, "taken": shell.ui.pace.taken, "turnMs": shell.ui.turn_ms, "worstTurnMs": shell.ui.worst_ms, "collections": shell.ui.collections, "collectMs": shell.ui.collect_ms},
                    "pack": {"bytes": w.bytes, "fetched": w.fetched, "from": packs.directory(), "id": serde_json::from_str::<Value>(&w.meta).unwrap_or(Value::String(w.meta.clone())), "chart": w.chart.is_some()},
                    "loadMs": w.load_ms,
                    "frameMs": timing.avg(), "worstMs": timing.worst(), "late": timing.late, "lateBy": {"cpu": timing.late_cpu, "gpu": timing.late_gpu}, "frames": timing.frames, "pace": set.pace,
                    "cpuMs": {"match": sim_ms, "select": select_ms, "draw": draw_ms, "interface": ui_ms, "interfaceDraw": ui_draw_ms},
                    "gpuMs": if set.profile { json!(gpu_ms) } else { Value::Null },
                    "draws": street.draws + people.draws,
                    "tris": street.triangles,
                    "figureTris": people.triangles,
                    "clusters": {"listed": c.clusters, "unseen": c.unseen, "outside": c.outside, "turned": c.turned, "drawn": c.drawn},
                    "chunks": {"listed": c.listed, "seen": c.chunks},
                    "governor": {"on": set.govern, "budget": set.budget, "scale": scale, "held": hold},
                    "eye": [camera.eye.x, camera.eye.y, camera.eye.z], "yaw": camera.yaw * (180.0 / PI), "pitch": camera.pitch * (180.0 / PI),
                    "player": {"pos": [p.body.pos.x, p.body.pos.y, p.body.pos.z], "yaw": p.yaw, "pitch": p.pitch, "speed": p.speed(), "sprinting": p.sprinting, "crouched": p.crouched, "sights": game.arms.ads},
                    "match": {
                        "health": p.health, "us": game.score[0], "them": game.score[1], "clock": game.clock, "standing": game.standing(), "tick": game.tick,
                        "kills": game.tally.kills, "deaths": game.tally.deaths, "shots": game.tally.shots, "hits": game.tally.hits,
                        "weapon": game.arms.active, "magazine": game.arms.magazine[game.arms.active], "reserve": game.arms.reserve[game.arms.active],
                    },
                    "sound": {"on": sounding.on, "sounds": w.sounds.len(), "played": sounding.played, "voices": sounding.mixer.playing(), "rate": mix::OUTPUT_RATE},
                    "settings": {"auto": set.auto, "god": set.god, "nearer": set.nearer, "everything": set.everything, "profile": set.profile, "interface": set.interface, "merge": set.look.merge, "cull": set.look.cull,
                        "sky": set.look.sky, "haze": set.look.haze, "shadows": set.look.shadows, "effects": set.look.effects, "figures": set.look.figures, "shades": set.look.shades, "flash": set.look.flash},
                    "programs": {"compiled": gpu.compiled, "cached": gpu.cached},
                    "msaa": samples,
                    "pictures": if linear { "rows" } else { "swizzled" },
                    "memory": {"geometry": w.gfx.geometry_bytes, "pictures": w.gfx.picture_bytes, "vram": w.vram.reserved(), "vertices": w.gfx.vertex_count, "indices": w.gfx.index_count},
                    "clockMhz": [scePowerGetArmClockFrequency(), scePowerGetGpuClockFrequency()],
                });
            }
            shell.dev.publish(frame_no, "duty");
            if shell.report && frame_no % 30 == 0 {
                paths::write_text("status.json", &shell.dev.engine.to_string());
            }
            serve(&mut shell.dev, frame_no, action);
            frame_no = frame_no.wrapping_add(1);
        }
    }
}

/// Answers the wired-debug host: status, the Devkit menu, a capture, a file staged on the card, another native
/// build to start.
unsafe fn serve(dev: &mut dev::Host, frame: u32, action: Action) {
    let mut request = dev.poll();
    let op = request.as_ref().map(|r| r.command.op).or(match action {
        Action::Capture => Some(Op::Capture),
        _ => None,
    });
    match op {
        Some(Op::Status) => request.take().unwrap().finish(Ok(dev.status(frame, "duty"))),
        Some(Op::Menu) => {
            dev.menu.visible = !dev.menu.visible;
            request.take().unwrap().finish(Ok(json!({"menu": dev.menu.visible})));
        }
        Some(Op::Capture) => {
            if let Some(request) = request.take() {
                let _ = request.reply.try_send(dev.capture(frame));
            } else {
                dev.capture_from_menu(frame);
            }
        }
        Some(Op::Stage) => {
            // The host's worker wrote the file and read it back before this request reached the frame.
            let request = request.take().unwrap();
            let path = pocketjs_vita::dev_protocol::stage_path(&request.command, dev::TITLE_ID);
            request.finish(Ok(json!({ "path": path })));
        }
        Some(Op::Native) => {
            let request = request.take().unwrap();
            g::vita2d_wait_rendering_done();
            let result = dev::exec_native(request.native_path.as_ref().unwrap());
            request.finish(result.map(|_| json!({})));
        }
        Some(Op::Push | Op::Reload | Op::Reset) => {
            if let Some(request) = request.take() {
                request.finish(Err("Pocket Duty's interface is read from the share when the game starts; use native to start it again".into()));
            }
        }
        None => {}
    }
}
