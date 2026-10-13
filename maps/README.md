# Maps

A map of Pocket Duty is a plan drawn by hand, built by the reference's kit in Chrome, exported and cooked like the reference's own street.

**A map is a directory, and no file lists the maps.** `maps/street.json` names the reference's street. Every other map is `maps/<id>/`: its directory's name is its id, and tools, the interface and the Rust table (`duty_interface::maps`) find it by its `map.json`. A map's author writes only files under `maps/<id>/`:

| File | What it is |
| --- | --- |
| `map.json` | `{ "title", "line", "order" }`: the name in the map list, one line under it, and its place in the list (lower first; then by id) |
| `place.js` | The plan: floors and what stands on them |
| `look.js` | The look: the map's palette, its own surface functions, its sky and the cook's light |
| `views.json` | Eye points for `look`: `[{ "name", "pos": [east, north, up], "look": [east, north, up], "fov" }]`, one from above and nine a player knows the place by |

`maps/yards/` (Twin Yards) is the worked example of all four.

## Where a map comes from

A map may be a place a player knows, restated by us: **its floor plan, its proportions, its heights and its landmarks follow the place; every surface, every piece of geometry and every name is ours.**

**How a plan is drawn.** The author makes pictures of the place from above for their own study: its floors coloured by height over a grid of metres, and a note of its sizes. The plan is then **typed by hand against those pictures**, floor by floor, in the place's own coordinates (metres east and north of its middle, heights above its lowest floor): `maps/<id>/place.js`. A number in a plan is a number its author read and typed.

**What is never done.**

- **Nothing is converted.** No program turns a file of another game into geometry, and no tool of this repository reads one.
- **Nothing of such a file is in the repository, a pack, a package or a page**: no vertex, no texture, no texel, no sound, no entity list, no file name, no map name, no name of that game or its series.
- **The study stays out of every repository.** Its pictures and notes are the author's own working papers, kept outside the checkout; no committed file names where they are.
- **Nothing written or painted on the place's walls is taken**: no marking, sign or word. A map has none, or marks of our own invention.

**The look derives from the place's family and is made by our code.** A place has kinds of surface and a palette. `maps/<id>/look.js` states them:

- `SURFACES`: the map's own surface functions, each written with `surface(body, { metres, relief, like })` from `web/src/pocket/placelook.js`: the body of a GLSL function that writes a colour, a height and a roughness from a tile's coordinates, with the reference's noise at hand. The reference's material system bakes it; no picture is read.
- `LOOK.materials`: the map's palette keys, each `{ of, tint, scale }` over a surface of the map's own, of the kit's (**`courses`, `cobbles`, `grit`, `flutes`, `boards`, `water`**, which every map has) or of the reference's library (`plaster`, `concrete_floor`, `brick`, `metal_painted`, `corrugated`, `wood`, …).
- `LOOK.keys`: which key the kit uses for `ground`, `side`, `wall`, `band`, `cap`, `lid`, `wood`, `woodDark`, `iron`, `water`, `mark`, `apron`.
- `LOOK.windows`: `{ every }`, a shuttered window to so many metres of wall, or none.
- `LOOK.sky`: `{ hour, weather: { coverage, turbidity } }`: the sun's height and the sky.
- `LOOK.light`: `{ sky, bounce, lamp, reach }`: the cook's light for the place (below).

The proportions, the motifs, the doors and the crates are our own build, and the palette is shifted from the place's.

**How likeness is measured.** `likeness.py` lies in the study's directory, beside the letters (`a`, `b`, …), and out of the repository. An author runs it for their letter:

```
python3 -I <study>/likeness.py <letter> chart      # the place's walkable floor as text, a character a metre: heights in half metres
python3 -I <study>/likeness.py <letter> roofs      # what stands beside it: roofs and crate tops, in metres
python3 -I <study>/likeness.py <letter> at 45 -30  # every floor's height at a point
python3 -I <study>/likeness.py <letter> compare .pocket-build/duty/maps/<id>/ir   # the numbers, and likeness.png beside the study
```

`chart`, `roofs` and `at` are for reading: **read, then type; nothing generated.** `compare` takes the map's walk grid from its export and the place's floor a walker reaches from its starts, on cells of one metre: the share of the place's floor the map has, the share of the map's floor the place has, and the mean difference of height where both have floor, with the squares of ten metres where they differ. A place is like when **both shares are 85% or more and the mean difference is under 0.5 m**; Twin Yards measures **97.8%, 89.0% and 0.07 m**. The numbers do not see the views a player knows a place by: those are looked at.

**What a reviewer checks.** The diff of a new map is its four files under `maps/<id>/`: text. No binary file, no picture, no pack. `git grep` for the other game's names finds nothing.

## What a place states

`place.js` is read in the place's coordinates: **[east, north] in metres from the middle, heights in metres above the lowest floor**. `web/src/pocket/placekit.js` builds it; there is no street, no gate and no perimeter of a fixed size, and a place is as large as it is.

| Export | What it is |
| --- | --- |
| `PLACE` | `id` (the directory's name), `title` (as in `map.json`), `turn` (the level's turn in the world: which side the sun is on), `wall` (how high the walls stand over a floor), `starts`: `[east, north, facing in degrees from north toward east, tag]`, the player's first, **four or more** |
| `GROUND` | **The floors, as polygons** of `[east, north]`, level (`y`) or sloped (`y: { from, to }`, each `[east, north, up]`). `key` names its surface, `roof` puts a roof over it at that clear height (a covered way), `steps` draws steps over a slope, `walls` is its own wall height or `false` |
| `BLOCKS` | The tops of the blocks of houses beside the floors, where they are not `PLACE.wall` over the floor: **a floor's wall rises to the block beyond it**. A block lower than a floor gives that floor a parapet |
| `HOUSES` | **A building's face on a solid**: the reference's building (plinth, windows with frames and glass, shutters, balconies, a door, a roof) at any place, height and turn, in the map's stone: `{ at, size: [w, d], turn, y, floors, balconies, skipSides, … }`. Where a house stands at a floor's edge the kit builds no wall and no cap: the house's own face is the wall |
| `WALLS` | A wall along any line: `{ line, y, h, t, key }` with `holes` cut in it, each `{ at, w, h, sill, arch, frame, glass }`: a doorway (`sill` 0), a window (`sill`), an arch (`arch` 0…1), `frame: true` for a frame of boards, `glass: true` for a pane a player does not pass |
| `DOORS` | Leaves standing open in a doorway: `{ at, facing, w, h, open }`; two leaves, or `leaves: 1` with `hinge: -1 or 1`; `kind: 'roller'` with `raised` (0…1) is a roller door of corrugated sheet |
| `MASSES` | A solid of any footprint from `y` to `top` |
| `WATER` | A surface at a height over the floor that is its bed: `{ at, y, key }`. It carries no one and stops no one |
| `LADDERS` | `{ at, facing, h }`: rails and rungs, listed in the scene (see below for who climbs) |
| `CRATES` | `[east, north, side, turn, up]`: crates of boards with a frame and braces, on the floor or on one another |
| `MARKS` | A site's mark of our own on the floor: `{ at, size, key }` |
| `LAMPS` | `[east, north, up]`: the reference's bare bulb, and its light in the cook's pass |
| `PROPS` | The reference's props: `['palm', east, north, scale]`, or `[id, east, north, turn, scale]` for any prototype |

**The floors carry the plan.** Along every edge of a floor the kit builds nothing where another floor of the same height or higher lies beyond it, a face down to the lower floor where one lies beyond it, and **a wall where nothing lies beyond it**, with the band and a cap on its top and shuttered windows along it. Where there is no floor there is a block of houses, capped level with the nearest floor's walls. So a plan that has its floors right is closed, and its walls stand where the place's do.

**What a player and a soldier can do.** A player steps up 0.45 m and **climbs a ledge between 0.5 m and 1.85 m** (the jump held, pushing ahead): a crate, a window's sill, a low wall. A soldier walks the walk grid: a step between two nodes is **0.45 m at most**, so a slope a soldier takes rises **0.56 m a metre at most**.

- **Stairs at their true steepness.** A sloped floor with `steps: true` that is steeper than that keeps its steepness: its treads carry the player, and its nodes carry the stair's mark in the walk grid (flag 4), where the cook and the match let a step be 0.9 m. No stretched copy is needed up to **1.1 m a metre**.
- **A doorway** narrower than three cells (2.4 m) keeps a line through its middle open in the walk grid, so soldiers go through doors a player goes through. A window is never a soldier's way.
- **A ladder is drawn and listed, and nobody climbs it yet.** The match has no ladder: a player gets up only what his own climb reaches, and a soldier never uses one. Where a ladder is the only way up, give the player ledges of 1.85 m at most beside it, or wait for the walk grid's second level, which carries ladders with it.
- **Glass** stops a player and a bullet's surface is `glass`; a hole without `glass` is open.

**One floor a place, for now.** Where two floors overlap, the higher one is the ground, and the walk grid holds one height a node. A room over a way, a roof a player walks on and a bridge with a way under it need the second level.

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

In this order, with Twin Yards as the pattern:

1. `maps/<id>/map.json`, and `place.js` with `PLACE` and the first floors typed from `chart`.
2. `bun tools/duty.ts look --map <id>`: nine seconds, pictures of what stands so far.
3. The rest of the floors, then `export`, `walk` and `compare` until the numbers pass. A full round is under a minute.
4. `look.js`: the place's surfaces and palette, its sky, its light.
5. `BLOCKS` and crates from `roofs`, then walls, doors, houses and lamps, looking after each.
6. `cook`, then the views in the game's renderer, the browser's check and the emulators.

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

**A map's picture from above** (the corner of the PSP's and the browser's screen, the 3DS's lower screen) is the pack `<id>-map` in the same list, written by `tools/packs.ts` from that profile's own cook; the interface compiles no picture, so a map that arrives as pieces arrives with its picture. A map's title and line are compiled into the interface from the maps (`tools/maps.ts`): a map the interface was not built with is not in the list.

Checked in Chrome, PPSSPPHeadless and Azahar. On an Old 3DS (2026-10-13, the build before the map change collected the interface's cycles): the LAN delivery sent 41 pieces of 34.7 MiB in 190 s and nothing in 5.9 s when the card had them, and nine map changes ended running with the linear memory and the VRAM back to the same byte. **A PSP opening pieces from a Memory Stick, and a PSP starting again on another map, have run on no console.**
