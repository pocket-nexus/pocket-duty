# Pocket Duty in a browser tab

The game of the PSP build, drawn with [wgpu](https://wgpu.rs) 25: over WebGPU in a tab (wasm32), and over Metal on the machine that builds it, where frames go to files. It reads **the PSP's pack** (`profiles/psp30.json`) as it is: the same triangles, baked light, pictures, soldiers, sounds and walk grid. The match is `crates/duty-sim` and the flow is `crates/duty-interface`, as the PSP links them. The game's own interface (`ui/`, the bundle the PSP loads) runs over the street, and the page shows the game in a PSP's shell with its keys as the controls. `web/` is something else: the reference, Claude of Duty, which the export page runs once.

| Part | What it is |
| --- | --- |
| `src/app.rs` | The shell: a frame as `step`, the guest's turn, `draw`; the flow (`Session`), the pad, the control words, the sounds the match's events start. |
| `src/render.rs`, `src/shaders/` | The renderer: what the GE draws from the pack with no program (`psp/src/gfx.rs`), as WGSL programs, and the passes of a frame. |
| `src/figures.rs` | The soldiers and the held weapon posed from their clips, as `psp/src/figures.rs` poses them: one record a batch of four bones. |
| `src/pack.rs` | The pack read over HTTP with its progress, its sections as records, and a picture's palette indices as texels. |
| `src/web.rs`, `page/` | The tab: what the page calls, and the page itself. |
| `src/bin/shot.rs` | Frames on this machine's GPU: one as a PNG, or a run as rows of RGBA for an encoder; two pictures compared. |
| `vendor/pocketjs/devices/web/pocket-web-wgpu` | PocketJS's browser kernel: what is not this game's. The device and the screens, the overlay pass, ranges of a pack, and the page itself: the title card first and the frame loop, the interface's guest in its realm, and **the Pocket3D player** (the bar, the PSP's shell with its keys as the controls, the dock with Download and Remix). |
| `tools/wgpu.ts` | build, serve, dist, shot, check. |

```
bun tools/duty.ts cook               # the PSP's pack → .pocket-build/duty/psp30/duty.pack
bun tools/wgpu.ts serve              # build when there is no site, then http://127.0.0.1:8802/
bun tools/wgpu.ts shot --frames 600 --words "mode=play auto=1" --out a.png
bun tools/wgpu.ts check              # Chrome: from the title through a match to its results, by keys and pointer
bun tools/wgpu.ts dist               # the directory a static host serves
bun tools/wgpu.ts check --dist       # the same check of that directory
cargo test --manifest-path wgpu/Cargo.toml
```

The build needs the `wasm32-unknown-unknown` target and `wasm-bindgen` 0.2.126 on the path (the version in `Cargo.lock`; `build` refuses another). It compiles the interface for the PSP as `bun tools/ui.ts psp` does, so it needs what that needs: `bun install` in `vendor/pocketjs`, and the cook's `map.png` beside the pack. The kernel's page modules, the title card, the realm and PocketJS's UI core are staged by PocketJS's own tool (`stagePocket3dWeb` of `vendor/pocketjs/tools/pocket3d-web.ts`).

## The launch

The page plays the Pocket3D title card first (`playTitle()` of PocketJS's `pocket3d-title`, copied into the site as it is), over the whole page. While it plays the page opens the renderer on the canvas, starts the interface's guest and reads the pack. When the card has ended the canvas is shown. While the pack is on its way the frames show the interface alone, which says `Reading the street · 8.4 of 30.5 MB` (`Mode::Loading`), as the PSP's loading steps do; a start that fails is said the same way. A browser without WebGPU is told so in one sentence after the card; there is no other renderer.

## The pack over HTTP

**The first frame of the street needs the whole pack, 30.5 MB**: every section is read before the match is made, as the PSP reads it. Reads are `Source::range(offset, size)` of the kernel, 8 MiB at a time, so the loading screen counts.

The source has two forms:

- **The pack's file**, on a server that answers byte ranges. `serve` does this.
- **The pack cut into pieces of one size** with a manifest that lists them (`<meta name="pocket-pack">` names a `.json`). A read fetches the pieces it lies in, whole and with plain requests, all at once. `dist` writes this form.

**A page has a pack a map** (`maps/maps.json`): `<meta name="pocket-pack">` names the first map's and `<meta name="pocket-packs">` the others', as `<map>=<pack>` with spaces between; `serve` and `dist` write both for every map that is cooked, and `?map=<id>` opens the page on one. The shell holds one world. When the interface asks for a match on another map, `App::leave` drops the world (the collision world and the walk grid the match borrowed go with it, `pack::Keep`), the loading screen names the map, `Duty::map_wanted` hands its id to the page, the page reads that pack and `Duty::enter` starts the match.

`bun tools/wgpu.ts dist` writes `.pocket-build/wgpu/dist` for a host that limits a file to 32 MiB and keeps a file ten minutes in a browser's cache (Pocket Studio's site deployments): `index.html` and `icon.png`; everything else of the site under `app/<build>/`, named by a hash of its contents; `pack/<hash>.json` and the pieces, each named by its own hash. The page alone is asked for again at every visit. With pieces of 2 MiB the directory is **53 files and 33.1 MB**: 15 pieces and their manifest (30.5 MB), 35 files of the site (2.6 MB), the page and the icon. `dist` refuses a directory the host would (a file over 32 MiB, more than 4 000 files or 1 GiB, a top-level `play/` or `runtime/`). A checkout that `pocket-studio register` has linked (`.pocket-studio.json`, which Git ignores) gets the project's id and the Studio's origin written into the page (`<meta name="pocket-app">`, `<meta name="pocket-studio">`), for a host that answers no `/app.json`. The game's host on Pocket Studio answers one, and **the player's dock takes its links from it: Download to `/games/duty?get`, Remix to `/games/duty?remix`**. `pocket-studio site .pocket-build/wgpu/dist` uploads the directory.

## The interface

The interface is the game's own: `ui/`, compiled for the PSP by `tools/ui.ts`, the bundle and its pak as the PSP loads them. The page runs it as a guest in a realm of its own: PocketJS's AppInstance on the UI core built for wasm, started without the text worker. The guest opens the service it opens on the PSP, `pocket.overlay`, and the lines are carried in the page:

1. `step`: what the guest asked for on its last turn (`start`, `menu`, `restart`, `title`, `option`, `prefs`, `idle`, `wake`) goes to `Session::obey`; then the match's ticks; then `Session::publish` writes the state the interface is shown.
2. The guest's turn. **A turn is offered 30 times a second, as the PSP's host offers one** (`TURNS` in `psp/src/interface.rs`; the realm is told that rate before the bundle runs, `simHz`). A turn is taken on the frames `duty_interface::Pace` says it is worth one: the line of state it has not seen goes in (`heard`), with the buttons held at any moment since a turn was last offered, and what it sends comes back (`say`). The turns passed over are time the interface's clock is owed, as on the PSP.
3. When the guest's draw hash has changed, its picture is drawn again and handed to the renderer.
4. `draw`: the street, then the picture over it in a pass of its own, premultiplied.

What the interface keeps between visits (the settings, `prefs`) is in the browser's `localStorage` under `pocket-duty.interface`, where a PSP has a file beside the pack.

## The controls

The page's handhelds are the PSP and the Nintendo 3DS (`?device=3ds`, or the bar), and what their buttons do is the game's: a 3DS's X A B Y are where a PSP's △ ○ × □ are, and its lower screen is the interface's second surface, drawn on a canvas of its own (320 × 240) with the level from above and four keys that take the pointer or a finger. The turn after a contact leaves a screen is always taken, so the key under it is let go at the title too, where nothing else brings a turn (`App::guest_due`). What the PSP's buttons do is the game's (`Shape::pad`, from `read_pad` in `psp/src/main.rs`). The kernel reads the keyboard and the shell's own keys under a pointer or a finger:

| | Keys |
| --- | --- |
| Walk: the stick. All the way forward runs | W A S D |
| Look: △ ○ ✕ □ | I L K J (or V Z X C) |
| Fire: R | E |
| Raise the sights: L | Q |
| Jump and climb, crouch and slide, change the magazine, next weapon: the direction pad | the arrows: up, down, left, right |
| Fire mode: SELECT | Shift |
| Menu: START | Space (or Escape) |
| Lists: move, choose, back | the arrows, Z (or Enter), X (or Backspace) |

**From the keys the stick is at rest or all the way, and all the way forward is a sprint, in which the sights stay down.** While L is held the stick's travel ends at nine tenths (`SIGHTS_STICK` in `src/app.rs`): the player walks and the sights come up. A thumb on a PSP eases the stick itself.

**Sound** is the simulation crate's mixer (`duty_sim::mix`), which the PSP runs on a thread of its own: 16 voices at 22 050 frames a second, started by the match's events with the PSP's gains and priorities. It starts on the first key or pointer (a browser asks for a gesture): the page opens its output at the mixer's rate and pulls blocks of 512 frames through a `ScriptProcessorNode`. A stopped match is silent but for the street's bed, as on the PSP.

## The frame

1. `step`: what the interface asked for, then one tick of the match per sixtieth of a second since the last frame, with the pad the page's buttons make. Each event of a tick starts its effect and its sound.
2. The guest's turn and, when what it shows has changed, its picture.
3. The eye (the player's, or behind the title a slow look up and down the street from where a match starts), and **the PSP's own choice of what to draw** (`duty_sim::view::select`): the clusters the eye's cell can see, inside the view and not all turned away, at the level of detail their distance asks.
4. One pass into a target of 480 × 272: the sky's band of pictures and the zenith's colour over it, with no depth; the street, a texel times the corner's baked colour, doubled, under the haze; the ground once more under its shadow picture, multiplied in where the depth is the ground's own; the glass, the farthest chunk first; the soldiers, lit by the sky and by the sun where the shadow picture has sun at their feet; the effects' squares; the held weapon in the nearest sixteenth of the depth range, and the flash at its muzzle.
5. One pass onto the canvas: the target in **the PSP display's colours, five bits of red, six of green and five of blue, with the GE's ordered dither** (`shaders/post.wgsl`). Then the interface over it.

## What differs from the PSP's picture

- **One depth range of 24 bits.** The GE's depth is 16 bits, so the PSP draws the cells within 12 m of the eye and the rest in two passes with a range each, and the effects once in each. Here one range holds the street from 0.15 m to 420 m.
- **No triangle is cut on the CPU.** The GE drops a triangle with a corner outside its guard band, and the PSP tests and cuts the large ones. This rasterizer clips.
- **The levels of detail stay at the pack's distances** (9 m and 28 m). The PSP holds a frame to 22 500 triangles and 24.5 ms and brings the levels in when a view is over either, down to 0.35 of their distances. A heavy view here draws 37 000 triangles.
- **Every tick of the match is drawn**: 60 frames a second on a display that refreshes 60 times. The PSP draws every second tick.
- **The frame is brought to 16 bits once, at its end.** The GE dithers every write and blends the glass, the shadow and the effects with what the 16-bit buffer holds. Here they blend at eight bits a colour and the dither is one pass over the finished frame.
- **The interface is drawn at eight bits a colour** over the dithered frame. The PSP draws it into the 16-bit buffer.
- **A picture is texels, not palette indices.** The pack stores each level as one byte a texel in the GE's tiles of 16 by 8, with a palette of 256 colours; the module looks the colours up at load. Both mix between texels after the lookup. Which level a pixel takes is this GPU's choice, from its own derivatives.
- **A corner's numbers are laid out on four-byte boundaries.** The pack's corner is 12 bytes in the GE's order; here it is 16, with the colour of 16 bits expanded to the eight a GE reads it as. The matrices and the figures' light are 32-bit floats; the GE takes a light's colour as eight bits a part, and the record is clamped the same way.
- **The glass is ordered by each chunk's distance.** The PSP draws the far pass's glass, then the near pass's.
- **The interface takes every turn that brings it news**, 30 a second. On the PSP a turn is 9 ms, and a turn that would bring nothing but new readouts is taken every second or third frame.
- **The frame is paced by the display's refresh and the browser's clock.** A display that does not refresh 60 times a second gets whole ticks when its interval is a multiple of a sixtieth of a second, and owes the rest to the next frame otherwise.

## Measured

Chrome 154 (headless, WebGPU on the Apple GPU, Metal 3) on an M3 Max, the pack `psp30` (`752f4d9e…`), served from the same machine, 2026-10-13. `bun tools/wgpu.ts check` and `check --dist` write these to `.pocket-build/validation/web/`.

| | |
| --- | --- |
| Frames a second over 5 s, a match played by the bench's player with the interface over it | 59.96 to 60.16 in six runs |
| A frame's cost without the display, over 300 frames | 0.34 to 0.37 ms |
| What a frame draws in those matches | 564 to 1 029 draws, 21 800 to 37 200 triangles of the street, 3 680 to 4 380 of soldiers and the weapon |
| A turn of the guest; turns taken a second | 0.06 to 0.09 ms; 26 to 30 of 30 |
| A redraw of the interface's picture; redraws a second | 0.54 to 0.60 ms; 16 to 21 |
| The tab's frame of a held eye against this machine's own (`duty-shot`, Metal) | the same picture: no colour differs |
| This renderer's frame of the match's start against the compiler's own drawing of it (`bun tools/duty.ts shot`, 480 × 272, the street alone, eight bits a colour) | a colour differs by 2.2 of 255 on average; 1.1% of the pixels by more than 16 |
| On a line of 16 Mbit/s: the interface up, the first frame, the first frame of the street | 1.9 to 3.9 s, 2.4 s, 16.5 s (33.3 MB received) |
| The module; PocketJS's UI core | 665 kB (233 kB gzipped); 421 kB (156 kB gzipped) |
| Geometry and texels on the GPU | 27.4 MB |

The check also drives the page: the title card covers the page before the game is shown; from the title into a match by the ○ key; the stick runs; a face button turns the view; L raises the sights with the stick held; R fires and a block of the mixer's output is not silence; the direction pad changes the magazine and the weapon; START stops the match under the menu; a setting changed there is in `localStorage`; the bench's player fells a soldier; a soldier fells the player, who is back after four seconds; the clock runs out to the results and the results' first row starts a match; the PSP's shell is whole in windows of 1440 × 900, 1280 × 800 and 390 × 844, and its ○ key starts a match under a pointer and under a finger; a browser without WebGPU is told so.

**Not checked:** no browser but Chrome and no GPU but this Mac's has drawn it; no phone has; nobody has played it by hand or listened to it; and no tool compares its frame with a capture from a console (the compiler's drawing stands in for one).
