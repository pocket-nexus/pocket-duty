//! Pocket Duty on PSP.
//!
//! The reference's street, its soldiers and its weapons, lowered by the
//! compiler for this machine (`profiles/psp30.json`): 480 × 272 at 30 frames a
//! second, the GE's fixed-function pipeline, one stick.
//!
//! Controls in a match: the stick walks (all the way forward runs), triangle,
//! circle, cross and square look, R fires, L raises the sights, up on the pad
//! jumps and climbs, down crouches, left changes the magazine, right changes
//! the weapon, SELECT changes the fire mode. A setting swaps the stick and
//! the four buttons. Everything else is the interface's (`interface`): a
//! PocketJS guest drawn over the street, which START brings up as a menu.
//!
//! Development loop over PSPLINK: the pack and the interface's bundle are
//! read from `host0:/duty/`, `control.txt` there steers the run and
//! `status.json` reports it.

#![no_std]
#![no_main]

extern crate alloc;

mod audio;
mod figures;
mod gfx;
mod interface;
mod mem;
mod store;

use alloc::format;
use alloc::string::String;
use core::ffi::c_void;
use core::fmt::Write;

use duty_interface::{channel, pad, parse_f32, Mode, Pad, Session};
use duty_pack::sound::{Id, Sound};
use duty_pack::{self as pack, Scene};
use duty_sim::bvh::{Bvh, Node, Tri};
use duty_sim::hear;
use duty_sim::game::Game;
use duty_sim::level::Level;
use duty_sim::math::*;
use duty_sim::nav::{Path, Pathfinder};
use duty_sim::tune::{WeaponDef, DT};
use duty_sim::weapon::{self, WEAPONS};
use pocketjs_psp::{arena, host};
use psp::sys::*;

psp::module!("PocketDuty", 1, 0);

/// Degrees the view takes top to bottom at its widest.
const FOV: f32 = 65.0;

fn psp_main() {
    psp::enable_home_button();
    unsafe {
        // This thread has the EBOOT's directory as its working directory, and no other thread does:
        // it opens what lies there, and once the frame thread runs it serves storage below it.
        store::locate();
        // The frame thread, with this thread's attributes and four times its stack: the interface's
        // guest compiles its bundle and mounts its screens on the thread that runs it, and that
        // takes more than 256 KB.
        let id = sceKernelCreateThread(b"duty_frame\0".as_ptr(), frame_thread, 32, 1024 * 1024, ThreadAttributes::USER | ThreadAttributes::VFPU, core::ptr::null_mut());
        if id.0 < 0 {
            psp::dprintln!("Could not start: the frame thread was not created ({:08x})", id.0 as u32);
            loop {
                sceKernelDelayThread(1_000_000);
            }
        }
        sceKernelStartThread(id, 0, core::ptr::null_mut());
        sceKernelChangeThreadPriority(SceUid(sceKernelGetThreadId()), 40);
        store::serve()
    }
}

unsafe extern "C" fn frame_thread(_: usize, _: *mut c_void) -> i32 {
    start();
    0
}

unsafe fn start() {
    // PSPLINK can start a thread with floating-point exceptions on; the interface's layout computes with NaN.
    host::reset_fpu_status();
    let mut ui = interface::Ui::none("");
    let mut on_screen = false;
    if let Err(e) = run(&mut ui, &mut on_screen) {
        store::note(&format!("{{\"target\":\"psp\",\"stage\":\"failed\",\"error\":\"{e}\",\"code\":\"{:08x}\"}}", core::ptr::addr_of!(store::LAST_CODE).read() as u32));
        if !on_screen {
            psp::dprintln!("Could not start: {}", e);
            // A test run (`boot.txt` names a frame to leave at) has nothing more to show.
            let mut buf = [0u8; 512];
            if store::boot_text(&mut buf).is_some_and(|text| text.contains("exit=")) {
                sceKernelDelayThread(300_000);
                sceKernelExitGame();
            }
        }
        let state = &mut channel().state;
        state.mode = Mode::Error;
        state.message.clear();
        state.message.push_str(e);
        let mut frames = 0u32;
        // No match to pace the guest by: it takes every turn.
        let waking = Session::new();
        loop {
            if on_screen && ui.up() {
                ui.turn(interface::TURN, 0, interface::STICK_CENTER, &waking);
                gfx::interlude(&ui);
                frames += 1;
                // A test run (`boot.txt` names a frame to leave at) ends here, with this screen as its frame.
                let mut buf = [0u8; 512];
                if frames == 30 && store::boot_text(&mut buf).is_some_and(|text| text.contains("exit=")) {
                    store::write_shot(&gfx::pixels(gfx::DRAW_BUFFER ^ 1));
                    sceKernelExitGame();
                }
            } else {
                sceKernelDelayThread(1_000_000);
            }
        }
    }
}

/// The pad as the flow reads it.
fn read_pad(data: &SceCtrlData) -> Pad {
    let b = data.buttons;
    // A worn stick rests off centre: nothing inside a quarter of its travel, the rest rescaled.
    let axis = |v: u8| {
        let x = (v as f32 - 127.5) / 127.5;
        let m = if x < 0.0 { -x } else { x };
        if m < 0.24 {
            0.0
        } else {
            (m - 0.24) / 0.76 * if x < 0.0 { -1.0 } else { 1.0 }
        }
    };
    let mut held = 0;
    for (from, to) in [
        (CtrlButtons::UP, pad::UP),
        (CtrlButtons::DOWN, pad::DOWN),
        (CtrlButtons::LEFT, pad::LEFT),
        (CtrlButtons::RIGHT, pad::RIGHT),
        (CtrlButtons::TRIANGLE, pad::NORTH),
        (CtrlButtons::CIRCLE, pad::EAST),
        (CtrlButtons::CROSS, pad::SOUTH),
        (CtrlButtons::SQUARE, pad::WEST),
        (CtrlButtons::LTRIGGER, pad::L),
        (CtrlButtons::RTRIGGER, pad::R),
        (CtrlButtons::START, pad::START),
        (CtrlButtons::SELECT, pad::SELECT),
    ] {
        if b.contains(from) {
            held |= to;
        }
    }
    Pad { held, sx: axis(data.lx), sy: -axis(data.ly) }
}

/// What a control text may set beside the flow: a pad held for a number of frames, a soldier who
/// plays for the player, the frame's budget, the development switches.
#[derive(Default)]
struct Remote {
    /// A pad to use instead of the device's, and for how many more frames.
    pad: Pad,
    pad_frames: u32,
    /// The player's aim and trigger are a bench's: at the nearest soldier, whenever one is in sight.
    auto: bool,
    /// Nothing hurts the player: for looking at soldiers up close.
    god: bool,
    /// Frames still to be filmed, the frames filmed so far, and the buffer a frame is copied to.
    film: u32,
    filmed: u32,
    reel: alloc::vec::Vec<u8>,
    /// The bench's legs: where the player is walked to and how far the stick is pushed, the way
    /// there over the walk grid, and the search that finds it.
    go: Option<(V3, f32)>,
    way: Path,
    way_at: u8,
    way_asked: bool,
    finder: Option<Pathfinder>,
    /// Where the legs stood a frame ago, and for how many frames they have stood still.
    stood: V3,
    still: u8,
}

/// The words of a control text. In `boot.txt`, `@<frame>:<word>` says a word at that frame, so a
/// test run can walk the interface. `mode=title|play|menu` sets the flow outright, `ui=start|menu|
/// resume|restart|title` asks what the interface's lists ask, `press=<mask>` presses buttons on the
/// interface as a thumb would (PocketJS's button bits), `rest=<turns>` leaves it alone that many
/// of its turns, `pad=<held>,<x>,<y>,<frames>` holds the player's pad (the flow's own bits, the
/// stick in hundredths), `at=<x>,<y>,<z>,<yaw>,<pitch>` puts the player somewhere (degrees),
/// `near=<soldier>,<metres>` puts him in front of a soldier, `auto=1` hands his aim and trigger to
/// the bench, `god=1` keeps him whole, `hurt=<points>` hits him from ahead, `clock=<seconds>` sets
/// what is left of the match, `skill=<hundredths>` sets how hard the soldiers hit,
/// `budget=<triangles>` and `option=<bits>` are the renderer's, `film=<frames>` writes the picture
/// of each of the next frames to `host0:/duty/film/` (480 × 272 texels of 16 bits a file) and
/// steps the match two sixtieths of a second a frame while it does, whatever the writing takes,
/// `go=<x>,<z>[,<stick>]` walks the player there over the walk grid as a soldier would find his
/// way, the stick pushed that many hundredths (100 runs).
unsafe fn remote(text: &str, ui: &mut interface::Ui, session: &mut Session, game: &mut Game, gfx: &mut gfx::Gfx, state: &mut Remote) {
    for word in text.split_ascii_whitespace() {
        // (a word for a later frame is the loop's to say when its frame comes)
        if word.starts_with('@') {
            continue;
        }
        let Some((key, value)) = word.split_once('=') else { continue };
        let mut numbers = value.split(',').map(|n| parse_f32(n).unwrap_or(0.0));
        let mut next = || numbers.next().unwrap_or(0.0);
        match key {
            "press" => ui.press(next() as u32),
            "rest" => ui.rest(next() as u32),
            "mode" => {
                if let Some(mode @ (Mode::Title | Mode::Play | Mode::Menu)) = Mode::parse(value) {
                    if mode == Mode::Play && session.mode == Mode::Title {
                        session.command(game, duty_interface::Command::Start);
                    }
                    session.mode = mode;
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
            "pad" => {
                state.pad = Pad { held: next() as u32, sx: next() * 0.01, sy: next() * 0.01 };
                state.pad_frames = next() as u32;
            }
            "at" => {
                let (x, y, z, yaw, pitch) = (next(), next(), next(), next(), next());
                game.player.place(v3(x, y, z), yaw * (PI / 180.0));
                game.player.pitch = pitch * (PI / 180.0);
                session.apply(game);
            }
            "auto" => state.auto = next() != 0.0,
            "clock" => game.clock = next(),
            "hurt" => {
                let from = game.player.head() + forward(game.player.yaw, 0.0) * 8.0;
                game.player.hurt(next(), from, &mut game.events);
            }
            "god" => state.god = next() != 0.0,
            "film" => state.film = next() as u32,
            "go" => {
                let (x, z, stick) = (next(), next(), next());
                state.go = Some((v3(x, 0.0, z), if stick > 0.0 { stick * 0.01 } else { 1.0 }));
                state.way = Path::default();
                state.way_at = 0;
                state.way_asked = false;
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
            "budget" => gfx.budget = next() as u32,
            "run" => gfx.run = (next() as u32).max(1),
            "option" => gfx.option = next() as u32,
            _ => {}
        }
    }
}

/// The Pocket3D title card, drawn into video memory before the GE is set up.
unsafe fn title() {
    // the uncached mirror of video memory: what is written is what the display reads
    let vram = (sceGeEdramGetAddr() as usize | 0x4000_0000) as *mut u8;
    sceDisplaySetMode(DisplayMode::Lcd, 480, 272);
    let mut surface = pocket3d_title::Surface { pixels: core::slice::from_raw_parts_mut(vram, 512 * 272 * 4), width: 480, height: 272, stride: 512, layout: pocket3d_title::Layout::Rgba8 };
    pocket3d_title::play(&mut surface, |_| {
        sceDisplaySetFrameBuf(vram, 512, DisplayPixelFormat::Psm8888, DisplaySetBufSync::NextFrame);
        sceDisplayWaitVblankStart();
    });
}

/// Rolling frame statistics over the last `N` frames.
struct Timing {
    ms: [f32; Timing::N],
    at: usize,
    late: u32,
    frames: u32,
}

impl Timing {
    const N: usize = 60;
    fn push(&mut self, ms: f32, late: bool) {
        self.ms[self.at] = ms;
        self.at = (self.at + 1) % Timing::N;
        self.frames += 1;
        self.late += late as u32;
    }
    fn avg(&self) -> f32 {
        self.ms.iter().sum::<f32>() / Timing::N as f32
    }
    fn worst(&self) -> f32 {
        self.ms.iter().fold(0.0f32, |a, b| if *b > a { *b } else { a })
    }
}

/// Milliseconds a frame may take without its interface turn before the levels of detail come in.
const BUSY: f32 = 24.5;

fn micros() -> u32 {
    unsafe { sceKernelGetSystemTimeLow() }
}

unsafe fn run(ui: &mut interface::Ui, on_screen: &mut bool) -> Result<(), &'static str> {
    scePowerSetClockFrequency(333, 333, 166);
    // Test runs: `boot.txt` on the share holds control words for the start, plus `title=0` (no title card),
    // `probe=1` (time the frame's smallest pieces of work on this console and write `probe.json`),
    // `shot=N` (write frame N to `shot.raw`) and `exit=N` (leave at frame N).
    // (a film's script is longer than a test run's words: `BOOT_MAX` in tools/psp.ts is this size)
    let mut boot_buf = [0u8; 2048];
    let boot: &str = store::boot_text(&mut boot_buf).unwrap_or("");
    let word = |key: &str| boot.split_ascii_whitespace().find_map(|w| w.strip_prefix(key).and_then(|w| w.strip_prefix('=')).and_then(parse_f32));
    if word("title") != Some(0.0) {
        title();
    }
    psp::dprintln!("Pocket Duty\n");
    let t_load = sceKernelGetSystemTimeLow();
    let free_at_start = sceKernelTotalFreeMemSize();
    let file = store::PackFile::open()?;
    let scene: Scene = *file.records::<Scene>(pack::SCEN)?.first().ok_or("scene section")?;

    // The interface starts first when memory has room for it beside the pack: the loading steps
    // are then its first screen.
    let room = arena::stats().tail_free_bytes + mem::kernel_room();
    let arena_before_ui = arena::stats().bump_bytes;
    if room < file.resident_bytes() + 1024 * 1024 + interface::RESERVE {
        *ui = interface::Ui::none("no memory for the interface beside the pack");
        store::leave_beside(store::SCRIPT);
        store::leave_beside(store::PAK);
    } else {
        psp::dprintln!("  Starting the interface");
        *ui = interface::Ui::boot(store::read_beside(store::SCRIPT, &[0]), store::keep_beside(store::PAK));
        ui.bytes = arena::stats().bump_bytes - arena_before_ui;
    }
    if ui.up() {
        // From here the screen belongs to the GE, and the loading steps to the interface.
        gfx::init();
        *on_screen = true;
    }
    // While the pack loads the guest takes every turn it is offered.
    let waking = Session::new();
    let mut stage = |name: &str| {
        if ui.up() {
            let state = &mut channel().state;
            state.message.clear();
            state.message.push_str(name);
            ui.turn(interface::TURN, 0, interface::STICK_CENTER, &waking);
            gfx::interlude(ui);
        } else {
            psp::dprintln!("  {}", name);
        }
        if file.host {
            store::note(&format!("{{\"target\":\"psp\",\"stage\":\"loading\",\"step\":\"{name}\"}}"));
        }
    };
    let mut gfx = gfx::Gfx::load(&file, scene, &mut stage)?;
    stage("Reading the ground");
    let solid_verts: store::Kept<[f32; 3]> = file.resident(pack::COLV)?;
    let solid_tris: store::Kept<Tri> = file.resident(pack::COLT)?;
    let solid_nodes: store::Kept<Node> = file.resident(pack::COLN)?;
    let nav: store::Kept<u8> = file.resident(pack::NAVG)?;
    let weapons: alloc::vec::Vec<WeaponDef> = file.records(pack::WEAP)?;
    let defs: [WeaponDef; WEAPONS] = if weapons.len() == WEAPONS { [weapons[0], weapons[1], weapons[2]] } else { weapon::reference() };
    // The pack's sections live as long as the program.
    let solid = Bvh { nodes: &*(solid_nodes.as_slice() as *const [Node]), tris: &*(solid_tris.as_slice() as *const [Tri]), verts: &*(solid_verts.as_slice() as *const [[f32; 3]]) };
    let level: Level<'static> = Level::parse_nav(solid, &*(nav.as_slice() as *const [u8])).ok_or("the pack's walk grid")?;
    stage("Reading the soldiers");
    let mut figures = figures::Figures::load(&file);
    stage("Reading the sounds");
    let sounds: Option<(store::Kept<Sound>, store::Kept<u8>)> = file.resident(pack::SNDS).ok().zip(file.resident(pack::SNDD).ok());
    let sounding = match &sounds {
        Some((records, samples)) if records.len() == Id::ALL.len() => audio::start(&*(records.as_slice() as *const [Sound]), &*(samples.as_slice() as *const [u8])),
        _ => false,
    };
    stage("Starting");
    store::start(&file);
    if !*on_screen {
        // No interface: the debug text stood until here.
        gfx::init();
        *on_screen = true;
    }
    let mut game = Game::new(level, defs, 0x5eed_1234);
    game.timer = Some(micros);
    let mut session = Session::new();
    // The map picture's corner, as the compiler laid it out about the level's origin.
    session.map = (scene.level_origin[0] - 80.0, scene.level_origin[2] - 80.0);
    session.focal = 136.0 / tan(FOV * 0.5 * (PI / 180.0));
    session.apply(&mut game);
    {
        // What the interface asked to have kept in an earlier run.
        let mut buf = [0u8; store::KEPT_MAX];
        if let Some(text) = store::kept(&mut buf) {
            let state = &mut channel().state;
            state.prefs.clear();
            state.prefs.push_str(text);
        }
    }
    let mut steer = Remote::default();
    remote(boot, ui, &mut session, &mut game, &mut gfx, &mut steer);
    if file.host && word("probe") == Some(1.0) {
        // What this console takes for the frame's smallest pieces of work.
        let start = v3(scene.start[0], scene.start[1] + 1.6, scene.start[2]);
        let costs = gfx.probe(&gfx::Camera { eye: start, yaw: scene.start[3], pitch: -0.3, roll: 0.0, fov: FOV });
        let mut text = String::new();
        let _ = write!(text, "{{\"nanoseconds\":{{\"command\":{},\"wordPastCache\":{},\"wordThroughCache\":{},\"flush64\":{},\"modelMatrix\":{},\"classify\":{},\"cut\":{},\"multiplyAdd\":{},\"divide\":{},\"farRead\":{},\"clock\":{}}}}}", costs[0], costs[1], costs[2], costs[3], costs[4], costs[5], costs[6], costs[7], costs[8], costs[9], costs[10]);
        store::write_text(b"host0:/duty/probe.json\0", &text);
    }
    // (a frame number no run reaches when the word is absent)
    let (shot_at, exit_at) = (word("shot").map_or(u32::MAX, |v| v as u32), word("exit").map_or(u32::MAX, |v| v as u32));
    let load_ms = sceKernelGetSystemTimeLow().wrapping_sub(t_load) / 1000;
    let free_after = sceKernelTotalFreeMemSize();
    let arena_after_load = arena::stats().bump_bytes;
    if sounding {
        audio::ambience(Id::Ambience, 110);
    }

    sceCtrlSetSamplingCycle(0);
    sceCtrlSetSamplingMode(CtrlMode::Analog);
    let mut data: SceCtrlData = core::mem::zeroed();
    let mut status = String::with_capacity(3072);
    let mut prefs: Option<String> = None;
    let mut timing = Timing { ms: [33.3; Timing::N], at: 0, late: 0, frames: 0 };
    let (mut gpu_ms, mut sim_ms, mut ui_ms, mut ge_ms, mut ge_frames, mut busy_ms) = (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0u32, 0.0f32);
    let mut last_swap = sceKernelGetSystemTimeLow();
    let mut last_vcount = sceDisplayGetVcount();
    // Display refreshes a frame: 30 frames a second.
    let pace = 2u32;
    let mut ticks = pace;
    let mut frame = 0u32;
    let mut sounds_played = 0u32;
    let mut was_playing = false;
    let mut orbit = 0.0f32;
    let ms = |from: u32| sceKernelGetSystemTimeLow().wrapping_sub(from) as f32 / 1000.0;

    loop {
        // -------------------------------------------------------------- input and the match
        sceCtrlPeekBufferPositive(&mut data, 1);
        store::control(|text| remote(text, ui, &mut session, &mut game, &mut gfx, &mut steer));
        // The words `boot.txt` keeps for this frame.
        for word in boot.split_ascii_whitespace().filter(|w| w.starts_with('@')) {
            if let Some((at, word)) = word[1..].split_once(':') {
                if parse_f32(at).is_some_and(|at| at as u32 == frame) {
                    remote(word, ui, &mut session, &mut game, &mut gfx, &mut steer);
                }
            }
        }
        if let Some(text) = session.obey(&mut game) {
            prefs = Some(text);
        }
        // What the interface asked to have kept goes to the storage thread, which writes it beside the pack.
        if let Some(text) = prefs.take() {
            if !store::keep(&text) {
                prefs = Some(text);
            }
        }
        let mut pad = read_pad(&data);
        if steer.pad_frames > 0 {
            steer.pad_frames -= 1;
            pad = steer.pad;
        }
        if steer.auto && session.mode == Mode::Play {
            // The bench's player: he turns to the nearest soldier in sight and holds the trigger.
            let (fire, reload) = hear::bench_aim(&mut game, ticks as f32 * DT);
            pad.held &= !(pad::R | pad::L | pad::LEFT);
            if reload {
                pad.held |= pad::LEFT;
            }
            if fire {
                pad.held |= pad::R;
            }
        }
        if let Some((to, stick)) = steer.go.filter(|_| session.mode == Mode::Play) {
            // The bench's legs: one way asked of the walk grid, then the stick toward its next
            // point. The view turns that way too unless the bench's aim has it.
            let level = &game.level;
            let at = game.player.body.pos;
            let finder = steer.finder.get_or_insert_with(|| Pathfinder::new(level.flags.len()));
            if !steer.way_asked {
                steer.way_asked = true;
                match (level.nearest_open(at.x, at.z, at.y), level.nearest_below(to.x, to.z)) {
                    (Some(from), Some(goal)) => finder.ask(0, from, goal),
                    _ => steer.go = None,
                }
            }
            if let Some((_, way)) = finder.step(level, 3000) {
                steer.way = way;
                steer.way_at = 0;
                if way.len == 0 {
                    steer.go = None;
                }
            }
            if steer.way_at < steer.way.len {
                let toward = (steer.way.points[steer.way_at as usize] - at).flat();
                if toward.len2() < 0.4 * 0.4 {
                    steer.way_at += 1;
                    // A way holds sixteen points: a long walk asks again from where one ends.
                    if steer.way_at == steer.way.len {
                        if (to - at).flat().len2() > 1.5 * 1.5 {
                            steer.way_asked = false;
                        } else {
                            steer.go = None;
                        }
                    }
                } else {
                    let toward = toward.norm_or(V3::ZERO);
                    if !steer.auto {
                        game.player.yaw = ease_angle(game.player.yaw, yaw_of(toward), 4.0, ticks as f32 * DT);
                    }
                    let ahead = forward(game.player.yaw, 0.0);
                    let right = ahead.cross(V3::UP).norm_or(v3(1.0, 0.0, 0.0));
                    pad.sx = toward.dot(right) * stick;
                    pad.sy = toward.dot(ahead) * stick;
                    // The grid lets a soldier over what is no taller than a knee; the player jumps it.
                    steer.still = if (at - steer.stood).flat().len2() < 0.02 * 0.02 { steer.still.saturating_add(1) } else { 0 };
                    steer.stood = at;
                    if steer.still > 5 {
                        pad.held |= pad::UP;
                    }
                }
            }
        }
        let ts = sceKernelGetSystemTimeLow();
        {
            let fx = &mut gfx.fx;
            session.run(&mut game, &pad, ticks, |game, event| {
                let (eye, yaw, pitch, _, _) = game.player.eye();
                let ahead = forward(yaw, pitch);
                let right = ahead.cross(V3::UP).norm_or(v3(1.0, 0.0, 0.0));
                fx.on_event(event, eye, ahead, right, right.cross(ahead));
                if sounding {
                    hear::sound(game, event, &mut sounds_played, |p| audio::play(p.id, p.left, p.right, p.pitch, p.priority));
                }
            });
        }
        if steer.god {
            game.player.health = 100.0;
        }
        let playing = matches!(session.mode, Mode::Play | Mode::Down);
        if playing {
            gfx.fx.step(ticks as f32 * DT);
        }
        if sounding && playing != was_playing {
            // A stopped match is silent but for the street.
            if !playing {
                audio::silence();
            }
            audio::ambience(Id::Ambience, if playing { 110 } else { 70 });
        }
        was_playing = playing;
        sim_ms = sim_ms * 0.9 + ms(ts) * 0.1;

        // -------------------------------------------------------------- the eye
        let camera = if matches!(session.mode, Mode::Title | Mode::Loading) {
            // Behind the title the eye stands where a match starts, a little above a head, and
            // looks slowly up and down the street.
            orbit += ticks as f32 * DT;
            let eye = v3(scene.start[0], scene.start[1] + 2.3, scene.start[2]);
            gfx::Camera { eye, yaw: scene.start[3] + sin(orbit * 0.11) * 0.42, pitch: 0.03 + sin(orbit * 0.07) * 0.03, roll: 0.0, fov: FOV }
        } else {
            let (eye, yaw, pitch, roll, zoom) = game.player.eye();
            gfx::Camera { eye, yaw, pitch, roll, fov: FOV * zoom }
        };
        {
            let state = &mut channel().state;
            session.publish(&game, state);
            // The statistics line twice a second, while its setting is on.
            if !session.stats {
                state.stats.clear();
            } else if frame % 15 == 0 {
                let avg = (timing.avg() * 10.0) as u32;
                let fps = 100_000 / avg.max(1);
                state.stats.clear();
                let _ = write!(state.stats, "{}.{} fps · {}.{} ms · late {} · {} draws · {}.{}k tris", fps / 10, fps % 10, avg / 10, avg % 10, timing.late, gfx.stats.draws, gfx.stats.triangles / 1000, gfx.stats.triangles % 1000 / 100);
            }
        }
        // The interface's turn, while the GE draws the previous frame: the pad as PocketJS's hosts
        // pass it (the pad's own bits, the stick packed x then y).
        let tu = sceKernelGetSystemTimeLow();
        // A frame with no room for a turn waits a frame more for its readouts.
        ui.readouts_every = if busy_ms > 27.0 { 3 } else { 2 };
        if gfx.option & 512 == 0 {
            ui.turn(ticks as f32 / 60.0, data.buttons.bits(), (data.lx as u32) << 8 | data.ly as u32, &session);
        }
        let turn_now = ms(tu);
        ui_ms = ui_ms * 0.9 + turn_now * 0.1;
        // The frame's draws are chosen while the GE draws the frame before. The levels of detail
        // come in when the frames take more than `BUSY` of this CPU and the GE, the interface's
        // turn apart: a turn is 9 ms, and a frame has 33.
        gfx.load = busy_ms / BUSY;
        gfx.figures = figures.as_ref().map_or(0, |f| f.triangles * 3 / 2);
        gfx.choose(&camera);

        // -------------------------------------------------------------- the previous frame leaves the GE
        let t1 = sceKernelGetSystemTimeLow();
        sceGuSync(GuSyncMode::Finish, GuSyncBehavior::Wait);
        let waited = ms(t1);
        gpu_ms = gpu_ms * 0.9 + waited * 0.1;
        // What the frame took of this CPU and the GE, the interface's turn apart.
        busy_ms = busy_ms * 0.8 + (ms(last_swap) - turn_now) * 0.2;
        // From the list's first word to the GE's last: what the GE took when it was waited for,
        // and no more than that when it was not.
        if frame > 0 && waited > 0.2 {
            ge_ms = ge_ms * 0.9 + ms(gfx.list_from) * 0.1;
            ge_frames += 1;
        }
        // The GE has read the vertices of the interface's last list.
        pocketjs_psp::ge::reset_pool();
        if shot_at != u32::MAX && frame == shot_at + 1 {
            store::write_shot(&gfx::pixels(gfx::DRAW_BUFFER));
        }
        // A film's frame: the list just drawn, before it is shown. The file takes longer than a
        // frame, so the match does not follow the wall clock while a film runs (`ticks` below).
        let filming = file.host && steer.film > 0 && frame > 0;
        if filming {
            steer.reel.resize(480 * 272 * 2, 0);
            gfx::read_pixels(gfx::DRAW_BUFFER, &mut steer.reel);
            store::write_frame(steer.filmed, &steer.reel);
            steer.filmed += 1;
            steer.film -= 1;
        }
        if frame >= exit_at {
            sceKernelDelayThread(300_000);
            store::note(&status);
            sceKernelExitGame();
        }
        // Present on a display refresh, `pace` of them after the last frame.
        while ((sceDisplayGetVcount().wrapping_sub(last_vcount)) & 0xffff) < pace {
            sceDisplayWaitVblankStart();
        }
        gfx::swap();
        let vcount = sceDisplayGetVcount();
        let refreshes = (vcount.wrapping_sub(last_vcount) & 0xffff) as u32;
        // A late frame catches up, by one refresh at most. A filmed frame is one thirtieth of a second.
        ticks = if filming { pace } else { refreshes.clamp(1, pace + 1) };
        last_vcount = vcount;
        let now = sceKernelGetSystemTimeLow();
        timing.push(now.wrapping_sub(last_swap) as f32 / 1000.0, refreshes > pace && !filming);
        last_swap = now;

        // -------------------------------------------------------------- this frame's list
        if let Some((camera, view)) = gfx.world() {
            let stood = !matches!(session.mode, Mode::Title | Mode::Loading);
            if let Some(figures) = figures.as_mut() {
                figures.triangles = 0;
                if stood {
                    figures.soldiers(&mut gfx, &camera, &game);
                }
            }
            gfx.mark(6);
            gfx.effects(&camera);
            gfx.mark(7);
            if let Some(figures) = figures.as_mut() {
                if stood {
                    if let Some(muzzle) = figures.held(&mut gfx, &view, &game) {
                        if game.arms.flash > 0 {
                            gfx.flash(muzzle + v3(0.0, 0.0, -0.06), 0.035 + 0.012 * game.arms.flash as f32);
                        }
                    }
                }
            }
            gfx.mark(8);
            gfx.finish(ui);
        }
        // A match nobody touches is still being watched: the console's idle timers start again.
        if frame % 120 == 0 {
            scePowerTick(PowerTick::All);
        }
        frame += 1;

        if file.host && frame % 15 == 0 {
            status.clear();
            let s = gfx.stats;
            let c = s.counts;
            let recent = ui.recent();
            let p = &game.player;
            // (a frame's worth: the steps of the last fifteen frames)
            let spent = core::mem::take(&mut game.spent).map(|us| us / 15);
            let _ = write!(
                status,
                "{{\"target\":\"psp\",\"stage\":\"running\",\"frames\":{},\"frameMs\":{:.2},\"worstMs\":{:.2},\"late\":{},\"mode\":\"{}\",\"draws\":{},\"tris\":{},\"figureTris\":{},\"clusters\":{{\"listed\":{},\"unseen\":{},\"outside\":{},\"turned\":{},\"drawn\":{},\"cut\":{},\"large\":{}}},\"cut\":{{\"tested\":{},\"near\":{},\"corners\":{},\"cut\":{}}},\"nearer\":{:.2},\"farNear\":{:.2},\"budget\":{},\"gpuMs\":{:.2},\"busyMs\":{:.2},\"geMs\":{:.2},\"geWaited\":{},\"cpuMs\":{{\"sim\":{:.2},\"select\":{:.2},\"list\":{:.2}}},\"simUs\":[{},{},{},{}],\"near\":[{},{},{}],\"selectUs\":{},\"listUs\":[{},{},{},{},{},{},{},{},{},{}],\"words\":{},\"eye\":[{:.2},{:.2},{:.2}],\"yaw\":{:.1},\"pitch\":{:.1},\"match\":{{\"health\":{:.0},\"us\":{},\"them\":{},\"clock\":{:.0},\"standing\":{},\"kills\":{},\"deaths\":{},\"shots\":{},\"hits\":{},\"weapon\":{},\"magazine\":{}}},\"sound\":{},\"memory\":{{\"freeAtStart\":{},\"freeAfterLoad\":{},\"free\":{},\"resident\":{},\"arena\":{{\"capacity\":{},\"afterLoad\":{},\"now\":{}}},\"kernelBlocks\":{}}},\"loadMs\":{},\"pack\":{},\"interface\":{{\"up\":{},\"error\":\"{}\",\"turns\":{},\"why\":[{},{},{},{},{}],\"turnMs\":{{\"script\":{:.2},\"layout\":{:.2},\"news\":{:.2},\"quiet\":{:.2},\"ticks\":{:.2},\"worst\":{:.2},\"recent\":{:.2}}},\"collections\":{},\"cpuMs\":{:.2},\"words\":{},\"bytes\":{}}}}}",
                timing.frames,
                timing.avg(),
                timing.worst(),
                timing.late,
                session.mode.name(),
                s.draws,
                s.triangles,
                figures.as_ref().map_or(0, |f| f.triangles),
                c.clusters,
                c.unseen,
                c.outside,
                c.turned,
                c.drawn,
                c.cut,
                c.large,
                s.tested,
                s.near,
                s.corners,
                s.cut,
                gfx.nearer,
                gfx.far_near,
                gfx.budget,
                gpu_ms,
                busy_ms,
                ge_ms,
                core::mem::take(&mut ge_frames),
                sim_ms,
                s.select as f32 / 1000.0,
                s.list as f32 / 1000.0,
                spent[0],
                spent[1],
                spent[2],
                spent[3],
                game.near.gathered,
                game.near.crowded,
                game.near.held(),
                s.chosen,
                s.parts[0],
                s.parts[1],
                s.parts[2],
                s.parts[3],
                s.parts[4],
                s.parts[5],
                s.parts[6],
                s.parts[7],
                s.parts[8],
                s.parts[9],
                s.words,
                camera.eye.x,
                camera.eye.y,
                camera.eye.z,
                camera.yaw * (180.0 / PI),
                camera.pitch * (180.0 / PI),
                p.health,
                game.score[0],
                game.score[1],
                game.clock,
                game.standing(),
                game.tally.kills,
                game.tally.deaths,
                game.tally.shots,
                game.tally.hits,
                game.arms.active,
                game.arms.magazine[game.arms.active],
                sounding,
                free_at_start,
                free_after,
                sceKernelTotalFreeMemSize(),
                gfx.resident_bytes,
                arena::stats().capacity_bytes,
                arena_after_load,
                arena::stats().bump_bytes,
                core::ptr::addr_of!(mem::KERNEL_BYTES).read(),
                load_ms,
                file.bytes,
                ui.up(),
                ui.error,
                ui.turns,
                ui.pace.taken[0],
                ui.pace.taken[1],
                ui.pace.taken[2],
                ui.pace.taken[3],
                ui.pace.taken[4],
                ui.script_ms,
                ui.layout_ms,
                ui.news_ms,
                ui.quiet_ms,
                ui.tick_ms,
                ui.worst_ms,
                recent[0] + recent[1],
                ui.collections,
                ui_ms,
                ui.words(),
                ui.bytes
            );
            store::publish(&status);
        }
    }
}
