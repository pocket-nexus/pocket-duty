# Pocket Duty

[Claude of Duty](https://github.com/mshumer/Claude-of-Duty) on the PSP, the Nintendo 3DS and the PS Vita: the reference's market street, its soldiers and its three weapons at **30 frames a second**, at 480 × 272 through the PSP's fixed-function GE and at 400 × 240 through the 3DS's PICA200, and at **60 frames a second** at 960 × 544 with four samples a pixel through the Vita's GXM. Built on [Pocket3D](https://github.com/pocket-nexus/pocketjs) and PocketJS.

Its page on Pocket Studio, [studio.pocket.nexus/games/duty](https://studio.pocket.nexus/games/duty), has recordings from a console and the package for the PSP.

A browser tab plays it too, at [duty.studio.pocket.nexus](https://duty.studio.pocket.nexus/): the same match compiled to wasm32, with a wgpu renderer that draws the PSP's pack inside a PSP's shell or a Nintendo 3DS's, chosen in the page's bar (`wgpu/`, "In a browser tab").

The reference is a browser game: three.js on WebGL2, every mesh, texture, animation and sound built by code at start. This repository keeps it unmodified in `web/`, runs it once in a desktop browser to record what it builds, and compiles the record for the console.

| | |
|---|---|
| Reference | `web/`, Claude of Duty by mshumer at `d9b237b`, MIT (`web/LICENSE`) |
| PSP | PSP-2000 or later (the pack needs the 52 MB memory setting; a PSP-1000 has 24 MB). A whole match on a console: **29.9 frames a second, 55 frames late of 18 375** |
| Nintendo 3DS | Old 3DS or New 3DS with the Homebrew Launcher: one `.3dsx` and the maps' packs in `packs/` beside it on the memory card. A whole match on an Old 3DS: **29.8 frames a second, 48 frames late of 17 954** |
| PS Vita | A PS Vita that runs homebrew: one `.vpk` with every map's pack, the interface and the programs inside. A match on a console: **59.9 frames a second, 1 frame late of 4 698**, with 135 000 triangles of the street in a frame |
| Browser tab | As a PSP, 480 × 272 in the PSP display's 16-bit colours; as a Nintendo 3DS, 400 × 240 over a lower screen of 320 × 240 with its four keys; as a PS Vita, 960 × 544 with the right stick looking. wgpu over WebGPU, the PSP's pack. Chrome 154 on an M3 Max: **60 frames a second, under 0.4 ms a frame** |

The consoles and the browser are measured in "What is measured".

## Build and run

Tools: Bun, Rust, Chrome (the export drives it through `playwright-core`), and the PSP toolchain PocketJS pins (`vendor/pocketjs/tools/psp-toolchain.ts`).

```
bun run setup                      # the submodule, the tools' packages, the reference's packages
bun tools/duty.ts export           # the reference in Chrome → .pocket-build/duty/ir (DutyIR, 119 MB)
bun tools/duty.ts cook             # DutyIR → .pocket-build/duty/psp30/duty.pack and receipt.json (4 minutes)
bun tools/duty.ts shot --png f.png # a frame of the cooked pack, drawn on this computer
bun tools/psp.ts build             # ui/ + psp/ → dist/psp/{EBOOT.PBP,duty.js,duty.pak}
bun tools/psp.ts emu               # the same EBOOT in PPSSPPHeadless: a frame and its status
bun tools/psp.ts emu --ctl "changes=20 held=4096"   # twenty changes of map in the running program, in an XMB start's memory: a line a change (changes.txt)
bun tools/psp.ts package           # dist/psp/PSP/GAME/PocketDuty for a Memory Stick
```

The PS Vita's, with VitaSDK (`VITASDK`), `cargo-vita` and the toolchain PocketJS's Vita host pins (`nightly-2026-05-28`):

```
bun tools/duty.ts cook --profile vita60   # DutyIR → .pocket-build/duty/vita60/duty.pack (2 minutes once the sight lists are kept)
bun tools/wgpu.ts shot --pack .pocket-build/duty/vita60/duty.pack --size 960x544 --out f.png   # a frame of that pack, drawn on this computer
bun tools/vita.ts serve            # the USB host of Pocket Devkit, the development container on the console; keep it running
bun tools/vita.ts run              # interface + packs → the share, vita/ → a SELF, the SELF into Devkit's native slot
bun tools/vita.ts bench --clock 60 # a match played by the bench's player through to its results: frame times from the console
bun tools/vita.ts cost             # one held view with each part of the frame left out in turn, the GPU timed
bun tools/vita.ts capture --out f.png
bun tools/vita.ts package          # dist/vita/pocket-duty-PKDT00001.vpk
```

Those are the first map's commands, the reference's street. Another map is the same with `--map <id>` (a directory under `maps/` a map):

```
bun tools/duty.ts export --map yards   # maps/yards/place.js through the reference's kit → .pocket-build/duty/maps/yards/ir
bun tools/duty.ts walk --map yards     # where the soldiers can walk, read from the export
bun tools/duty.ts cook --map yards     # → .pocket-build/duty/maps/yards/psp30/duty.pack (100 s)
bun tools/psp.ts emu --map yards       # the PSP program started on it
bun tools/duty.ts maps                 # which maps are exported and cooked
```

For the 3DS, with Docker (the build runs in PocketJS's pinned devkitARM container) and Azahar for `emu`:

```
bun tools/duty.ts cook --profile n3ds30   # .pocket-build/duty/n3ds30/duty.pack (28.3 MB)
bun tools/n3ds.ts build            # n3ds/ + ui/ + the first map's pack → dist/3ds/pocket-duty.3dsx (31.4 MB)
bun tools/n3ds.ts build --embed none   # the same program with no pack inside (3.1 MB): it reads the card's
bun tools/n3ds.ts card <directory> # what a memory card takes: 3ds/pocket-duty/ with the .3dsx and packs/
bun tools/n3ds.ts emu              # the same .3dsx in Azahar, every cooked map on its card: both screens and the status
bun tools/n3ds.ts install          # send it to a paired console over the LAN and start it
bun tools/n3ds.ts status           # frame time, triangles, the match, memory
bun tools/n3ds.ts bench            # a match played by the bench's player, 60 s of frame timings
bun tools/n3ds.ts capture          # both screens
bun tools/release.ts               # dist/release: the PSP's zip and the 3DS's zip, with release.json
```

`install`, `status`, `bench` and `capture` reach a console that runs a Pocket Runtime program with the paired wire (this one, once it is on the card) and whose pairing key is under `vendor/pocketjs/.pocket/3ds/devices/`. By hand: copy `pocket-duty.3dsx` to `sdmc:/3ds/` and start it from the Homebrew Launcher.

In a browser tab (the `wasm32-unknown-unknown` target and `wasm-bindgen` 0.2.126; Chrome for `check`):

```
bun tools/wgpu.ts serve            # build, then http://127.0.0.1:8802/ with the cooked pack
bun tools/wgpu.ts shot --frames 600 --words "mode=play auto=1" --out f.png   # a frame on this machine's GPU
bun tools/wgpu.ts check [--dist]   # the page in Chrome with WebGPU: the title, a match, the menu, the results, the same as a 3DS
bun tools/wgpu.ts dist             # the directory a static host serves → .pocket-build/wgpu/dist
```

Over PSPLINK, with a PSP on the cable:

```
bun tools/psp.ts serve             # usbhostfs_pc, when none is running
bun tools/psp.ts run               # build, stage on the share, reset PSPLINK, start
bun tools/psp.ts status            # host0:/duty/status.json: frame time, triangles, the match
bun tools/psp.ts ctl "ui=start"    # host0:/duty/control.txt (the words are in psp/src/main.rs)
bun tools/psp.ts bench             # a match played by the bench's player, 60 s of frame timings
bun tools/psp.ts tour              # every start point, four headings, turning and walking: late frames by place
bun tools/psp.ts film --boot "@10:ui=start @24:go=-14.68,-26.73,60 @24:auto=1" --frames 600 --out DIR
                                   # a run's pictures, a thirtieth of a second of the match apart
bun tools/psp.ts capture
```

## Controls

| | |
|---|---|
| Stick | Walk. All the way forward runs |
| △ ○ × □ | Look. On a PS Vita the right stick looks too |
| R | Fire |
| L | Raise the sights |
| Up | Jump. Held against a ledge, climb |
| Down | Crouch. While running, slide |
| Left | Change the magazine |
| Right | Next weapon |
| SELECT | Fire mode |
| START | Menu |

On a 3DS the Circle Pad is the stick, A B X Y look, and the +Control Pad, L, R, SELECT and START do what the PSP's do. A New 3DS's C-Stick looks too. The lower screen shows the level from above with the player and the soldiers he has seen, and four keys for a thumb or a stylus: next weapon, fire mode, magazine, menu.

A setting swaps the stick and the four buttons. Aim assist slows the look while a soldier in sight is under the crosshair, and raising the sights turns the view onto the nearest soldier in front of it; it is on by default and a setting turns it off.

## The match

The reference has no match: six soldiers, no score, no end. Pocket Duty adds one, in `crates/duty-sim/src/game.rs`:

- **Team Deathmatch, 10 minutes, first side to 30.** The player is one side, the soldiers the other.
- Two squads of three stand at the start. When fewer than five soldiers stand, three more arrive at the start point farthest from the player that he cannot see.
- A player who falls is back after 4 seconds at the start point farthest from the soldiers, with full magazines.
- A squad that has neither seen nor heard the player for 14 seconds walks to where he is.
- Three difficulties scale the soldiers' damage: 0.45, 0.7 and 1.0 of the reference's.

Taken from the reference as they are: the weapons' tables and recoil patterns, the player's speeds, jump, slide and climb, the soldiers' senses, cover and flanking, health and its return, penetration by surface.

Left out: leaning, lying down, and the reference's ragdolls (a soldier who falls plays a fall and lies).

## Maps

A map is a plan drawn by hand (`maps/<id>/place.js`): **floors as polygons at their heights**, in the place's own coordinates, built by the reference's kit in Chrome, exported and cooked like the street. **A map is a directory under `maps/`** with its own name, plan, look and views, and no file lists them; the reference's street is first, then Twin Yards: two sites, a long lane with a pit, a middle lane, covered ways on two heights and a high start.

- `web/src/pocket/placekit.js` builds a place with no street, gate or perimeter: a wall where a floor's edge has nothing beyond it, a face where a lower floor lies beyond, a cap over each block between, and what a plan lists beside its floors (walls with openings, covered ways, doors, crates, houses). `placelook.js` gives a map surfaces of its own, drawn by functions of ours and baked by the reference's material system, and its sky. The reference's files are unchanged.
- The cook, the pack, the match and the devices hold no code for a particular map: starts, walk grid, collision and sight lists are sections of each map's pack.
- **A device holds one map's pack at a time.** The interface lists the maps a device has a pack for. A browser tab reads the other pack in place of the first; a PSP, which cannot give a pack's memory back, writes the map's name beside its packs and starts itself again; a 3DS's package holds one map, the first unless `bun tools/n3ds.ts build --map <id>` names another.
- **A map may be a place a player knows, restated by us.** Its plan, proportions, heights and landmarks follow the place, typed by hand against the author's own study pictures; its surfaces are made by our code in the place's family of colours; its names are ours. Nothing is converted from a file of another game, and nothing of one enters the repository, a pack, a package or a page.

`maps/README.md` is how a map is made, what a layout states, how to add one and how it is checked.

## How it is built

```
maps/                the maps: a directory a map (map.json, place.js, look.js, views.json)
web/                 the reference, unmodified, plus the export page (web/src/pocket, web/export.html)
tools/               duty.ts (export, cook, shot, look, walk), maps.ts (where each map's files lie), ui.ts (the interface), psp.ts and n3ds.ts (build, emu, run, bench), wgpu.ts (the browser), release.ts
profiles/            psp30.json, n3ds30.json: what a console's pack is (cell size, levels of detail, texture format, budgets)
crates/duty-pack     the pack's records
crates/duty-cook     DutyIR → pack
crates/duty-sim      the match, the choice of what a frame draws, the posing and the choice of sounds (no_std)
crates/duty-interface  the flow and the protocol to the interface
ui/                  the interface: one PocketJS app
psp/                 the PSP program
n3ds/                the 3DS program: a C host and PICA200 renderer (src/), the match as a Rust static library (core/)
wgpu/                the browser's program: a wgpu renderer of the PSP's pack, the shell around the match, the page
vendor/pocketjs      PocketJS and Pocket3D
```

### Export

`web/export.html` starts the reference as its own `main.js` does, with a fixed seed, and writes DutyIR:

- **The world's triangles**, with the colour and the light the reference's own materials give each vertex. The page draws every vertex as one point into a texture through the material's program, twice: once for the colour before light, once for the light with the sun off (sky, lamps, bounce). No material is written a second time in the compiler.
- **The tone table**: the reference's exposure, AgX and grade passes run on a 64³ table of light values.
- **The sky** as six faces, the textures, the collision triangles, the walk grid and its cover.
- **The soldiers and the held weapons**: meshes, bones, and 25 clips sampled from the reference's animators.
- **58 sounds**, rendered through the reference's own mixer.

### Cook

`crates/duty-cook` lowers DutyIR for `profiles/psp30.json`:

- **8 m cells, three levels of detail** a cell and material (meshopt, vertices shared between cells kept in place), **4 310 chunks** in all: 428 969, 162 308 and 57 310 triangles.
- **Light at the vertex**: the exported sunless light scaled by how much sky the vertex sees, plus the sun where rays reach it. Edges are cut where the sun's shadow crosses a face. The result goes through the reference's tone table to a 16-bit colour.
- **The ground's shadow** as one 512² picture over 128 m, multiplied in by a second pass.
- **Clusters** of at most 32 triangles, grouped by facing, each with a sphere, a box and a cone: **43 270**. The cone has an apex behind the plane of every face, so an eye behind a flat cluster's plane leaves it out. For each of 409 cells, the clusters any ray from the cell reaches (400 000 rays a cell, through all three levels at once: a frame mixes them).
- **A sphere and the longest edge of each large triangle** (339 568 of them, 8 bytes each): what the device asks before it reads a triangle's corners.
- **A chunk's record is 128 bytes**, two lines of the PSP's data cache, with what a frame asks of a chunk it leaves out in the first.
- **Textures**: 24 pictures with 256-colour palettes, 695 KiB.
- **The walk grid**: only its largest connected part stays open (29 776 of 38 742 nodes), so every way a soldier asks for exists.
- The pack is **30.5 MB**; `receipt.json` records its hash and counts.

### The frame

`psp/src/gfx.rs` draws what `duty_sim::view::select` chooses: the clusters the eye's cell can see, inside the view and not all turned away, at the level of detail their distance asks.

- **Two depth passes.** The GE's depth is 16 bits. The cells within 12 m of the eye are drawn from 0.15 m in one part of the depth range; the rest from a few metres in another. A ray from the eye meets every near cell before every far one, so the passes never compare depths.
- **The guard band.** The GE drops a triangle with a vertex outside its 4096-pixel space. A triangle reaches from the view to outside the band only when its longest edge is more than half its distance from the eye. Of the large triangles of a cluster whose box is not inside the band, the ones that near are tested, and cut on the CPU when the GE cannot take them.
- **A budget of 22 500 triangles and of 24.5 ms** a frame: over either, the levels of detail move in. A triangle of a soldier or of the held weapon counts for one and a half.
- Soldiers and the held weapon are skinned by the GE with four bones a batch, and lit by the sun, where the ground's shadow picture says it reaches, and the sky. A soldier more than 16 m off is drawn from a model of 406 triangles in place of 696.

What the console's parts cost decides how the frame is written (`probe=1` in `boot.txt` times them):

- The CPU's floating point unit takes **20 cycles for a multiply and an add**. A chunk answers for its clusters where it can: a cluster is asked only about the sides of the view its chunk reaches across, and about the guard band only when its chunk is not inside it.
- A line of the data cache fetched while the GE draws takes **about a microsecond**: the CPU and the GE share the bus. A cell out of the view answers for its chunks without their records being read, and the player's body keeps the triangles about it from step to step.
- A word of the list `sceGu` writes is **a bus cycle of its own**. The world's commands are written through the cache into runs that the list calls.
- The GE takes **0.7 ms for a thousand triangles** it is sent, drawn or not.

### The frame on a 3DS

`profiles/n3ds30.json` is the PSP's profile with two differences: textures are **RGB565, RGBA5551 and A8 in the PICA200's tiled order** in place of palettes, and the large-triangle records are left out. The pack is **28.3 MB**. `n3ds/core` runs the same `duty-sim` as the PSP and hands `n3ds/src/render.c` the frame's draws.

- **One depth pass.** The depth buffer is 24 bits, so the near and far cells share it. The held weapon takes the nearest 8% of the range with a frustum of its own.
- **No guard band.** The rasterizer cuts a triangle at every side of the view. The core selects with `Guard::open()` and no triangle is cut on the CPU.
- **The ground in one draw.** The first combiner multiplies the texel by the vertex's colour; for ground a second multiplies by the shadow picture, read through the second texture unit at the vertex's place. The PSP draws the ground twice for that.
- **Haze in the combiners.** The vertex program writes what the haze leaves into the colour's alpha and the last combiner mixes the haze's colour in by it.
- **Soldiers and the weapon** are moved by four bones and lit in a vertex program.
- **A budget of 36 000 triangles**, each counted once. In the measured match the street stays at its full levels of detail.
- **A draw is written as GPU commands.** `C3D_DrawElements` looks at all 96 uniform registers on every call: **16 microseconds a draw on an Old 3DS**, 9.4 ms for a view's draws. `render.c` writes a cell's matrix, the buffer offset and the draw registers itself and has citro3d write its state only after a texture or combiner change: **7 microseconds a draw**, 2.8 ms for the same view.
- **Sound** is mixed by `duty-sim` at 22 050 Hz and played through `ndsp`. `ndsp` needs the DSP firmware on the card (`sdmc:/3ds/dspfirm.cdc`); without it the program plays through CSND.

### The frame on a PS Vita

`profiles/vita60.json` is what the Vita's pack holds more than the PSP's. `vita/` runs the same `duty-sim`, reads the pack a section at a time through `pocket3d-packs`, and draws the browser renderer's frame with GXM (`vita/src/world.rs`, `vita/src/figures.rs`, six Cg programs in `vita/shaders/`).

| | `psp30` | `vita60` |
|---|---|---|
| Pictures | 256 colours from a palette, **128 texels a side** (64 and 32 for six families), 4 levels | **true colour, the side the reference bakes: 512** (256 for six families), every level down to one texel |
| Texels of the surfaces' pictures | 0.22 million | **3.5 million** |
| Sky | 4 pictures of 256 × 128: 1 024 texels round | 8 pictures of 512 × 512: **4 096 texels round** |
| The ground's shadow picture | 512 a side: 25 cm a texel | **2 048 a side: 6 cm a texel** |
| Triangles of the street, by level | 428 969 / 162 308 / 57 310 | **1 105 375** / 353 309 / 121 408 |
| The nearest level reaches | 9 m | **96 m** |
| A triangle across a shadow's edge is cut to | 1 m | **35 cm** |
| The figures' budgets: a soldier, the held weapon, an arm | 700, 1 100 and 260 triangles | **6 000, 9 000 and 2 200** |
| The street's pack, Twin Yards' | 29.1 MiB, 8.9 MiB | **93.0 MiB, 49.7 MiB** |
| Triangles in a frame of a match | 19 000 | **135 000** |

- **Straight into the display's surface.** vita2d starts GXM with a display surface of four samples a pixel and the street is drawn into it; no second target and no copy.
- **The pictures are true colour** (`texture::RGBA8`, `"format": "rgba"`), mixed between two texels and between two levels. A picture of 512 a side or less goes to video memory in the GPU's own order of texels through the transfer engine; the ground's shadow picture stays as rows, because the engine refuses a copy of 2 048 a side (`0x805b0003`).
- **A picture with holes keeps its cover.** A smaller level is a mean of texels, and the mean of a leaf is under one half everywhere. The compiler raises each level's alpha until as much of it passes the test as passes in the first level.
- **The ground in one pass.** A ground chunk's fragment multiplies its texel by the shadow picture at the fragment's own place.
- **A soldier's shadow.** His triangles are drawn once more with each vertex sent down the sun's ray to the height of his feet, and the stencil lets a pixel darken once.
- **A muzzle's light.** While the player's flash shows, the street's programs add a light at the muzzle that falls off with the square of the distance (`FLASH`). The programs without it are bound in every other frame.
- **The right stick looks** (`Session::second`); the four buttons still look or walk as the layout setting says.
- **A budget of 170 000 triangles.** Over it, or when a frame waits more than 6 ms for the GPU to finish the frame before last, the levels' distances come in.
- **Programs are compiled on the console** by SceShaccCg at the first start, 28 in all, and kept on the memory card under the hash of their source. A package holds them (`app0:gxp`).
- **Another map is read in place.** The geometry's memory and the pictures' video memory go back first.

Not done: the sun's shadow per pixel on walls and a place's lamps per pixel. A vertex holds one colour, the lit one; both need the compiler to hand over the colour without the sun and the lamps' list. The dust skirts and the pock marks the reference lays on walls (`prop_dust_skirt`, `prop_pock`) stay out of the pack: they need a material that blends by its texel's alpha.

### The interface

Title, settings, the match's instruments, the menu and the results are one PocketJS app (`ui/`) drawn over the street. Neither console's program draws text. On a 3DS the same app has a second surface, the lower screen (`ui/app/presentations/dual.tsx`); the upper screen keeps the PSP's layout. `crates/duty-interface` sends the match's state as JSON lines and takes commands back (`ui/app/protocol.ts`).

```
bun tools/ui.ts preview    # pictures of every screen, .pocket-build/ui/preview/
bun tools/ui.ts test       # the flow through a mock renderer
```

## In a browser tab

`wgpu/` is the game for a tab: **`duty-sim` and `duty-interface` compiled to wasm32, a wgpu renderer over WebGPU that reads the PSP's pack as it is, and PocketJS's Pocket3D player around it** (`vendor/pocketjs/devices/web/pocket-web-wgpu`). The reference in `web/` is not published as the game.

- **The pack is the PSP's**: the triangles, the baked light, the palette pictures, the sight lists, the soldiers, the sounds and the walk grid of `duty.pack`. What the GE draws from it with no program, WGSL programs draw: a texel times the corner's colour, doubled, under the haze; the ground once more under its shadow picture; four bones and two lights for a soldier. The frame is shown in the PSP display's 16-bit colours with the GE's dither.
- **A browser's GPU needs none of the PSP's three devices for a frame**: one depth range of 24 bits holds the street, the rasterizer clips, and the levels of detail stay at the pack's distances. Every tick of the match is drawn, 60 a second.
- **The page is the kernel's player**: a PSP's shell with the screen in it and its keys as the controls (W A S D the stick, I J K L the four look buttons, E and Q for R and L, the arrows for the direction pad, Space for START). The interface is `ui/` as the PSP loads it, run in a realm of the page and laid over the street.
- **The page plays as every device the game has a package for**: a PSP and a Nintendo 3DS (`DEVICES` in `wgpu/page/main.js`, `?device=3ds`). Each is a shape of the renderer (`SHAPES` in `wgpu/src/app.rs`) and the interface bundle that device's own build loads (`ui/3ds` has the lower screen, which the page draws on a second canvas and whose keys take the pointer). As a 3DS the page draws the PSP's pack at 400 × 240 with eight bits a colour; the 3DS's own pack and the PICA200's combiners are not in the tab.
- **A device with a package is a device of the page.** `tools/wgpu.test.ts` holds the release's targets (`tools/release.ts`), the page's devices and `runsOn`, the renderer's shapes, the interface bundles of the site and `<meta name="pocket-devices">` of `wgpu/page/index.html` to one another, and `bun run test` fails when one of them names a device another does not. A new target lands with its shape, its bundle and its place in the page in one change, and the site is deployed before the package is uploaded.
- **The same renderer writes frames to files** on the build machine (`wgpu/src/bin/shot.rs`), a sixtieth of a second a frame.
- `bun tools/wgpu.ts dist` writes the directory a static host serves: 53 files, 33.1 MB, the pack in pieces of 2 MiB named by their hashes. `pocket-studio site .pocket-build/wgpu/dist` deploys it to the game's address on Pocket Studio, from the checkout `pocket-studio register` linked.

`wgpu/README.md` has the frame, the controls, what differs from the PSP's picture and what was measured.

## What is measured

Host: `cargo test --workspace` (duty-sim 43 tests, duty-cook 7). `cargo run --release -p duty-sim --example match -- .pocket-build/duty/psp30/duty.pack 30` plays 30 minutes of matches with a player who aims at the nearest soldier in sight; `--example view -- <pack> x y z yaw pitch` prints what a frame from a place draws and what becomes of its large triangles.

Emulator (`bun tools/psp.ts emu`, PPSSPPHeadless with its software renderer): the title, a match, the menu, settings, a fall and the results draw as intended. The emulator's times count instructions: they miss what the floating point unit, the data cache and the bus cost a console, which is most of a frame.

Emulator (`bun tools/n3ds.ts emu`, Azahar with its software PICA200): a frame of a match on both screens, with no read or write of unmapped memory in the log.

Console (a PSP with 64 MB, over PSPLINK, 2026-10-13):

| | |
|---|---|
| `bench --seconds 615`: a whole match played by the bench's player, who falls 4 times and wins 18 to 4 | **18 375 frames, 55 late (0.30%)**, 29.88 a second, the longest 103 ms |
| The same with a player who cannot fall and stays near his start | 18 435 frames, 9 late (0.05%) |
| `tour`: 8 start points × 4 headings, walking while the view turns | **2 250 frames, none late**; 3 late of 2 340 in another run |
| A frame of the first match, on average | match 2.4 ms, choosing the draws 7.5 ms, the list 11.2 ms, the interface 2.9 ms (a turn is 9.7 ms, 9 a second), waiting for the GE 2.9 ms |
| The GE, from the list's first word to its last pixel | 27 ms on average in that match: 19 000 triangles of the street, 6 soldiers, the weapon and the interface |
| Memory | 38.9 MB of the program's 50.8 MB after the match, 38.2 MB at its start |
| `run --standalone --boot "title=0 changes=20 held=4096"`: twenty changes of map in the running program, files beside the PRX, 4 MiB kept from it | **no restart; the arena's free bytes (4 846 880 and 4 859 168) and its largest free block (3 344 912) are the same from the third change to the twentieth**; a change takes 0.85 s to Twin Yards and 1.65 s to Market Street; 33.5 to 33.6 ms a frame in the 120 frames after each; no open off the main thread |
| The whole match again with this program (`bench --seconds 615`; the bench's player falls twice and wins 20 to 2, another course than the first row's) | 18 375 frames, 85 late (0.46%), 29.88 a second, the longest 106 ms. Not compared with the program before it on the same day |
| The same started on Twin Yards (six changes), and from `host0:/duty/` with nothing kept back (two) | level from the third change (9 053 440 and 9 041 152 free); 13.2 MB free with nothing kept back |

That first run started on a PSPLINK session that had been up for a day and had 44.7 MB free at its start; after a reset the same words start with 48.9 MB free, and with nothing kept back 53.1 MB. A start from the XMB has not been measured with this program.

The first run on the console took 35.7 ms a frame with one frame in six late. What changed is in "The frame" above.

Console (an Old 3DS over the paired LAN wire, 2026-10-13):

| | |
|---|---|
| `bench --seconds 600`: a whole match played by the bench's player, who falls 3 times and leads 21 to 3 when the clock ends | **17 954 frames, 48 late (0.27%)**, 29.81 a second, the longest 117 ms |
| A frame of that match, on average | CPU 22.9 ms: match 3.9 ms, choosing the draws 8.4 ms, drawing 8.5 ms, the interface 1.8 ms (a turn is 5.9 ms, 9 a second). The PICA200 13.5 ms, 16.2 ms at most |
| What a frame sends | 32 700 triangles in 858 draws on average, 34 900 and 990 at most; the levels of detail at 0.96 of their distances on average, 0.8 at least |
| The street's draws in the start view (383 draws) | 2.8 ms as GPU commands; 9.4 ms through `C3D_DrawElements` |
| Memory | 19.6 MB of the 31.3 MB heap after the match, 18.7 MB at its start; 5.2 MB of linear memory free at both |
| Captures from the console | title, match, menu, settings, a fall and the results on both screens; a soldier at 3 m, a kill with its notice; the four keys of the lower screen change the weapon, the fire mode, the magazine and open the menu |

Sound on that console plays through CSND: its card has no `dspfirm.cdc`. A person played it on the console and heard the match's sounds (2026-10-13). The `ndsp` path has not run on a console.

Browser (`bun tools/wgpu.ts check`, Chrome 154 headless with WebGPU on an M3 Max, 2026-10-13): the page goes from the title card through a match played by keys to its results, with the menu, a fall and the settings on the way; **60 frames a second, under 0.4 ms a frame** with up to 37 200 triangles of the street; the tab's frame of a held eye is the build machine's own; and that frame is 2.2 of 255 a colour from the compiler's drawing of the same view. No other browser, no phone and no other GPU has drawn it, and nobody has played it by hand or listened to it.

Console (a PS Vita in Pocket Devkit's native slot over USB, 2026-10-13), 960 × 544, four samples a pixel, clocks at 444 / 222 / 222 / 166 MHz:

| | |
|---|---|
| `bench --seconds 75 --clock 60` on the street: a match played by the bench's player to its results (6 kills, 240 rounds) | **4 698 frames, 1 late**, 16.68 ms a frame, the longest interval 29.9 ms; 134 500 triangles of the street in 1 109 draws on average, 137 900 at most |
| A frame of that match, on this thread | choosing the draws 3.4 ms, drawing 3.7 ms, the interface's draw 0.8 ms, the match 0.9 ms |
| The same on Twin Yards, where the bench's player stays at his start | **5 036 frames, 1 late**, the longest interval 17.9 ms |
| `cost`: a held view down the street, 116 000 triangles of the street and 11 000 of two soldiers in 800 draws | **the GPU 12.95 ms** |
| The same view with two samples a pixel, and with one | 10.41 ms, 8.84 ms: **four samples cost 4.1 ms** |
| A thousand triangles of the street | 0.043 ms of the GPU |
| The interface over the frame | 1.3 to 2.3 ms of the GPU by what it shows; 0.8 ms of this thread |
| The ground's shadow picture; the sky; the haze | 0.51 ms; 0.30 ms; under 0.02 ms |
| Two soldiers; of that their shadows | 0.33 ms of the GPU and 0.7 ms of this thread; 0.08 ms and 0.1 ms |
| The nearest level to 96 m in place of 48 m | 0.09 ms in that view |
| Runs of a chunk drawn one by one, and joined across 200 triangles, against joined across 50 (the default) | the GPU 0.07 ms more and this thread 0.20 ms more; the GPU 0.48 ms more and this thread 0.11 ms less |
| Pictures in the GPU's order of texels in place of rows | 0.16 ms less |
| Four held views of Twin Yards | the GPU 4.1 to 5.8 ms |
| A start | the street read in 13.9 s with 15 MB of it over the cable; Twin Yards in 16.9 s with 41 MB over the cable |
| Memory | 49 MB of geometry and 44 MB of pictures in video memory for the street; 8 MB and 40 MB for Twin Yards |
| Captures from the console | the title and a match on both maps, the results |

A capture stops the frame it is taken in for 100 ms. The package (`.vpk`) was built and was not installed on a console.

## Releases

`bun tools/release.ts` builds the package Pocket Studio takes from the checked-out commit and writes it to `dist/release/`, which Git ignores:

```
bun tools/release.ts [--targets psp,3ds,vita] [--out dist/release] [--no-build] [--no-cook] [--upload]
```

| Target | File | Holds |
| --- | --- | --- |
| `psp` | `pocket-duty-<version>-psp.zip` | `PSP/GAME/PocketDuty/` for the root of a Memory Stick: `EBOOT.PBP`, every map's pack as pieces (`pocket.packs` and the `.bin` files it lists), and the interface's `duty.js` and `duty.pak` |
| `3ds` | `pocket-duty-<version>-3ds.zip` | `3ds/pocket-duty/` for the root of a memory card: `pocket-duty.3dsx` with the interface in its ROMFS (**3.1 MB**) and `packs/` with every map's pack as pieces |
| `vita` | `pocket-duty-<version>-vita.vpk` | The SELF with no debug link, `packs/` with every map's pack as pieces (**141 MiB**, 48 MB in the package), the interface, and `gxp/` with the 28 programs a console compiled. Built after a development run has left the programs on the USB share (`bun tools/vita.ts run`) |

A map's pack is cut into pieces named by their bytes, and one list (`pocket.packs`, PocketJS's `docs/POCKET3D-PACKS.md`) says which pieces make which map's pack. `bun tools/packs.ts cut [--profile psp30]` writes them under `.pocket-build/duty/packs/<profile>/`; `bun tools/packs.ts bring <directory>` copies the pieces a directory lacks. The browser, the PSP and the 3DS read a pack through that list. Beside each map's pack the list has `<id>-map`: the map from above, from the cook's `map.png` and `map.json` of the same profile, as 16 bytes of facts and a PocketJS image entry in the device's pixel format (`duty_interface::Chart`). The device reads it with the map's pack and hands it to the interface, so no map's picture is compiled into the interface. A `.3dsx` built without `--embed none` holds the first map's pieces in its ROMFS and runs with no card files; it is **31.4 MB**, and the paired wire installs at most 32 MiB.

- It cooks the profile's pack of every map the package holds from that map's export with this commit's compiler (`--no-cook` takes the cooked packs as they are), builds the program and writes the zip. **The zip's bytes follow from its entries alone**: fixed dates, fixed modes, entries in the order of their names.
- `release.json` beside it records the commit, the version (`ui/pocket.json`), the file's size and SHA-256, the export and the pack it was built from, and the toolchains.

**Packages go to Pocket Studio and to no page on GitHub.** `--upload` runs `pocket-studio package <file> --target <id> --version <version>` for each package from the repository's root, where `pocket-studio register --title "Pocket Duty"` wrote `.pocket-studio.json` (ignored by Git). `POCKET_STUDIO_CLI` names the command when it is not on `PATH`.

## The listing

`bun tools/listing.ts` films what the game's page on Pocket Studio shows and writes it to `dist/listing/`, which Git ignores:

```
bun tools/listing.ts [--only FILE… | --words] [--emu] [--upload]
```

- **The words are `listing/listing.json`**: one sentence, the paragraphs, a caption for each picture, and the Japanese of each under `translations.ja`. `--words` writes `dist/listing/listing.json` from them for the pictures already there.
- **Every picture is drawn by a PSP.** A run is the program started with control words in `boot.txt` (`runs` in `listing/listing.json`), filmed from its first frame: `film=<frames>` has the console write each frame's picture to the PSPLINK share and step the match two sixtieths of a second a frame while it does, so a run of the same words on the same build gives the same pictures. A take keeps frames of a run: a clip (`from`, `frames`, `poster`) at 30 a second, or a still (`frame`).
- The player in a run is walked by `go=<x>,<z>`, which finds a way over the walk grid as a soldier does, and aimed by `auto=1`, the bench's player. The words are `remote` in `psp/src/main.rs`.
- A clip and a still are the console's 480 × 272 with each texel doubled. The share picture (1200 × 630) and the cover (`dist/cover.png`, 480 × 272) come from a run with no interface over the street (`bare`).
- `bun tools/psp.ts film --boot "<words>" --frames N --out DIR` films one run; `--emu` has PPSSPPHeadless draw it, for trying a take. `--upload` refuses a picture the emulator drew: the listing says a PSP drew each one.

It needs the built program (`bun tools/psp.ts build`), a PSP on the cable under PSPLINK, ffmpeg and ffprobe. `--upload` runs `pocket-studio listing dist/listing` from the repository's root; `POCKET_STUDIO_CLI` names the command when it is not on `PATH`. No picture goes to Git.

## Licence

MIT (`LICENSE`). The reference in `web/` is Claude of Duty, © its authors, MIT (`web/LICENSE`). PocketJS and Pocket3D are under their own terms in `vendor/pocketjs`; the title card and the icon are Pocket3D's.
