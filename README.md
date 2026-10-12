# Pocket Duty

[Claude of Duty](https://github.com/mshumer/Claude-of-Duty) on the PSP: the reference's market street, its soldiers and its three weapons at **480 × 272 and 30 frames a second**, through the GE's fixed-function pipeline. Built on [Pocket3D](https://github.com/pocket-nexus/pocketjs) and PocketJS.

Its page on Pocket Studio, [studio.pocket.nexus/games/duty](https://studio.pocket.nexus/games/duty), has recordings from a console and the package for the PSP.

The reference is a browser game: three.js on WebGL2, every mesh, texture, animation and sound built by code at start. This repository keeps it unmodified in `web/`, runs it once in a desktop browser to record what it builds, and compiles the record for the console.

| | |
|---|---|
| Reference | `web/`, Claude of Duty by mshumer at `d9b237b`, MIT (`web/LICENSE`) |
| Console | PSP-2000 or later (the pack needs the 52 MB memory setting; a PSP-1000 has 24 MB) |
| State | A whole match on a console: **29.9 frames a second, 55 frames late of 18 375** (see "What is measured") |

## Build and run

Tools: Bun, Rust, Chrome (the export drives it through `playwright-core`), and the PSP toolchain PocketJS pins (`vendor/pocketjs/tools/psp-toolchain.ts`).

```
bun run setup                      # the submodule, the tools' packages, the reference's packages
bun tools/duty.ts export           # the reference in Chrome → .pocket-build/duty/ir (DutyIR, 119 MB)
bun tools/duty.ts cook             # DutyIR → .pocket-build/duty/psp30/duty.pack and receipt.json (4 minutes)
bun tools/duty.ts shot --png f.png # a frame of the cooked pack, drawn on this computer
bun tools/psp.ts build             # ui/ + psp/ → dist/psp/{EBOOT.PBP,duty.js,duty.pak}
bun tools/psp.ts emu               # the same EBOOT in PPSSPPHeadless: a frame and its status
bun tools/psp.ts package           # dist/psp/PSP/GAME/PocketDuty for a Memory Stick
```

Over PSPLINK, with a console on the cable:

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
| △ ○ × □ | Look |
| R | Fire |
| L | Raise the sights |
| Up | Jump. Held against a ledge, climb |
| Down | Crouch. While running, slide |
| Left | Change the magazine |
| Right | Next weapon |
| SELECT | Fire mode |
| START | Menu |

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

## How it is built

```
web/                 the reference, unmodified, plus the export page (web/src/pocket, web/export.html)
tools/               duty.ts (export, cook, shot), ui.ts (the interface), psp.ts (build, emu, run, bench)
profiles/psp30.json  what the PSP's pack is: cell size, levels of detail, texture sizes, budgets
crates/duty-pack     the pack's records
crates/duty-cook     DutyIR → pack
crates/duty-sim      the match, and the choice of what a frame draws (no_std)
crates/duty-interface  the flow and the protocol to the interface
ui/                  the interface: one PocketJS app
psp/                 the PSP program
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

### The interface

Title, settings, the match's instruments, the menu and the results are one PocketJS app (`ui/`) drawn over the street. The PSP program draws no text. `crates/duty-interface` sends the match's state as JSON lines and takes commands back (`ui/app/protocol.ts`).

```
bun tools/ui.ts preview    # pictures of every screen, .pocket-build/ui/preview/
bun tools/ui.ts test       # the flow through a mock renderer
```

## What is measured

Host: `cargo test --workspace` (duty-sim 43 tests, duty-cook 7). `cargo run --release -p duty-sim --example match -- .pocket-build/duty/psp30/duty.pack 30` plays 30 minutes of matches with a player who aims at the nearest soldier in sight; `--example view -- <pack> x y z yaw pitch` prints what a frame from a place draws and what becomes of its large triangles.

Emulator (`bun tools/psp.ts emu`, PPSSPPHeadless with its software renderer): the title, a match, the menu, settings, a fall and the results draw as intended. The emulator's times count instructions: they miss what the floating point unit, the data cache and the bus cost a console, which is most of a frame.

Console (a PSP with 64 MB, over PSPLINK, 2026-10-13):

| | |
|---|---|
| `bench --seconds 615`: a whole match played by the bench's player, who falls 4 times and wins 18 to 4 | **18 375 frames, 55 late (0.30%)**, 29.88 a second, the longest 103 ms |
| The same with a player who cannot fall and stays near his start | 18 435 frames, 9 late (0.05%) |
| `tour`: 8 start points × 4 headings, walking while the view turns | **2 250 frames, none late**; 3 late of 2 340 in another run |
| A frame of the first match, on average | match 2.4 ms, choosing the draws 7.5 ms, the list 11.2 ms, the interface 2.9 ms (a turn is 9.7 ms, 9 a second), waiting for the GE 2.9 ms |
| The GE, from the list's first word to its last pixel | 27 ms on average in that match: 19 000 triangles of the street, 6 soldiers, the weapon and the interface |
| Memory | 38.9 MB of the program's 50.8 MB after the match, 38.2 MB at its start |

The first run on the console took 35.7 ms a frame with one frame in six late. What changed is in "The frame" above.

## Releases

`bun tools/release.ts` builds the package Pocket Studio takes from the checked-out commit and writes it to `dist/release/`, which Git ignores:

```
bun tools/release.ts --targets psp [--out dist/release] [--no-build] [--no-cook] [--upload]
```

| Target | File | Holds |
| --- | --- | --- |
| `psp` | `pocket-duty-<version>-psp.zip` | `PSP/GAME/PocketDuty/` for the root of a Memory Stick: `EBOOT.PBP`, `duty.pack`, and the interface's `duty.js` and `duty.pak` |

- It cooks the profile's pack from the export with this commit's compiler (`--no-cook` takes the pack under `.pocket-build/duty/psp30` as it is), builds the program and writes the zip. **The zip's bytes follow from its entries alone**: fixed dates, fixed modes, entries in the order of their names.
- `release.json` beside it records the commit, the version (`ui/pocket.json`), the file's size and SHA-256, the export and the pack it was built from, and the toolchains.
- The tool also names a `3ds` target; its program is not in this repository.

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
