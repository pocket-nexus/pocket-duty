//! The interface: one PocketJS guest (`ui/`, shared with the other devices)
//! drawn over the street through PocketJS's Vita host library. It owns every
//! 2D pixel, and what the buttons and the panel mean while a list is up; this
//! side owns the scene, shows it the match's state and does what it asks
//! (`crates/duty-interface`).
//!
//! The guest is laid out for 480 × 272 and drawn at twice that, the PSP's own
//! layout on the Vita's panel. It is offered a turn 30 times a second, every
//! second frame, and takes the ones worth their cost (`duty_interface::Pace`).

use std::collections::VecDeque;
use std::time::Instant;

use duty_interface::{channel, guest, Pace, Session};
use libquickjs_sys::{JSContext, JSRuntime, JS_RunGC, JS_SetGCThreshold};
use pocketjs_vita::{graphics, input, Runtime};

// libquickjs-sys omits this one; the linked QuickJS provides it.
extern "C" {
    fn JS_GetRuntime(ctx: *mut JSContext) -> *mut JSRuntime;
}

/// The guest's turns a second; it is told so before it mounts. The UI core counts sixtieths, so a turn is
/// this many of its ticks.
const TURNS: u32 = 30;
const TICKS: u32 = 60 / TURNS;
/// Seconds between two turns.
pub const TURN: f32 = 1.0 / TURNS as f32;
const RATE: &[u8] = b"globalThis.__simHz=30;";
/// The compiled interface (`bun tools/ui.ts vita`).
const SCRIPT: &str = "duty.js";
const PAK: &str = "duty.pak";

/// A pad nobody touches.
pub const NEUTRAL: input::Pad = input::Pad { buttons: 0, lx: 128, ly: 128, rx: 128, ry: 128 };

pub struct Ui {
    runtime: Option<Runtime>,
    /// Seconds the guest is owed.
    owed: f32,
    /// Buttons seen since the last turn: a press shorter than a turn still reaches the guest.
    latched: u32,
    /// Which of the turns it is offered are taken.
    pub pace: Pace,
    /// Of the turns that would bring it nothing but new readouts, one in this many is taken.
    pub readouts_every: u8,
    /// Turns offered and not taken since the last one taken: the core's clock is owed them.
    skipped: u32,
    /// A contact was on the panel at the turn last offered.
    touched: bool,
    /// Buttons a control message presses, each held two turns and let go for one.
    presses: VecDeque<(u32, u8)>,
    /// Places a control message taps, in the interface's pixels: a finger down for two turns, up for one.
    taps: VecDeque<((u16, u16), u8)>,
    /// Why there is no interface, for the status report.
    pub error: String,
    /// Turns taken, and the milliseconds of one: smoothed over the last ones, and the longest.
    pub turns: u32,
    pub turn_ms: f32,
    pub worst_ms: f32,
    /// Milliseconds the guest's last collection took, and how many there were.
    pub collect_ms: f32,
    pub collections: u32,
}

impl Ui {
    /// Boots the guest from `duty.js` and `duty.pak` (the USB share in a development build, else the package,
    /// else the data folder).
    ///
    /// # Safety
    /// Render thread, after vita2d is up and outside any scene.
    pub unsafe fn boot() -> Self {
        let read = |name: &str| crate::paths::candidates(name).iter().find_map(|p| crate::hostfs::read(p, 16 << 20)).ok_or(format!("{name} is missing"));
        let boot = || -> Result<Runtime, String> {
            // The guest borrows the pak for as long as it lives.
            let pak: &'static [u8] = Box::leak(read(PAK)?.into_boxed_slice());
            let mut script = RATE.to_vec();
            script.extend(read(SCRIPT)?);
            script.push(0);
            let script = String::from_utf8(script).map_err(|e| e.to_string())?;
            let mut runtime = Runtime::new(pak)?;
            // The channel to this renderer takes the place of the host's wire.
            guest::mount(runtime.context(), runtime.global());
            runtime.eval(&script)?;
            // The guest's collector does not start by itself: a collection walks the whole heap, some 35 ms on
            // this console, and one that starts in the middle of a turn is two late frames. Reference counts
            // free what a turn leaves; what is left in cycles waits for `collect`.
            JS_SetGCThreshold(JS_GetRuntime(runtime.context()), usize::MAX as _);
            Ok(runtime)
        };
        let mut ui = Self {
            runtime: None,
            owed: TURN,
            latched: 0,
            pace: Pace::new(),
            readouts_every: 1,
            skipped: 0,
            touched: false,
            presses: VecDeque::new(),
            taps: VecDeque::new(),
            error: String::new(),
            turns: 0,
            turn_ms: 0.0,
            worst_ms: 0.0,
            collect_ms: 0.0,
            collections: 0,
        };
        match boot() {
            Ok(runtime) => ui.runtime = Some(runtime),
            Err(error) => ui.fail(error),
        }
        ui
    }

    /// The guest is on the screen.
    pub fn up(&self) -> bool {
        self.runtime.is_some()
    }

    /// Without a guest the channel is shut, and the session keeps the flow on the pad.
    unsafe fn fail(&mut self, error: String) {
        pocketjs_vita::vita_log(format_args!("duty: interface: {error}"));
        self.error = error;
        self.runtime = None;
        channel().close();
    }

    /// Puts a picture (a PocketJS image entry) in the interface's core: its texture there, or -1.
    ///
    /// # Safety
    /// Render thread, outside any scene.
    pub unsafe fn picture(&mut self, entry: &[u8]) -> i32 {
        let Some(runtime) = &mut self.runtime else { return -1 };
        let ui = runtime.ui();
        let handle = ui.upload_img_entry(entry);
        if handle >= 0 {
            graphics::register_texture(ui, handle);
        }
        handle
    }

    /// Gives back a picture `picture` put there.
    pub fn forget(&mut self, handle: i32) {
        if self.runtime.is_some() && handle >= 0 {
            graphics::free_texture(handle);
        }
    }

    /// Collects the guest's garbage now. The renderer calls it where the screen changes whole (a map has
    /// been read, the title comes back).
    ///
    /// # Safety
    /// Render thread, outside a turn.
    pub unsafe fn collect(&mut self) {
        let Some(runtime) = &self.runtime else { return };
        let start = Instant::now();
        JS_RunGC(JS_GetRuntime(runtime.context()));
        self.collect_ms = start.elapsed().as_secs_f32() * 1000.0;
        self.collections += 1;
    }

    /// Presses `buttons` on the interface as a thumb would.
    pub fn press(&mut self, buttons: u32) {
        self.presses.push_back((buttons, 3));
    }

    /// Taps the panel at a place of the interface (480 × 272) as a finger would.
    pub fn tap(&mut self, x: u16, y: u16) {
        self.taps.push_back(((x, y), 3));
    }

    /// The guest's turn when `dt` more seconds make one due: the pad and the panel go in, the state is read
    /// and commands are left on the channel. `quiet` withholds the panel (another layer has the screen).
    /// With a `session` the turn is taken only when it is worth its cost (`Pace`); without one (a map is
    /// being read) always.
    ///
    /// # Safety
    /// Render thread, outside any scene.
    pub unsafe fn turn(&mut self, dt: f32, pad: &input::Pad, quiet: bool, session: Option<&Session>) {
        self.latched |= pad.buttons;
        self.owed = (self.owed + dt).min(2.0 * TURN);
        if self.owed < TURN {
            return;
        }
        self.owed -= TURN;
        let mut touches = if quiet { input::TouchSnapshot::EMPTY } else { input::read_touches() };
        // The turn after a contact has left is the one that tells the guest so.
        let touching = !touches.packed().is_empty();
        let contact = touching || self.touched;
        self.touched = touching;
        if let Some(session) = session {
            // (the pace is asked first: it notes what it was shown)
            if !self.pace.due(session, self.latched, self.readouts_every) && !contact && self.presses.is_empty() && self.taps.is_empty() {
                self.latched = 0;
                self.skipped += 1;
                return;
            }
        }
        let Some(runtime) = &mut self.runtime else { return };
        let start = Instant::now();
        if let Some(((x, y), turns)) = self.taps.front_mut() {
            *turns -= 1;
            if *turns > 0 {
                touches = input::TouchSnapshot::from_logical(&[(1, *x, *y)]);
            } else {
                self.taps.pop_front();
            }
        }
        let mut buttons = core::mem::take(&mut self.latched);
        if let Some((pressed, turns)) = self.presses.front_mut() {
            *turns -= 1;
            if *turns > 0 {
                buttons |= *pressed;
            } else {
                self.presses.pop_front();
            }
        }
        if let Err(error) = runtime.frame_with_input(buttons as i32, pad.left_analog(), &touches) {
            self.fail(error);
            return;
        }
        // The turns passed over were time too: what fades keeps its pace.
        for _ in 0..TICKS * (1 + core::mem::take(&mut self.skipped).min(3)) {
            runtime.tick();
        }
        let ms = start.elapsed().as_secs_f32() * 1000.0;
        self.turns += 1;
        // The first turns mount the screens; the figures are for the ones after.
        if self.turns > 8 {
            self.turn_ms += (ms - self.turn_ms) * 0.1;
            self.worst_ms = self.worst_ms.max(ms);
        }
    }

    /// Draws the interface into the open vita2d scene, over what is there.
    ///
    /// # Safety
    /// Render thread, inside the display scene.
    pub unsafe fn draw(&mut self) {
        if let Some(runtime) = &mut self.runtime {
            runtime.render_over();
        }
    }
}
