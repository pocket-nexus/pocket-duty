# Maps

A map of Pocket Duty is a plan written by hand in the terms of the reference's kit, built by the reference in Chrome, exported and cooked like the reference's own street. `maps/maps.json` lists the maps; the first is the reference's street, which has no file here.

| Map | Directory | What it is |
| --- | --- | --- |
| Market Street (`street`) | none: `web/src/world/layout.js`, the reference's | One long street, two alleys and a gate |
| Twin Yards (`yards`) | `maps/yards/` | Two walled yards, a long lane, a covered way and the street between |

## Where a map comes from

A map may take its structure from a place a player knows: which grounds there are, how many ways join them, where a way is long and open and where it is narrow and roofed. Structure is the only thing taken, and it is taken by looking, never by reading a file.

**What is taken.** The count and order of the grounds, the ways between them, their rough proportions, and which of them is open, walled or roofed. A person who knows the place it resembles can find their way.

**What is never taken.** No file of another game is opened by a tool of this repository, and nothing of such a file goes into the repository, a pack, a package or a page: **no vertex, no texture, no sound, no entity list, no file name, no map name, no name of that game or its series**. A layout holds numbers typed by its author and words of this repository.

**How a plan is written.**

1. Study the structure: play the place, or look at a picture of it from above. A study picture is the author's own working note. It stays under the ignored `.pocket-build/study/` and is never committed, attached or published.
2. Write the plan from that understanding, in metres, on the kit's own grid: `maps/<id>/layout.js`. Every measure is the author's choice within the kit (a building is one of the kit's buildings at a size it builds, a wall is the kit's wall). The plan's header draws it in ASCII so a reviewer reads it without a tool.
3. The look is this game's: the reference's materials, buildings, props, light and sky, the same as the street's. A plan states no surface of its own.
4. The name is ours: a title and one line in `maps/maps.json`, and an id of lowercase letters. The title names what is on the map (yards, a depot, a causeway), never the place it resembles.

**What a reviewer checks.** The diff of a new map is `maps/<id>/layout.js`, `maps/<id>/views.json` and one entry of `maps/maps.json`: text. No binary file, no picture, no pack. `git grep` for the other game's names finds nothing.

## What a layout states

`layout.js` exports the reference's terms (read `web/src/world/layout.js` for their fields) and the terms a map of ours adds. Coordinates are metres in level space: **+x east, +z south**, the perimeter wall a square of **±58 m**, one level of ground at y = 0.

| Export | Whose | What it is |
| --- | --- | --- |
| `STREET` | reference | The one road with its kerbs, along x = 0 from `zMin` to `zMax`. The reference barricades both ends, so the ends reach a wall or a building |
| `ALLEYS` | reference | Open ground with the junk the reference leaves in an alley: about **0.85 things a square metre**, a seventh of them with collision. Narrow ways only |
| `BUILDINGS` | reference | The kit's buildings: place, size, floors, wall, the sides to leave out. They stand at y = 0 |
| `GATE` | reference | The gate across the street, at `z` |
| `SET_PIECES` | reference | Stalls, barriers, sandbags, wrecks, palms, lamps, cables, laundry, rubble, tyres |
| `MAP` | ours | `id`, `title`, the level's turn in the world (`level.yaw`) and the starts: `[x, z, yaw, tag]`, the player's first, **four or more**. A yaw of 0 looks north (−z) |
| `GROUND` | ours | Open ground with no junk: the slab the reference lays under an alley, alone |
| `WALLS` | ours | Free walls from `a` to `b`. A wall with `y` starts above the ground: the lintel of a doorway |
| `COVERED` | ours | A way under a roof with a block over it, and its bulbs |
| `RAISED`, `RAMPS` | ours | Ground above the rest, and the wedges up to it |
| `COVER` | ours | Crates, stacks, blocks and drums, each with its collision |

`web/src/pocket/mapkit.js` builds the terms of ours with the reference's boxes, materials, props and collision proxies. A new term goes there, built from the kit, and is listed in this table.

## How a map reaches the reference

`web/` outside `web/src/pocket` is the reference, unmodified. Three files of `web/src/pocket` stand between it and a layout of ours:

- `vite.config.js`: the export's server. With `DUTY_MAP=<id>` it answers the reference's `./layout.js`, imported by `web/src/world/`, with `maps/<id>/layout.js`. The reference's ground, buildings and dressing then build our plan with no change to their files.
- `mapworld.js`: the reference's world system with another `init`: the same passes in the same order, then `mapkit.js`. It replaces what the reference keeps as constants of its own: the starts, the level's turn, the playable box.
- `mapnav.js`: the walk grid. The reference's grid sends one ray a cell down from above the level, so a cell under a roof holds the roof. For each underside the world states (a covered way, a lintel, the gate's passage) the reference's grid class is built again with its rays started under it. A wall thinner than a cell is closed as a band **half a cell beyond each face**, and the ground beyond the perimeter is closed.

The cook, the pack, the match and the devices have no code for a particular map: the starts, the walk grid, the collision world and the sight lists are sections of the map's pack.

## Add a map

1. `maps/<id>/layout.js`: the plan. Start from `maps/yards/layout.js`.
2. `maps/<id>/views.json`: eye points for `look`, `[{ "name", "pos": [x, y, z], "look": [x, y, z], "fov" }]` in level coordinates. One from above and one for each ground and way.
3. One entry in `maps/maps.json`: `{ "id", "title", "line" }`. The interface's list, the Rust table (`duty_interface::maps`) and the tools read this file.

```
bun tools/duty.ts look --map <id>      # pictures as the reference draws it → .pocket-build/validation/maps/<id>/reference (9 s)
bun tools/duty.ts export --map <id>    # DutyIR → .pocket-build/duty/maps/<id>/ir (20 s)
bun tools/duty.ts walk --map <id>      # the walk grid from the export: its parts, each start, walk.png. Exits 1 when a start is off the grid
bun tools/duty.ts cook --map <id>      # the PSP's pack → .pocket-build/duty/maps/<id>/psp30/duty.pack (6 to 7 minutes)
cargo run --release -p duty-sim --example grid -- .pocket-build/duty/maps/<id>/psp30/duty.pack
cargo run --release -p duty-sim --example match -- .pocket-build/duty/maps/<id>/psp30/duty.pack 10
bun tools/wgpu.ts shot --map <id> --words "mode=play auto=1"   # a frame on this computer's GPU
bun tools/wgpu.ts check                # the page in Chrome: every cooked map is chosen from the list and played
bun tools/psp.ts emu --map <id>        # the PSP program on it in PPSSPPHeadless
bun tools/duty.ts maps                 # which maps are exported and cooked
```

A map is ready when:

- `walk` reports **every start on the part that stays** and **no ground off it**. Red in `walk.png` is ground a player reaches and a soldier does not.
- The cook's `walk grid:` line reports **0 starts moved**.
- The pack is near the street's **30.5 MB**. A PSP holds one pack in **50.8 MB** with the program, the interface and the match beside it; Twin Yards is **32.3 MB**.
- A bench match finds its ways: `ways asked, found, not there, given up` with none `not there` and a share `given up` near the street's (32 of 402 in ten minutes).
- The `look` pictures show each ground and way as the plan's header draws it.

What costs triangles, from the first map: ground listed under `ALLEYS`, and buildings beyond the perimeter wall, where only what stands above the wall shows. With every open ground an alley and five large buildings beyond the wall, Twin Yards exported **2.42 million triangles** and cooked to **38.3 MB**; with the yards under `GROUND` and those buildings smaller, **1.68 million** and the size above.

## On the devices

A device holds **one map's pack at a time**. The interface lists the maps the device has a pack for (`maps` of the state); with one, the title's first row starts the match as before.

| Device | Where the packs are | A change of map |
| --- | --- | --- |
| Browser tab | A file or a manifest a map, named by the page (`<meta name="pocket-pack">` for the first, `<meta name="pocket-packs">` for the others) | The shell lets its world go, the page reads the other pack (`Duty::map_wanted`), the match starts when it is there |
| PSP | `duty.pack` and `<id>.pack` beside `EBOOT.PBP` | A pack's memory cannot be given back, so another map is another run: the program writes `map.txt` and starts itself again (`store::start_again`), into the match. Where the kernel will not start it, the screen says to start it again, and that start opens on the chosen map |
| Nintendo 3DS | `romfs:/duty.pack`, inside the `.3dsx` | None: the package holds the first map. The `.3dsx` is **31.1 MB** and the paired wire installs **32 MiB** at most, so a second pack does not fit in it |

A second map on a 3DS needs three things this repository does not have: the host opening `sdmc:/3ds/pocket-duty/<id>.pack` before `romfs:/duty.pack` (`n3ds/src/main.c`, where the pack is opened), a way to put a pack of 28 MB on the card (the wire sends one program), and a package of more than one file on Pocket Studio. The map's pack itself is `bun tools/duty.ts cook --map <id> --profile n3ds30`.
