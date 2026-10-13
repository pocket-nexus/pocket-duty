//! The shell around the renderer: a frame's steps, in the order the PSP's
//! loop takes them (`psp/src/main.rs`), for a tab and for the build machine
//! alike.
//!
//! The match is `duty_sim::game::Game` and the flow around it is
//! `duty_interface::Session`, as on the PSP. The interface is the PocketJS
//! guest of `ui/`, run by the page. A frame is two calls with the guest's turn
//! between them: [`App::step`] (what the interface asked for, then the match's
//! ticks), the turn (the page hands it [`App::heard`] and brings back what it
//! says to [`App::say`]), and [`App::draw`], which lays the interface's
//! picture over the street. The shell is up before the world is: until
//! [`App::enter`] the frames show the interface alone, which says how much of
//! the pack has arrived.

use duty_interface::{channel, pad, parse_f32, Command, Mode, Pace, Pad, Session};
use duty_pack::sound::{Id, Sound};
use duty_pack::{self as pack, Chunk, Cluster, Scene, ViewCell};
use duty_sim::bvh::{Bvh, Node, Tri};
use duty_sim::clip::Guard;
use duty_sim::event::Event;
use duty_sim::fx::{self, Fx, FxVertex};
use duty_sim::game::Game;
use duty_sim::level::Level;
use duty_sim::mat;
use duty_sim::math::*;
use duty_sim::mix::{self, Mixer, UNITY};
use duty_sim::tune::{WeaponDef, DT};
use duty_sim::view::{self, Draw};
use duty_sim::weapon::{self, WEAPONS};
use pocket_web_wgpu::gpu::{Gpu, Screen};
use pocket_web_wgpu::overlay::Overlay;
use pocket_web_wgpu::source::Source;
use pocket_web_wgpu::task;
use pocket_web_wgpu::wgpu::TextureFormat;
use serde_json::{json, Value};

use crate::figures::{Drawn, Figures};
use crate::pack::{kept, records, section, Progress};
use crate::render::{Look, Renderer, View};

/// Degrees the view takes top to bottom at its widest, and its near and far planes, as on the PSP.
const FOV: f32 = 65.0;
const NEAR: f32 = 0.15;
const FAR: f32 = 420.0;
/// The ground of the Pocket3D title card: what a frame shows under the interface while there is no world.
const GROUND: [f32; 3] = [0.09, 0.07, 0.15];
/// Frames whose intervals the average and the worst are taken over.
const WINDOW: usize = 240;
/// Sounds heard at once, as on the PSP.
const VOICES: usize = 16;
/// The bed's priority. A sound played stays under it, so no other sound takes the bed's voice.
const BED: u8 = u8::MAX;
/// Of the interface's turns that would bring it nothing but new readouts, one in this many is taken. The
/// PSP takes one in two or three: a turn is a quarter of its frame. Here a turn is a tenth of a millisecond.
const READOUTS_EVERY: u8 = 1;
/// The stick's travel while the sights' button is held. From the keys the stick is at rest or all the
/// way, and all the way forward is a sprint, in which the sights stay down.
const SIGHTS_STICK: f32 = 0.9;

/// PocketJS's button bits (`contracts/spec`): what a page holds of a handheld's buttons, and what the
/// interface's guest is handed.
pub mod button {
    pub const SELECT: u32 = 0x0001;
    pub const START: u32 = 0x0008;
    pub const UP: u32 = 0x0010;
    pub const RIGHT: u32 = 0x0020;
    pub const DOWN: u32 = 0x0040;
    pub const LEFT: u32 = 0x0080;
    pub const L: u32 = 0x0100;
    pub const R: u32 = 0x0200;
    pub const TRIANGLE: u32 = 0x1000;
    pub const CIRCLE: u32 = 0x2000;
    pub const CROSS: u32 = 0x4000;
    pub const SQUARE: u32 = 0x8000;
}

/// What a page holds of a handheld's controls: its buttons, and its sticks in -1…1, right and up positive.
#[derive(Clone, Copy, Debug, Default)]
pub struct Held {
    pub buttons: u32,
    pub left: [f32; 2],
    pub right: [f32; 2],
}

/// A screen and what its device's own build does around it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shape {
    pub name: &'static str,
    pub width: u32,
    pub height: u32,
    /// The interface's turns a second: what the device's own host tells its guest (`globalThis.__simHz`).
    pub turns: u32,
    /// Whether the device's display has 16-bit colours (the PSP's, with the GE's dither). A 3DS draws its
    /// upper screen into eight bits a channel.
    pub sixteen: bool,
}

/// The screens the game runs on, with the turns a second each one's host gives the interface (`TURNS` in
/// `psp/src/interface.rs`, `rate` in `n3ds/src/guest.c`). A 3DS's is its upper screen: the lower one is the
/// interface's alone, and the page draws it.
pub const SHAPES: [Shape; 2] = [Shape { name: "psp", width: 480, height: 272, turns: 30, sixteen: true }, Shape { name: "3ds", width: 400, height: 240, turns: 30, sixteen: false }];

impl Shape {
    pub fn named(name: &str) -> Option<Shape> {
        SHAPES.iter().copied().find(|s| s.name == name)
    }

    /// Sixtieths of a second in one turn of the interface.
    pub fn turn(&self) -> u32 {
        60 / self.turns.clamp(1, 60)
    }

    /// The flow's pad from the device's controls, as its own build reads them (`read_pad` in
    /// `psp/src/main.rs` and in `n3ds/src/main.c`: a 3DS's X A B Y are where a PSP's △ ○ × □ are).
    pub fn pad(&self, held: &Held) -> Pad {
        let mut out = 0;
        for (from, to) in [
            (button::UP, pad::UP),
            (button::DOWN, pad::DOWN),
            (button::LEFT, pad::LEFT),
            (button::RIGHT, pad::RIGHT),
            (button::TRIANGLE, pad::NORTH),
            (button::CIRCLE, pad::EAST),
            (button::CROSS, pad::SOUTH),
            (button::SQUARE, pad::WEST),
            (button::L, pad::L),
            (button::R, pad::R),
            (button::START, pad::START),
            (button::SELECT, pad::SELECT),
        ] {
            if held.buttons & from != 0 {
                out |= to;
            }
        }
        let travel = if held.buttons & button::L != 0 { SIGHTS_STICK } else { 1.0 };
        Pad { held: out, sx: clamp(held.left[0], -1.0, 1.0) * travel, sy: clamp(held.left[1], -1.0, 1.0) * travel }
    }
}

/// What a development host can set.
pub struct Settings {
    pub look: Look,
    /// Moves the levels' distances (1 is the pack's own), and whether every chunk the view holds is drawn.
    pub nearer: f32,
    pub everything: bool,
    /// A fixed eye: where it is, its yaw and pitch in radians, degrees of view.
    pub view: Option<(V3, f32, f32, f32)>,
    /// The player's aim and trigger are a bench's: at the nearest soldier, whenever one is in sight.
    pub auto: bool,
    /// Nothing hurts the player.
    pub god: bool,
    /// A pad to use instead of the page's, and for how many more frames.
    pub pad: (Pad, u32),
}

/// The world once the pack has been read: its geometry and pictures on the GPU, and the match in it.
pub struct World {
    renderer: Renderer,
    scene: Scene,
    chunks: Vec<Chunk>,
    clusters: Vec<Cluster>,
    larges_at: Vec<u32>,
    cells: Vec<ViewCell>,
    lists: Vec<u16>,
    bits: Vec<u8>,
    by_cell: view::Cells,
    figures: Figures,
    game: Game<'static>,
    fx: Fx,
    sounds: Vec<Sound>,
    /// The sounds' samples, signed 16-bit.
    samples: Vec<i16>,
    meta: String,
    bytes: usize,
}

impl World {
    /// The world of a whole pack: its programs for a screen of `format`.
    pub fn load(gpu: &Gpu, format: TextureFormat, bytes: &[u8]) -> Result<World, String> {
        let scene: Scene = *records::<Scene>(bytes, pack::SCEN)?.first().ok_or("the pack's scene section is empty")?;
        let chunks: Vec<Chunk> = records(bytes, pack::CHNK)?;
        let clusters: Vec<Cluster> = records(bytes, pack::CLUS)?;
        let larges_at: Vec<u32> = records(bytes, pack::LTRO)?;
        if larges_at.len() != clusters.len() {
            return Err("the pack's chunks and clusters disagree: cook the pack again".into());
        }
        let by_cell = view::Cells::new(&scene, &chunks);
        // The collision world and the walk grid live as long as the program: the match borrows them.
        let solid = Bvh { nodes: kept::<Node>(bytes, pack::COLN)?, tris: kept::<Tri>(bytes, pack::COLT)?, verts: kept::<[f32; 3]>(bytes, pack::COLV)? };
        let nav: &'static [u32] = kept::<u32>(bytes, pack::NAVG)?;
        // SAFETY: the words are the pack's NAVG section, held for good and aligned as words.
        let level = unsafe { Level::parse_nav(solid, core::slice::from_raw_parts(nav.as_ptr() as *const u8, nav.len() * 4)) }.ok_or("the pack's walk grid")?;
        let weapons: Vec<WeaponDef> = records(bytes, pack::WEAP)?;
        let defs: [WeaponDef; WEAPONS] = if weapons.len() == WEAPONS { [weapons[0], weapons[1], weapons[2]] } else { weapon::reference() };
        let figures = Figures::parse(section(bytes, pack::FIGS)?)?;
        let sounds: Vec<Sound> = records(bytes, pack::SNDS).unwrap_or_default();
        let samples: Vec<i16> = records(bytes, pack::SNDD).unwrap_or_default();
        let renderer = Renderer::new(gpu, format, bytes, &scene, &chunks, &figures)?;
        let meta = pack::section(bytes, pack::META).map(|m| String::from_utf8_lossy(m).trim_end_matches('\0').to_string()).unwrap_or_default();
        Ok(World {
            renderer,
            scene,
            chunks,
            clusters,
            larges_at,
            cells: records(bytes, pack::VCEL)?,
            lists: records(bytes, pack::VLST)?,
            bits: records(bytes, pack::VBIT)?,
            by_cell,
            figures,
            game: Game::new(level, defs, 0x5eed_1234),
            fx: Fx::new(),
            sounds: if sounds.len() == Id::ALL.len() { sounds } else { Vec::new() },
            samples,
            meta,
            bytes: bytes.len(),
        })
    }

    /// Reads the pack from `source`, saying in `progress` how much has arrived, and loads it.
    pub async fn read(gpu: Gpu, format: TextureFormat, source: Source, progress: Progress) -> Result<World, String> {
        let bytes = crate::pack::read(&source, &progress).await?;
        World::load(&gpu, format, &bytes)
    }
}

/// The sounds that play: what the PSP's output thread mixes (`psp/src/audio.rs`), mixed here when the
/// page asks for a block.
struct Sounding {
    mixer: Mixer<VOICES>,
    /// The voice the bed plays on, and its sound.
    bed: Option<(usize, u16)>,
    /// Sounds started so far: every other one of a kind is its second take.
    played: u32,
}

impl Sounding {
    fn play(&mut self, sounds: &[Sound], id: Id, left: u16, right: u16, priority: u8) {
        self.mixer.play(sounds, id as u16, left, right, UNITY, priority.min(BED - 1));
    }

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
}

/// How loud a sound at a place is in each ear, of 256: nearer is louder, and what is to the right
/// is louder on the right (`web/src/audio/spatial.js:194-198`).
fn heard(at: V3, eye: V3, yaw: f32, level: f32) -> (u16, u16) {
    let to = at - eye;
    let d = to.len();
    let gain = level * clamp(2.0 / (2.0 + 0.85 * max(0.0, d - 2.0)), 0.04, 1.0);
    let side = to.flat().norm().dot(v3(cos(yaw), 0.0, -sin(yaw)));
    ((gain * (1.0 - 0.45 * max(side, 0.0)) * 256.0) as u16, (gain * (1.0 + 0.45 * min(side, 0.0)) * 256.0) as u16)
}

/// What an event sounds like (`sound` in `psp/src/main.rs`).
fn sound(out: &mut Sounding, sounds: &[Sound], game: &Game, event: &Event) {
    let (eye, yaw) = (game.player.head(), game.player.yaw);
    out.played = out.played.wrapping_add(1);
    let count = out.played;
    let take = |a: Id, b: Id| if count & 1 == 0 { a } else { b };
    // A sound at a place, or with no place: in the player's own hands and head.
    let mut play = |id: Id, place: Option<V3>, level: f32, priority: u8| {
        let (l, r) = match place {
            Some(pos) => heard(pos, eye, yaw, level),
            None => ((level * 256.0) as u16, (level * 256.0) as u16),
        };
        out.play(sounds, id, l, r, priority);
    };
    match *event {
        Event::Shot { soldier: None, weapon, .. } => {
            let takes = [[Id::RifleShot0, Id::RifleShot1, Id::RifleShot2], [Id::SmgShot0, Id::SmgShot1, Id::SmgShot2], [Id::PistolShot0, Id::PistolShot1, Id::PistolShot2]];
            play(takes[weapon as usize % 3][(count % 3) as usize], None, 0.9, 9);
        }
        Event::Shot { soldier: Some(_), from, .. } => {
            let far = (from - eye).len() > 26.0;
            play(if far { take(Id::EnemyShotFar0, Id::EnemyShotFar1) } else { take(Id::EnemyShotNear0, Id::EnemyShotNear1) }, Some(from), 1.0, 7);
        }
        Event::Impact { pos, surface, .. } => {
            if (pos - eye).len2() < 40.0 * 40.0 {
                let id = match surface {
                    1 => take(Id::ImpactMetal0, Id::ImpactMetal1),
                    2 => take(Id::ImpactWood0, Id::ImpactWood1),
                    3 | 4 => take(Id::ImpactDirt0, Id::ImpactDirt1),
                    _ => take(Id::ImpactConcrete0, Id::ImpactConcrete1),
                };
                play(id, Some(pos), 0.8, 3);
            }
        }
        Event::Whizz { pos } => play(take(Id::Whizz0, Id::Whizz1), Some(pos), 0.9, 5),
        Event::Dry => play(Id::DryFire, None, 0.6, 6),
        Event::Reload { step: 1 } => play(Id::ReloadOut, None, 0.6, 6),
        Event::Reload { step: 2 } => play(Id::ReloadIn, None, 0.6, 6),
        Event::Reload { step: 3 } => play(Id::ReloadBolt, None, 0.6, 6),
        Event::Draw { .. } => play(Id::WeaponDraw, None, 0.5, 4),
        Event::Footstep { surface, running } => {
            let id = match surface {
                1 => take(Id::StepMetal0, Id::StepMetal1),
                2 => take(Id::StepWood0, Id::StepWood1),
                3 | 4 => take(Id::StepDirt0, Id::StepDirt1),
                _ => take(Id::StepConcrete0, Id::StepConcrete1),
            };
            play(id, None, if running { 0.5 } else { 0.32 }, 1);
        }
        Event::Land { speed, .. } => play(Id::Land, None, clamp(speed / 7.0, 0.35, 1.0) * 0.6, 3),
        Event::Struck { zone, killed, pos, .. } => {
            play(if killed { Id::Kill } else if zone == 0 { Id::Headshot } else { Id::Hitmarker }, None, 0.7, 8);
            play(take(Id::ImpactFlesh0, Id::ImpactFlesh1), Some(pos), 0.7, 4);
        }
        Event::Hurt { .. } => play(Id::Hurt, None, 0.8, 8),
        Event::Bark { kind, pos, .. } => {
            let id = match kind {
                0 => take(Id::BarkContact0, Id::BarkContact1),
                1 => Id::BarkReload,
                2 => Id::BarkHit,
                _ => take(Id::BarkDeath0, Id::BarkDeath1),
            };
            play(id, Some(pos), 1.0, 5);
        }
        Event::Fell { pos, .. } => play(Id::BodyFall, Some(pos), 0.8, 3),
        Event::Bounce { pos } => play(Id::GrenadeBounce, Some(pos), 0.9, 5),
        Event::Blast { pos } => play(Id::Explosion, Some(pos), 1.4, 10),
        _ => {}
    }
}

/// The eye of a frame.
#[derive(Clone, Copy, Debug)]
struct Camera {
    eye: V3,
    yaw: f32,
    pitch: f32,
    roll: f32,
    /// Degrees, top to bottom.
    fov: f32,
}

pub struct App {
    pub gpu: Gpu,
    pub screen: Screen,
    pub shape: Shape,
    /// A contact was on a screen at the turn last offered to the guest.
    touched: bool,
    /// The interface's picture, laid over every frame.
    pub overlay: Overlay,
    /// How much of the pack has arrived, for the loading screen.
    pub progress: Progress,
    world: Option<World>,
    pub session: Session,
    pub set: Settings,
    sounding: Sounding,
    was_playing: bool,
    pace: Pace,
    /// What the guest is owed: ticks no turn of its has taken, and the buttons held at any moment since a
    /// turn was last offered.
    guest: (u32, u32),
    /// Turns offered and not taken since the last one taken: the interface's clock is owed them.
    skipped: u32,
    /// What the interface asked to have kept, until the page has taken it.
    prefs: Option<String>,
    last: Option<f64>,
    /// Ticks that time has passed for and no frame has taken yet.
    owed: f32,
    /// Seconds the street has been looked at from behind the title.
    orbit: f32,
    camera: Camera,
    intervals: [f32; WINDOW],
    frames: u32,
    late: u32,
    began: f64,
    cpu_ms: f32,
    counts: view::Counts,
    drawn: (u32, u32),
    figure_triangles: u32,
    draws: Vec<Draw>,
    records: Vec<f32>,
    batches: Vec<Drawn>,
    squares: Vec<FxVertex>,
    /// What last went wrong with a read or a frame, for the status.
    pub trouble: String,
}

impl App {
    /// The shell on a screen, with no world yet: its frames show the interface alone. One to a program: the
    /// interface's channel is one.
    pub fn open(gpu: Gpu, screen: Screen, shape: Shape) -> App {
        let overlay = Overlay::new(&gpu, screen.format);
        let mut app = App {
            gpu,
            screen,
            shape,
            touched: false,
            overlay,
            progress: Progress::default(),
            world: None,
            session: Session::new(),
            set: Settings { look: Look::DEFAULT, nearer: 1.0, everything: false, view: None, auto: false, god: false, pad: (Pad::default(), 0) },
            sounding: Sounding { mixer: Mixer::new(), bed: None, played: 0 },
            was_playing: false,
            pace: Pace::new(),
            guest: (shape.turn(), 0),
            skipped: 0,
            prefs: None,
            last: None,
            owed: 0.0,
            orbit: 0.0,
            camera: Camera { eye: V3::ZERO, yaw: 0.0, pitch: 0.0, roll: 0.0, fov: FOV },
            intervals: [1000.0 / 60.0; WINDOW],
            frames: 0,
            late: 0,
            began: 0.0,
            cpu_ms: 0.0,
            counts: view::Counts::default(),
            drawn: (0, 0),
            figure_triangles: 0,
            draws: Vec::with_capacity(4096),
            records: Vec::new(),
            batches: Vec::new(),
            squares: vec![FxVertex::default(); fx::VERTICES],
            trouble: String::new(),
        };
        app.stage(Mode::Loading, "Reading the street");
        app.reshape(shape);
        app
    }

    /// The world has been read: the street stands behind the title.
    pub fn enter(&mut self, mut world: World) {
        let scene = world.scene;
        // The map picture's corner, as the compiler laid it out about the level's origin.
        self.session.map = (scene.level_origin[0] - 80.0, scene.level_origin[2] - 80.0);
        self.session.apply(&mut world.game);
        self.sounding.ambience(&world.sounds, Id::Ambience, 70);
        self.world = Some(world);
        unsafe { channel() }.state.message.clear();
        self.session.mode = Mode::Title;
    }

    /// Whether the world is there.
    pub fn entered(&self) -> bool {
        self.world.is_some()
    }

    /// Another screen from the next frame on.
    pub fn reshape(&mut self, shape: Shape) {
        if (self.screen.width, self.screen.height) != (shape.width, shape.height) {
            self.screen.resize(&self.gpu, shape.width, shape.height, 1);
        }
        self.shape = shape;
        // (pixels from the middle of the screen to its upper edge, over the tangent of half the view)
        self.session.focal = shape.height as f32 * 0.5 / tan(FOV * 0.5 * (PI / 180.0));
    }

    fn stage(&mut self, mode: Mode, message: &str) {
        let state = &mut unsafe { channel() }.state;
        state.mode = mode;
        if state.message != message {
            state.message.clear();
            state.message.push_str(message);
        }
    }

    /// The start failed: the interface says why.
    pub fn fail(&mut self, why: &str) {
        self.trouble = why.into();
        self.stage(Mode::Error, why);
    }

    /// Words for the flow and the renderer, as a development host sends them to the PSP (`remote` in
    /// `psp/src/main.rs`): `mode=title|play|menu` sets the flow outright, `ui=start|menu|resume|restart|
    /// title` asks what the interface's lists ask, `pad=<held>,<x>,<y>,<frames>` holds the player's pad
    /// (the flow's own bits, the stick in hundredths), `at=<x>,<y>,<z>,<yaw>,<pitch>` puts the player
    /// somewhere (degrees), `near=<soldier>,<metres>` puts him in front of a soldier, `auto=1` hands his
    /// aim and trigger to the bench, `god=1` keeps him whole, `hurt=<points>` hits him from ahead,
    /// `clock=<seconds>` sets what is left of the match, `skill=<hundredths>` sets how hard the soldiers
    /// hit. And this renderer's own: `view=<x>,<y>,<z>,<yaw>,<pitch>,<fov>` or `view=off` (a held eye,
    /// degrees), `nearer=<hundredths>` (the levels' distances), `everything=1`, and `sky= haze= shadows=
    /// effects= figures= sixteen=` (0 or 1).
    pub fn control(&mut self, words: &str) {
        for word in words.split_whitespace() {
            let Some((key, value)) = word.split_once('=') else { continue };
            let mut numbers = value.split(',').map(|n| parse_f32(n).unwrap_or(0.0));
            let mut next = || numbers.next().unwrap_or(0.0);
            let on = value != "0";
            match key {
                "sky" => self.set.look.sky = on,
                "haze" => self.set.look.haze = on,
                "shadows" => self.set.look.shadows = on,
                "effects" => self.set.look.effects = on,
                "figures" => self.set.look.figures = on,
                "sixteen" => self.set.look.sixteen = on,
                "everything" => self.set.everything = on,
                "nearer" => self.set.nearer = clamp(next() * 0.01, 0.1, 4.0),
                "view" => {
                    let v: Vec<f32> = value.split(',').filter_map(parse_f32).collect();
                    self.set.view = (v.len() >= 5).then(|| (v3(v[0], v[1], v[2]), v[3] * (PI / 180.0), v[4] * (PI / 180.0), v.get(5).copied().unwrap_or(FOV)));
                }
                "auto" => self.set.auto = on,
                "god" => self.set.god = on,
                "pad" => {
                    let pad = Pad { held: next() as u32, sx: next() * 0.01, sy: next() * 0.01 };
                    self.set.pad = (pad, next() as u32);
                }
                "ui" => unsafe { channel() }.receive(match value {
                    "start" => r#"{"type":"start"}"#,
                    "menu" => r#"{"type":"menu","on":true}"#,
                    "resume" => r#"{"type":"menu","on":false}"#,
                    "restart" => r#"{"type":"restart"}"#,
                    "title" => r#"{"type":"title"}"#,
                    _ => continue,
                }),
                _ => {}
            }
            let Some(w) = &mut self.world else { continue };
            let game = &mut w.game;
            match key {
                "mode" => {
                    if let Some(mode @ (Mode::Title | Mode::Play | Mode::Menu)) = Mode::parse(value) {
                        if mode == Mode::Play && self.session.mode == Mode::Title {
                            self.session.command(game, Command::Start);
                        }
                        self.session.mode = mode;
                    }
                }
                "at" => {
                    let (x, y, z, yaw, pitch) = (next(), next(), next(), next(), next());
                    game.player.place(v3(x, y, z), yaw * (PI / 180.0));
                    game.player.pitch = pitch * (PI / 180.0);
                    self.session.apply(game);
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

    // ---- the interface: a PocketJS guest the page runs, heard and answered through the channel

    /// A guest has opened the interface's channel: it is told the whole state on its next turn.
    pub fn interface_opened(&mut self) {
        unsafe { channel() }.open("pocket.overlay");
        // (its first turn is this frame's, and nothing held before it was there is its to hear)
        self.guest = (self.shape.turn(), 0);
        self.skipped = 0;
        self.pace = Pace::new();
        self.session.idle = false;
    }

    /// The guest went with its realm; until another opens the channel the pad keeps the flow.
    pub fn interface_closed(&mut self) {
        unsafe { channel() }.close();
    }

    /// The guest's turn in this frame, once a frame after [`App::step`]: the sixtieths of a second it is for
    /// and the buttons it is handed (PocketJS's bits, held at any moment since a turn was last offered), or
    /// `None` for a frame without one.
    ///
    /// A turn is offered `shape.turns` times a second, as the PSP's host offers it (`Ui::turn` in
    /// `psp/src/interface.rs`); it is taken when it is worth its cost (`duty_interface::Pace`), and the
    /// turns passed over are time the interface's clock is owed. No more than two are owed: a guest that
    /// fell behind does not run after the time.
    /// A contact is news of its own: a 3DS's lower screen has keys that answer it (`dt_guest_due` in
    /// `n3ds/core`).
    pub fn guest_due(&mut self, touching: bool) -> Option<(u32, u32)> {
        let turn = self.shape.turn();
        let (owed, buttons) = &mut self.guest;
        *owed = (*owed).min(2 * turn);
        if *owed < turn {
            return None;
        }
        *owed -= turn;
        let buttons = core::mem::take(buttons);
        // (while the world is read the guest takes every turn, as on the device)
        // The turn after a contact has left is the one that tells the guest so: at the title nothing
        // else would bring a turn, and the key under the finger would never be let go.
        let contact = touching || self.touched;
        self.touched = touching;
        // (the pace is asked first: it notes what it was shown)
        if self.world.is_some() && !self.pace.due(&self.session, buttons, READOUTS_EVERY) && !contact {
            self.skipped += 1;
            return None;
        }
        Some((turn * (1 + core::mem::take(&mut self.skipped).min(3)), buttons))
    }

    /// The line of state the guest has not seen, for its turn (`ui/app/protocol.ts`).
    pub fn heard(&mut self) -> Option<String> {
        unsafe { channel() }.poll()
    }

    /// A line the guest sent: a command for the next step.
    pub fn say(&mut self, line: &str) {
        unsafe { channel() }.receive(line);
    }

    /// The settings the page kept from the last visit, for the interface.
    pub fn prefs_stored(&mut self, text: &str) {
        unsafe { channel() }.state.prefs = text.into();
    }

    /// What the interface asked to have kept since the last call.
    pub fn prefs_take(&mut self) -> Option<String> {
        self.prefs.take()
    }

    // ---- a frame

    /// The first half of a frame at `now` (milliseconds on a clock that goes forward): what the interface
    /// asked for since the last frame, then the match's ticks. Returns the sixtieths of a second that passed.
    pub fn step(&mut self, now: f64, held: &Held) -> u32 {
        self.began = task::now();
        let interval = self.last.map(|last| (now - last) as f32);
        self.last = Some(now);
        let ticks = self.passed(interval.unwrap_or(1000.0 / 60.0));
        if let Some(interval) = interval {
            self.intervals[self.frames as usize % WINDOW] = interval;
            // A frame is shown for one refresh; one that took half a refresh more is late.
            self.late += (self.world.is_some() && interval > 1.5 * 1000.0 / 60.0) as u32;
        }
        self.guest.0 += ticks;
        self.guest.1 |= held.buttons;
        let Some(w) = &mut self.world else {
            let (read, of) = self.progress.get();
            if self.trouble.is_empty() {
                let message = if of > 0 { format!("Reading the street · {:.1} of {:.1} MB", read as f32 / 1e6, of as f32 / 1e6) } else { "Reading the street".into() };
                self.stage(Mode::Loading, &message);
            }
            return ticks;
        };
        let game = &mut w.game;
        // What the interface asked for on its last turn; what it wants kept comes back.
        if let Some(text) = self.session.obey(game) {
            self.prefs = Some(text);
        }
        let mut pad = self.shape.pad(held);
        if self.set.pad.1 > 0 {
            self.set.pad.1 -= 1;
            pad = self.set.pad.0;
        }
        if self.set.auto && self.session.mode == Mode::Play {
            // The bench's player: he turns to the nearest soldier in sight and holds the trigger.
            let eye = game.player.head();
            let mut best: Option<(f32, V3)> = None;
            for s in game.soldiers.iter().filter(|s| s.alive()) {
                let to = s.pos + v3(0.0, if s.crouched { 0.9 } else { 1.3 }, 0.0) - eye;
                if best.is_none_or(|b| to.len2() < b.0) && game.level.clear(eye, eye + to) {
                    best = Some((to.len2(), to));
                }
            }
            pad.held &= !(pad::R | pad::L | pad::LEFT);
            if game.arms.magazine[game.arms.active] == 0 {
                pad.held |= pad::LEFT;
            }
            if let Some((_, to)) = best {
                // He holds the sights against the weapon's climb, and fires once they are on.
                let (yaw, pitch) = (yaw_of(to) - game.player.kick[1].value(), atan2(to.y, to.flat().len()) - game.player.kick[0].value());
                game.player.yaw = ease_angle(game.player.yaw, yaw, 14.0, ticks as f32 * DT);
                game.player.pitch = ease(game.player.pitch, pitch, 14.0, ticks as f32 * DT);
                if abs(wrap_angle(game.player.yaw - yaw)) < 0.02 && abs(game.player.pitch - pitch) < 0.02 {
                    pad.held |= pad::R;
                }
            }
        }
        {
            let (fx, sounding, sounds) = (&mut w.fx, &mut self.sounding, &w.sounds);
            self.session.run(game, &pad, ticks, |game, event| {
                let (eye, yaw, pitch, _, _) = game.player.eye();
                let ahead = forward(yaw, pitch);
                let right = ahead.cross(V3::UP).norm_or(v3(1.0, 0.0, 0.0));
                fx.on_event(event, eye, ahead, right, right.cross(ahead));
                sound(sounding, sounds, game, event);
            });
        }
        if self.set.god {
            game.player.health = 100.0;
        }
        let playing = matches!(self.session.mode, Mode::Play | Mode::Down);
        if playing {
            w.fx.step(ticks as f32 * DT);
        }
        if playing != self.was_playing {
            // A stopped match is silent but for the street.
            if !playing {
                self.sounding.silence();
            }
            self.sounding.ambience(&w.sounds, Id::Ambience, if playing { 110 } else { 70 });
        }
        self.was_playing = playing;

        // The eye: behind the title it stands where a match starts, a little above a head, and looks
        // slowly up and down the street.
        self.camera = if let Some((eye, yaw, pitch, fov)) = self.set.view {
            Camera { eye, yaw, pitch, roll: 0.0, fov }
        } else if matches!(self.session.mode, Mode::Title | Mode::Loading) {
            self.orbit += ticks as f32 * DT;
            let scene = &w.scene;
            let eye = v3(scene.start[0], scene.start[1] + 2.3, scene.start[2]);
            Camera { eye, yaw: scene.start[3] + sin(self.orbit * 0.11) * 0.42, pitch: 0.03 + sin(self.orbit * 0.07) * 0.03, roll: 0.0, fov: FOV }
        } else {
            let (eye, yaw, pitch, roll, zoom) = game.player.eye();
            Camera { eye, yaw, pitch, roll, fov: FOV * zoom }
        };

        // The interface is shown the match as it now is.
        let state = &mut unsafe { channel() }.state;
        self.session.publish(game, state);
        if !self.session.stats {
            state.stats.clear();
        } else if self.frames % 30 == 0 {
            let average = self.intervals.iter().sum::<f32>() / WINDOW as f32;
            state.stats = format!("{:.1} fps · {:.1} ms · late {} · {} draws · {:.1}k tris", 1000.0 / average.max(0.1), self.cpu_ms, self.late, self.drawn.0, self.drawn.1 as f32 / 1000.0);
        }
        ticks
    }

    /// The second half: the street, with the interface over it. Before the world is there, the interface
    /// alone over the title card's ground.
    pub fn draw(&mut self) -> Result<(), String> {
        self.frames += 1;
        let Some(w) = &mut self.world else {
            let target = self.screen.frame(&self.gpu)?;
            let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
            drop(target.pass(&mut encoder, GROUND));
            self.overlay.draw(&mut encoder, &target);
            self.gpu.queue.submit([encoder.finish()]);
            target.present();
            return Ok(());
        };
        let camera = self.camera;
        let (width, height) = (self.screen.width as f32, self.screen.height as f32);
        let aspect = width / height;
        let look = forward(camera.yaw, camera.pitch);
        let projection = mat::perspective(camera.fov, aspect, NEAR, FAR);
        let seen = mat::view(camera.eye, look, camera.roll);
        let vp = mat::mul(&projection, &seen);

        // What the eye's cell sees, inside the view and not turned away, at the level of detail its
        // distance asks for: the PSP's own choice (`duty_sim::view`), with its levels' distances held.
        let planes = mat::unit_planes(&mat::planes(&vp));
        let guard = Guard::new(&vp, NEAR, width, height, 2048.0).boxes_along(&w.scene.level_x, &w.scene.level_z);
        let pack = view::World { scene: &w.scene, chunks: &w.chunks, clusters: &w.clusters, larges_at: &w.larges_at, cells: &w.cells, lists: &w.lists, bits: &w.bits };
        self.counts = view::select(&pack, &mut w.by_cell, &planes, camera.eye, &guard, &view::Options { nearer: self.set.nearer, everything: self.set.everything }, &mut self.draws);

        // The soldiers, the effects and the held weapon stand once a match has the street.
        let stood = !matches!(self.session.mode, Mode::Title | Mode::Loading) && self.set.view.is_none();
        self.records.clear();
        self.batches.clear();
        self.figure_triangles = 0;
        let (mut held, mut flash) = (None, None);
        if stood || self.set.view.is_some() {
            let renderer = &w.renderer;
            self.figure_triangles += w.figures.soldiers(&w.scene, &vp, camera.eye, look, &w.game, |x, z| renderer.sunlit(x, z), &mut self.records, &mut self.batches);
        }
        let held_from = self.batches.len();
        if stood {
            let renderer = &w.renderer;
            if let Some((frustum, muzzle, triangles)) = w.figures.held(&w.scene, &seen, aspect, &w.game, |x, z| renderer.sunlit(x, z), &mut self.records, &mut self.batches) {
                self.figure_triangles += triangles;
                held = Some(frustum);
                if w.game.arms.flash > 0 {
                    let (at, half) = (muzzle + v3(0.0, 0.0, -0.06), 0.035 + 0.012 * w.game.arms.flash as f32);
                    flash = Some([(-1.0, 1.0), (1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)].map(|(sx, sy): (f32, f32)| FxVertex { uv: [0.02 + (sx * 0.5 + 0.5) * 0.46, 0.52 + (0.5 - sy * 0.5) * 0.46], colour: [255, 220, 160, 240], pos: [at.x + sx * half, at.y + sy * half, at.z] }));
                }
            }
        }
        let right = look.cross(V3::UP).norm_or(v3(1.0, 0.0, 0.0));
        let (covering, glowing) = if stood || self.set.view.is_some() { w.fx.build(camera.eye, right, right.cross(look), |_| true, &mut self.squares) } else { (0, 0) };

        let view = View {
            world: vp,
            eye_turn: mat::mul(&projection, &mat::view(V3::ZERO, look, camera.roll)),
            eye: camera.eye,
            draws: &self.draws,
            records: &self.records,
            figures: &self.batches,
            held_from,
            held,
            effects: &self.squares,
            covering,
            glowing,
            flash,
            look: Look { sixteen: self.set.look.sixteen && self.shape.sixteen, ..self.set.look },
        };
        self.drawn = w.renderer.frame(&self.gpu, &self.screen, &self.overlay, &w.chunks, &view)?;
        self.cpu_ms += ((task::now() - self.began) as f32 - self.cpu_ms) * 0.1;
        Ok(())
    }

    /// A whole frame with no guest's turn in it.
    pub fn frame(&mut self, now: f64, held: &Held) -> Result<(), String> {
        self.step(now, held);
        self.draw()
    }

    /// Stereo sound for the page's output: `frames` frames at the mixer's own rate
    /// (`duty_sim::mix::OUTPUT_RATE` a second), left and right in turn. Silence while there is no world.
    pub fn audio(&mut self, frames: usize) -> Vec<i16> {
        let mut pcm = vec![0i16; frames * 2];
        if let Some(w) = &self.world {
            // SAFETY: 16-bit samples read as the bytes the mixer takes, which start on an even address.
            let samples = unsafe { core::slice::from_raw_parts(w.samples.as_ptr() as *const u8, w.samples.len() * 2) };
            self.sounding.mixer.render(&w.sounds, samples, &mut pcm);
        }
        pcm
    }

    /// Ticks of the match (sixtieths of a second) for a frame that comes `interval` milliseconds after the
    /// last. The PSP's display refreshes 60 times a second and a frame there is a whole number of ticks;
    /// so it is here when the time between two frames is within a twentieth of a whole number of them (a
    /// display of 59.94 a second, or of 120 with every second refresh drawn), and a late frame catches up.
    /// On any other display the part of a tick left over is owed to the next frame, so the match keeps
    /// its speed.
    fn passed(&mut self, interval: f32) -> u32 {
        // (a clock that went back, as after a measurement made ahead of it, is one frame's time)
        let interval = if interval > 0.0 { interval } else { 1000.0 / 60.0 };
        let passed = interval.min(100.0) * 0.06;
        let whole = (passed + 0.5).floor();
        if whole >= 1.0 && (passed - whole).abs() <= 0.05 * whole {
            self.owed = 0.0;
            return whole.min(3.0) as u32;
        }
        self.owed += passed;
        let ticks = self.owed.floor().min(3.0);
        self.owed = (self.owed - ticks).min(1.0);
        ticks as u32
    }

    /// The run as a JSON object.
    pub fn status(&self) -> String {
        let average = self.intervals.iter().sum::<f32>() / WINDOW as f32;
        let worst = self.intervals.iter().fold(0.0f32, |a, &b| a.max(b));
        let mut status = json!({
            "target": "web",
            "stage": if self.world.is_some() { "running" } else if self.trouble.is_empty() { "loading" } else { "error" },
            "mode": self.session.mode.name(),
            "shape": {"name": self.shape.name, "width": self.screen.width, "height": self.screen.height, "turns": self.shape.turns},
            "interface": {"open": unsafe { channel() }.is_open(), "turns": self.pace.taken},
            "frames": self.frames, "frameMs": average, "worstMs": worst, "late": self.late, "cpuMs": self.cpu_ms,
            "settings": {"auto": self.set.auto, "god": self.set.god, "nearer": self.set.nearer, "sixteen": self.set.look.sixteen},
            "adapter": self.gpu.adapter,
            "trouble": self.trouble,
        });
        if let Some(w) = &self.world {
            let (game, c) = (&w.game, &self.counts);
            let p = &game.player;
            status["pack"] = json!({"bytes": w.bytes, "id": serde_json::from_str::<Value>(&w.meta).unwrap_or(Value::String(w.meta.clone()))});
            status["draws"] = json!(self.drawn.0);
            status["tris"] = json!(self.drawn.1);
            status["figureTris"] = json!(self.figure_triangles);
            status["clusters"] = json!({"listed": c.clusters, "unseen": c.unseen, "outside": c.outside, "turned": c.turned, "drawn": c.drawn});
            status["chunks"] = json!({"listed": c.listed, "seen": c.chunks});
            status["gpuBytes"] = json!(w.renderer.bytes);
            status["eye"] = json!([self.camera.eye.x, self.camera.eye.y, self.camera.eye.z]);
            status["yaw"] = json!(self.camera.yaw * (180.0 / PI));
            status["pitch"] = json!(self.camera.pitch * (180.0 / PI));
            status["player"] = json!({"pos": [p.body.pos.x, p.body.pos.y, p.body.pos.z], "yaw": p.yaw, "pitch": p.pitch, "speed": p.speed(), "sprinting": p.sprinting, "crouched": p.crouched, "sights": game.arms.ads});
            status["match"] = json!({
                "health": p.health, "us": game.score[0], "them": game.score[1], "clock": game.clock, "standing": game.standing(), "tick": game.tick,
                "kills": game.tally.kills, "deaths": game.tally.deaths, "shots": game.tally.shots, "hits": game.tally.hits,
                "weapon": game.arms.active, "magazine": game.arms.magazine[game.arms.active], "reserve": game.arms.reserve[game.arms.active],
            });
            status["soldiers"] = json!(game.soldiers.iter().enumerate().filter(|(_, s)| s.used).map(|(number, s)| json!({"number": number, "pos": [s.pos.x, s.pos.y, s.pos.z], "alive": s.alive(), "variant": s.variant})).collect::<Vec<_>>());
            status["sound"] = json!({"sounds": w.sounds.len(), "played": self.sounding.played, "rate": mix::OUTPUT_RATE});
        }
        status.to_string()
    }
}
