# Pocket Duty

[Claude of Duty](https://github.com/mshumer/Claude-of-Duty) on the PSP: the reference's market street, its soldiers and its three weapons at **480 × 272 and 30 frames a second**, through the GE's fixed-function pipeline. Built on [Pocket3D](https://github.com/pocket-nexus/pocketjs) and PocketJS.

The reference is a browser game: Three.js with WebGPU, every mesh, texture, animation and sound built by code at start. This repository keeps it unmodified in `web/`, runs it once in a desktop browser to record what it builds, and compiles the record for the console.

| | |
|---|---|
| Reference | `web/`, Claude of Duty by mshumer at `d9b237b`, MIT (`web/LICENSE`) |
| Console | PSP-2000 or later (the pack needs the 52 MB memory setting; a PSP-1000 has 24 MB) |
| State | The program runs in PPSSPPHeadless: every screen, a whole match. **No console run is recorded yet** |

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
- **Clusters** of at most 32 triangles, grouped by facing, each with a sphere, a cone and a box: **43 270**. For each of 409 cells, the clusters any ray from the cell reaches (400 000 rays a cell, through all three levels at once: a frame mixes them).
- **Textures**: 24 pictures with 256-colour palettes, 695 KiB.
- **The walk grid**: only its largest connected part stays open (29 776 of 38 742 nodes), so every way a soldier asks for exists.
- The pack is **27.6 MB**; `receipt.json` records its hash and counts.

### The frame

`psp/src/gfx.rs` draws what `duty_sim::view::select` chooses: the clusters the eye's cell can see, inside the view and not all turned away, at the level of detail their distance asks.

- **Two depth passes.** The GE's depth is 16 bits. The cells within 12 m of the eye are drawn from 0.15 m in one part of the depth range; the rest from a few metres in another. A ray from the eye meets every near cell before every far one, so the passes never compare depths.
- **The guard band.** The GE drops a triangle with a vertex outside its 4096-pixel space. A cluster's large triangles are tested when its box is not inside the band, and cut on the CPU when they are not.
- **A budget of 20 000 triangles** a frame: over it, the levels of detail move in.
- Soldiers and the held weapon are skinned by the GE with four bones a batch, and lit by the sun, where the ground's shadow picture says it reaches, and the sky.

### The interface

Title, settings, the match's instruments, the menu and the results are one PocketJS app (`ui/`) drawn over the street. The PSP program draws no text. `crates/duty-interface` sends the match's state as JSON lines and takes commands back (`ui/app/protocol.ts`).

```
bun tools/ui.ts preview    # pictures of every screen, .pocket-build/ui/preview/
bun tools/ui.ts test       # the flow through a mock renderer
```

## What is measured

Host: `cargo test --workspace` (duty-sim 39 tests). `cargo run --release -p duty-sim --example match -- .pocket-build/duty/psp30/duty.pack 30` plays 30 minutes of matches with a player who aims at the nearest soldier in sight.

Emulator (`bun tools/psp.ts emu`, PPSSPPHeadless with its software renderer): the title, a match, the menu, settings, a fall and the results draw as intended. A frame from the start point sends **17 400 triangles in 545 draws**, 2 272 of them the held rifle. The emulator's times count instructions and do not say what a console takes.

Console: no frame timing and no capture yet.

## Licence

MIT (`LICENSE`). The reference in `web/` is Claude of Duty, © its authors, MIT (`web/LICENSE`). PocketJS and Pocket3D are under their own terms in `vendor/pocketjs`; the title card and the icon are Pocket3D's.
