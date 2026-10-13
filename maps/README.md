# Maps

A map of Pocket Duty is a plan drawn by hand, built by the reference's kit in Chrome, exported and cooked like the reference's own street. `maps/maps.json` lists the maps; the first is the reference's street, which has no file here.

| Map | File | What it is |
| --- | --- | --- |
| Market Street (`street`) | none: `web/src/world/layout.js`, the reference's | One long street, two alleys and a gate |
| Twin Yards (`yards`) | `maps/yards/place.js` | Two sites, a long lane with a pit, a middle lane, covered ways on two heights, a high start |

## Where a map comes from

A map may be a place a player knows, restated by us: **its floor plan, its proportions, its heights and its landmarks follow the place; every surface, every piece of geometry and every name is ours.**

**How a plan is drawn.** The author makes pictures of the place from above for their own study: its floors coloured by height over a grid of metres, and a note of its sizes. The plan is then **typed by hand against those pictures**, floor by floor, in the place's own coordinates (metres east and north of its middle, heights above its lowest floor): `maps/<id>/place.js`. A number in a plan is a number its author read and typed.

**What is never done.**

- **Nothing is converted.** No program turns a file of another game into geometry, and no tool of this repository reads one.
- **Nothing of such a file is in the repository, a pack, a package or a page**: no vertex, no texture, no texel, no sound, no entity list, no file name, no map name, no name of that game or its series.
- **The study stays out of every repository.** Its pictures and notes are the author's own working papers, kept outside the checkout; no committed file names where they are.
- **Nothing written or painted on the place's walls is taken**: no marking, sign or word. A map has none, or marks of our own invention.

**The look derives from the place's family and is made by our code.** A place has kinds of surface and a palette: for Twin Yards pale stone in courses, a band along the tops of walls, cream slab, cobbles in sand, orange-tan sand, weathered boards, under a clear sky. `web/src/pocket/placelook.js` draws each surface with a function of ours, baked by the reference's material system; the proportions of the courses, the band's motif, the doors and the crates are our own build, and the palette is shifted from the place's. No picture is read to make one.

**How likeness is measured.** Beside the study, and out of the repository, a script compares the map's walk grid with the place's floor on cells of one metre: the share of the place's walkable floor the map has, the share of the map's floor the place has, and the mean difference of height where both have floor. A place is like when **both shares are 85% or more and the mean difference is under 0.5 m**; Twin Yards measures **97.8%, 89.0% and 0.07 m**. The numbers do not see the views a player knows a place by: those are looked at (`bun tools/duty.ts look`).

**What a reviewer checks.** The diff of a new map is `maps/<id>/place.js`, `maps/<id>/views.json` and one entry of `maps/maps.json`: text. No binary file, no picture, no pack. `git grep` for the other game's names finds nothing.

## What a place states

`place.js` is read in the place's coordinates: **[east, north] in metres from the middle, heights in metres above the lowest floor**. `web/src/pocket/placekit.js` builds it; there is no street, no gate and no perimeter of a fixed size, and a place is as large as it is.

| Export | What it is |
| --- | --- |
| `PLACE` | `id`, `title`, `turn` (the level's turn in the world: which side the sun is on), `wall` (how high the walls stand over a floor), `starts`: `[east, north, facing in degrees from north toward east, tag]`, the player's first, **four or more** |
| `LOOK` | `materials`: palette keys of the map, each `{ of, tint, scale }` over a surface of ours (`courses`, `cobbles`, `grit`, `flutes`, `boards`) or of the reference's library; `keys`: which of them the kit uses for ground, walls, the band, wood; `windows`; `sky`: the hour and the weather; `light`: the cook's light for the place (below) |
| `GROUND` | **The floors, as polygons**, level (`y`) or sloped (`y: { from, to }`). `roof` puts a roof over one (a covered way), `steps` draws steps over a slope, `key` names its surface |
| `WALLS` | A wall along any line, of any height and thickness, with `holes` cut in it: doorways, windows (`sill`), arches (`arch`) |
| `BLOCKS` | The tops of the blocks of houses beside the floors, where they are not `PLACE.wall` over the floor: **a floor's wall rises to the block beyond it**, and the block's cap lies at that height. A block lower than a floor gives that floor a parapet |
| `MASSES` | A solid of any footprint from `y` to `top` |
| `HOUSES` | The reference's building, at any place, height and turn, in the map's stone |
| `DOORS` | Two leaves of boards standing open in a doorway |
| `CRATES` | `[east, north, side, turn, up]`: crates of boards with a frame and braces, on the floor or on one another |
| `LAMPS` | `[east, north, up]`: the reference's bare bulb on its flex, and its light in the cook's pass. Under a roof, a lamp hangs from it |
| `PROPS` | The reference's props: palms, and any prototype by its id |

**The floors carry the plan.** Along every edge of a floor the kit builds nothing where another floor of the same height or higher lies beyond it, a face down to the lower floor where one lies beyond it, and **a wall where nothing lies beyond it**, with the band and a cap on its top and shuttered windows along it. Where there is no floor there is a block of houses, capped level with the nearest floor's walls. So a plan that has its floors right is closed, and its walls stand where the place's do.

**One floor a place, for now.** Where two floors overlap, the higher one is the ground, and the walk grid holds one height a node. A room over a way, a roof a player walks on and a bridge with a way under it need a floor marked as standing over another and a grid with more than one height; the kit's terms leave room for both and build neither.

A map in the reference's own terms (a street, alleys, buildings: `maps/<id>/layout.js`, built by `web/src/pocket/mapworld.js` and `mapkit.js`) is still read; no map uses it now.

## How a map reaches the reference

`web/` outside `web/src/pocket` is the reference, unmodified. These files of `web/src/pocket` stand between it and a place:

- `vite.config.js`: the export's server. With `DUTY_MAP=<id>` the page's `@map/place` is `maps/<id>/place.js`, and the reference's own layout, which its world modules import, is a street of no length (`nolayout.js`).
- `placeworld.js`: the reference's world system with another `init`: its prop prototypes and its lights, then the place from `placekit.js`. The reference's ground, gate, perimeter and street dressing are not run.
- `placekit.js`: the kit above, made from the reference's boxes, wall panels with real openings, prisms, buildings, props and collision proxies.
- `placelook.js`: the map's surfaces and palette keys, added to the reference's material library and palette while the page loads, and its sky.
- `mapnav.js`: the walk grid. The reference's grid sends one ray a cell down from above the level, so a cell under a roof holds the roof. For each underside a place states (a covered way, a doorway's lintel) the reference's grid class is built again with its rays started under it, and a wall thinner than a cell is closed as a band.

The cook, the pack, the match and the devices have no code for a particular map. One thing in the cook follows a place: **its floors lie at any height and have surfaces of its own**, so the export's scene names the floors' surfaces (`ground` in `scene.json`), and the ground's shadow picture is taken at each floor's own height (`Lighting::floors`). Without that entry, as for the street, the cook is unchanged.

**A place states its own light** (`LOOK.light`, `light` in `scene.json`), which the cook adds to the sun and the sky's fill at every corner (`Lighting::added`):

- `sky`: the sky's fill times this, colour by colour. Shade under a blue sky is blue; a place of warm stone states a warmer one.
- `bounce`: what its sunlit ground sends back, as the ground's colour before light times a strength. A face takes it by the share of its half space the ground fills (**(1 − n.y) / 2**: a wall half, a floor none) times how open it is to the sky.
- `lamps`: each lamp of `LAMPS`, with one ray to it for its shadow, the square of the distance, and a reach where its light is gone.

The street's scene states no light, and its pack from this cook is the same byte for byte as before the cook had one.

## Add a map

1. `maps/<id>/place.js`: the plan. Start from `maps/yards/place.js`.
2. `maps/<id>/views.json`: eye points for `look`, `[{ "name", "pos": [east, north, up], "look": [east, north, up], "fov" }]`. One from above and nine a player knows the place by.
3. One entry in `maps/maps.json`: `{ "id", "title", "line" }`. The interface's list, the Rust table (`duty_interface::maps`) and the tools read this file.

```
bun tools/duty.ts look --map <id>      # pictures as the reference draws it → .pocket-build/validation/maps/<id>/reference (9 s)
bun tools/duty.ts export --map <id>    # DutyIR → .pocket-build/duty/maps/<id>/ir (20 s)
bun tools/duty.ts walk --map <id>      # the walk grid from the export: its parts, each start, walk.png. Exits 1 when a start is off the grid
bun tools/duty.ts cook --map <id>      # the PSP's pack → .pocket-build/duty/maps/<id>/psp30/duty.pack (100 s for Twin Yards)
cargo run --release -p duty-sim --example grid -- .pocket-build/duty/maps/<id>/psp30/duty.pack
cargo run --release -p duty-sim --example match -- .pocket-build/duty/maps/<id>/psp30/duty.pack 10
bun tools/wgpu.ts shot --map <id> --words "mode=play auto=1"   # a frame on this computer's GPU
bun tools/wgpu.ts check                # the page in Chrome: every cooked map is chosen from the list and played
bun tools/psp.ts emu --map <id>        # the PSP program on it in PPSSPPHeadless
bun tools/duty.ts cook --map <id> --profile n3ds30 && bun tools/n3ds.ts build --map <id> && bun tools/n3ds.ts emu --map <id>   # a .3dsx of it, in Azahar
bun tools/duty.ts maps                 # which maps are exported and cooked
```

A map is ready when:

- The likeness script, beside the study, reports **both shares at 85% or more and a mean height difference under 0.5 m**.
- `walk` reports **every start on the part that stays** and **no ground off it**. Red in `walk.png` is ground a player reaches and a soldier does not.
- The cook's `walk grid:` line reports **0 starts moved**.
- The pack is **under 34 MB**. A PSP holds one pack in **50.8 MB** with the program, the interface and the match beside it; the street's is 30.5 MB, and Twin Yards, with 39 000 triangles in its export, is **9.4 MB**.
- A bench match finds its ways: `ways asked, found, not there, given up` with none `not there`.
- The `look` pictures show the views a player knows the place by.

Two rules of the walk grid shape a plan: **a step between two floors is 0.45 m at most** (a ledge of half a metre is not walked), and **a slope rises 0.5 m a metre at most** (a stair is a slope with `steps` drawn over it).

## On the devices

A device holds **one map's pack at a time**, and reads it through one list of every map's pack, `pocket.packs`: the packs cut into **pieces named by their bytes** (PocketJS's `pocket3d-packs`; `tools/packs.ts` cuts them and brings them to a device, the pieces it lacks). The interface lists the maps the device has a pack for (`maps` of the state); with one, the title's first row starts the match as before.

| Device | Where the pieces are | A change of map |
| --- | --- | --- |
| Browser tab | `pack/<hash>.packs` and its `.bin` files in the page's directory; the page names `<manifest>#<map>` | The shell lets its world go, the page reads the other pack (`Duty::map_wanted`), the match starts when it is there |
| PSP | `pocket.packs` and the pieces beside `EBOOT.PBP` (on PSPLINK: `host0:/duty/`) | A pack's memory cannot be given back, so another map is another run: the program writes `map.txt` and starts itself again (`store::start_again`), into the match. Where the kernel will not start it, the screen says to start it again, and that start opens on the chosen map |
| Nintendo 3DS | `sdmc:/3ds/pocket-duty/packs/` on the memory card, then `romfs:/packs/` inside the `.3dsx` | In the running program: it frees the pack's sections and its textures and reads the other map's (`dt_wanted`, `dt_drop`, `load` in `n3ds/src/main.c`) |

**The 3DS package is a zip** of `3ds/pocket-duty/pocket-duty.3dsx` and `3ds/pocket-duty/packs/`, unpacked onto the root of the memory card: the list has every map. The `.3dsx` alone holds the first map in its ROMFS (`bun tools/n3ds.ts build --embed <id>`, `--embed none` for the zip's), so it stays under the **32 MiB** the paired wire installs and a program installed over the wire plays; `bun tools/n3ds.ts bring` then sends the running program the pieces its card lacks over the LAN.

Checked in Chrome, PPSSPPHeadless and Azahar. **No console has read pieces or changed map this way**: the LAN delivery to a 3DS, the map change on a 3DS's memory, and a PSP opening pieces from a Memory Stick are unchecked.
