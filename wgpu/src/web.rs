//! The tab's side: what the page calls (`page/main.js`).
//!
//! The page plays the Pocket3D title card and opens the shell on a canvas
//! ([`Duty::open`]); a map's pack is read beside the frames ([`Reader::read`]) and
//! handed in when it is there ([`Duty::enter`]). When the interface asks for a
//! match on another map the shell lets its world go and names the map
//! ([`Duty::map_wanted`]): the page reads that map's pack and hands it in the
//! same way. A frame is `step`, the turn of
//! the interface's guest, which the page runs in a realm of its own and whose
//! lines pass through `heard` and `say`, then `draw`. When what the guest
//! shows has changed, the page hands its picture to `overlay`. Everything a
//! frame does is in `app`.

use pocket_web_wgpu::gpu::{Gpu, Screen};
use pocket_web_wgpu::source::Source;
use pocket_web_wgpu::wgpu::TextureFormat;
use wasm_bindgen::prelude::*;

use crate::app::{self, App, Held, Shape, SHAPES};
use duty_interface::Chart;
use crate::pack::Progress;

fn describe(s: &Shape) -> String {
    format!("{{\"name\":\"{}\",\"width\":{},\"height\":{},\"turns\":{}}}", s.name, s.width, s.height, s.turns)
}

/// The screens the page can ask for, as a JSON array.
#[wasm_bindgen]
pub fn shapes() -> String {
    format!("[{}]", SHAPES.iter().map(describe).collect::<Vec<_>>().join(","))
}

/// The maps, in the list's order (`duty_interface::maps`), as a JSON array of `{"id", "title"}`. The first is the one
/// every page has a pack for.
#[wasm_bindgen]
pub fn maps() -> String {
    format!("[{}]", duty_interface::maps::MAPS.iter().map(|m| format!("{{\"id\":\"{}\",\"title\":\"{}\"}}", m.id, m.title)).collect::<Vec<_>>().join(","))
}

/// Frames a second the sound's blocks are made for.
#[wasm_bindgen]
pub fn sound_rate() -> u32 {
    duty_sim::mix::OUTPUT_RATE
}

#[wasm_bindgen]
pub struct Duty {
    app: App,
    /// The buttons of the guest's turn in this frame.
    handed: u32,
}

/// What reads the pack beside the frames: the GPU the shell draws with, and the screen its programs are for.
#[wasm_bindgen]
pub struct Reader {
    gpu: Gpu,
    format: TextureFormat,
    progress: Progress,
}

/// The world once the pack has been read.
#[wasm_bindgen]
pub struct World {
    world: app::World,
    /// The map from above, when the manifest has it beside the map's pack.
    chart: Option<Chart>,
}

/// The pack `<map>-map` beside a map's own (`duty_interface::Chart`, tools/packs.ts). A tab's interface
/// runs in a realm this module cannot reach into: the picture is told to it in the state, as text,
/// and it puts it in its core itself.
async fn read_chart(place: &str) -> Option<Chart> {
    let bytes = Source::open(place).await.ok()?.all().await.ok()?;
    let (mut chart, entry) = Chart::parse(&bytes)?;
    chart.entry = duty_interface::base64(entry);
    Some(chart)
}

#[wasm_bindgen]
impl Reader {
    /// Reads the pack at `url`: the pack's file, on a server that answers byte ranges, or the manifest
    /// of a game's packs in pieces with the map after `#` (`.packs#yards`; `pocket_web_wgpu::source`).
    pub async fn read(self, url: String) -> Result<World, JsError> {
        let source = Source::open(&url).await.map_err(|e| JsError::new(&e))?;
        let world = app::World::read(self.gpu, self.format, source, self.progress).await.map_err(|e| JsError::new(&e))?;
        // A pack named in a manifest has its picture under the same manifest, as `<map>-map`. A pack served
        // as a file (the development server) has it beside the file, under the same name with `.chart`.
        let chart = match url.strip_suffix(".pack") {
            _ if url.contains(".packs#") => read_chart(&format!("{url}-map")).await,
            Some(stem) => read_chart(&format!("{stem}.chart")).await,
            None => None,
        };
        Ok(World { world, chart })
    }
}

#[wasm_bindgen]
impl Duty {
    /// The shell on `canvas`, which has the shape's size in pixels, with no world yet: its frames show the
    /// interface alone. `prefs`: the settings the page kept from the last visit. One to a page.
    pub async fn open(canvas: web_sys::HtmlCanvasElement, shape: String, prefs: String) -> Result<Duty, JsError> {
        std::panic::set_hook(Box::new(|info| web_sys::console::error_1(&info.to_string().into())));
        let shape = Shape::named(&shape).ok_or_else(|| JsError::new("no such shape"))?;
        let (gpu, surface) = Gpu::for_canvas(canvas).await.map_err(|e| JsError::new(&e))?;
        // (the street has a target of its own; the canvas takes the frame in the display's colours)
        let screen = Screen::canvas(&gpu, surface, shape.width, shape.height, 1);
        let mut app = App::open(gpu, screen, shape);
        if !prefs.is_empty() {
            app.prefs_stored(&prefs);
        }
        Ok(Duty { app, handed: 0 })
    }

    /// What reads the pack for this shell.
    pub fn reader(&self) -> Reader {
        Reader { gpu: self.app.gpu.clone(), format: self.app.screen.format, progress: self.app.progress.clone() }
    }

    /// The maps the page has packs for, as ids with commas between: what the interface's map list offers.
    pub fn maps(&mut self, ids: &str) {
        self.app.maps(ids);
    }

    /// The map the page reads first, by its id.
    pub fn reads(&mut self, map: &str) {
        self.app.reads(map);
    }

    /// A map the interface asked for a match on: the page reads that map's pack and calls `enter`. Once
    /// for each asking; until `enter` the shell holds no world and shows the loading screen.
    pub fn map_wanted(&mut self) -> Option<String> {
        self.app.map_wanted().map(String::from)
    }

    /// The pack has been read: its map stands behind the title, or, when the interface asked for the map,
    /// a match starts on it.
    pub fn enter(&mut self, world: World) {
        self.app.enter(world.world);
        self.app.chart(world.chart);
    }

    /// Whether the world is there.
    pub fn entered(&self) -> bool {
        self.app.entered()
    }

    /// The start failed: the interface says why.
    pub fn fail(&mut self, why: &str) {
        self.app.fail(why);
    }

    /// The first half of a frame at `now` (the frame loop's clock, milliseconds): what the interface asked
    /// for, then the match. `buttons`: PocketJS's bits of the buttons held on the page's handheld; the
    /// stick in -1…1, right and up positive. Returns the sixtieths of a second that passed.
    pub fn step(&mut self, now: f64, buttons: u32, lx: f32, ly: f32) -> u32 {
        self.app.step(now, &Held { buttons, left: [lx, ly], right: [0.0, 0.0] })
    }

    /// The second half: the street, with the interface's picture over it.
    pub fn draw(&mut self) -> Result<(), JsError> {
        self.app.draw().map_err(|e| {
            self.app.trouble = e.clone();
            JsError::new(&e)
        })
    }

    /// A guest has opened the interface's channel; one that replaces another opens it again.
    pub fn interface_opened(&mut self) {
        self.app.interface_opened();
    }

    /// The guest went with its realm.
    pub fn interface_closed(&mut self) {
        self.app.interface_closed();
    }

    /// The guest's turn in this frame, once a frame after `step`: the sixtieths of a second it is for, or 0
    /// for a frame without one. `touching`: a contact is on a screen the guest draws, or has just left one.
    pub fn guest_due(&mut self, touching: bool) -> u32 {
        let (ticks, buttons) = self.app.guest_due(touching).unwrap_or((0, 0));
        self.handed = buttons;
        ticks
    }

    /// The buttons for the turn `guest_due` has just allowed: PocketJS's bits, held at any moment since a
    /// turn was last offered.
    pub fn guest_buttons(&self) -> u32 {
        self.handed
    }

    /// The line of state the guest has not seen, for its turn.
    pub fn heard(&mut self) -> Option<String> {
        self.app.heard()
    }

    /// A line the guest sent.
    pub fn say(&mut self, line: &str) {
        self.app.say(line);
    }

    /// The interface's picture as PocketJS's UI core rasterizes it with its alpha (`width` by `height` rows
    /// of premultiplied RGBA): it is laid over every frame from the next on.
    pub fn overlay(&mut self, pixels: &[u8], width: u32, height: u32) -> Result<(), JsError> {
        self.app.overlay.write(&self.app.gpu, pixels, width, height).map_err(|e| JsError::new(&e))
    }

    /// Nothing is laid over the frames until a picture is handed in again: the guest is being replaced.
    pub fn overlay_hide(&mut self) {
        self.app.overlay.hide();
    }

    /// What the interface asked to have kept since the last call (the settings, as JSON text).
    pub fn prefs_take(&mut self) -> Option<String> {
        self.app.prefs_take()
    }

    /// Another screen from the next frame on. The canvas has the new size already. `name` is one of
    /// `shapes()`; a size that is not zero replaces that shape's.
    pub fn reshape(&mut self, name: &str, width: u32, height: u32) -> Result<String, JsError> {
        let mut shape = Shape::named(name).ok_or_else(|| JsError::new("no such shape"))?;
        if width != 0 && height != 0 {
            (shape.width, shape.height) = (width, height);
        }
        self.app.reshape(shape);
        Ok(describe(&self.app.shape))
    }

    /// Words for the flow and the renderer, as a development host sends them (`App::control`).
    pub fn control(&mut self, words: &str) {
        self.app.control(words);
    }

    /// Stereo sound for the page's output: `frames` frames at `sound_rate()` a second, left and right in turn.
    pub fn audio(&mut self, frames: usize) -> Vec<i16> {
        self.app.audio(frames)
    }

    /// The run as a JSON object.
    pub fn status(&self) -> String {
        self.app.status()
    }
}
