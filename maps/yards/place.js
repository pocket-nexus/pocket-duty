/**
 * TWIN YARDS, a place of Pocket Duty.
 *
 * Drawn by hand, floor by floor, in the place's own coordinates: metres east and north of its middle,
 * heights in metres above its lowest floor (maps/README.md). North is up in the plan below.
 *
 *      -45        -25      -10   0    +10    +20        +35     +50
 *  +45   +--------+                                      +--------+
 *        |  WEST  |                  +-------------------+  back  |
 *  +30   |  SITE  +--ramp--+ low     |   EAST SITE (5.5) | (6.5)  |
 *        | (3-4)  | doors  | START   +--+ short +--------+  ramp  |
 *  +18   +--+-----+--------+ (0) ----+  | stair | low    |        |
 *           |covered|    | M |          +-------+ yard   | LONG   |
 *    0      | ways  +----+ I +- raised walk (3) -+       | LANE   |
 *           | (4)   stair| D |                           | (3)    |
 *  -14      +--+----+    | D +---------------------------+--+     |
 *              |         | L |        doors |  |   side  |  | pit |
 *  -28   +-----+ west    | E +--------------+  +---------+  | (0) |
 *        | lane (3)   +--+---+   outside    |               +-----+
 *  -41   +--ramp--+   |      |   (3)        |
 *        | HIGH START (6.5)  slope +--------+
 *  -60   +-------------------+-----+
 */

/** The map's name, its turn in the world (which side the sun is on), its walls' height, the starts. */
export const PLACE = {
  id: 'yards',
  title: 'Twin Yards',
  turn: 0.5877,
  wall: 6.5,
  // [east, north, facing in degrees from north toward east, tag]. The first is the player's.
  starts: [
    [-9, -54, 60, 'high start'],
    [-16, -52, 60, 'high start'],
    [-24, -55, 40, 'high start'],
    [-5, -57, 70, 'high start'],
    [0, -17, 0, 'top of the middle'],
    [25.5, -34, 0, 'outside the doors'],
    [-35, -27, 0, 'west lane'],
    [31, -9, 0, 'long lane'],
    [-33, 34, 180, 'west site'],
    [30, 32, 180, 'east site'],
    [44, 41, 180, 'east site, back'],
    [5, 22, 180, 'low start'],
    [-3, 22, 180, 'low start'],
    [12, 21, 200, 'low start'],
  ],
};

/** The look: surfaces of the place's own family, made by our code (web/src/pocket/placelook.js). */
export const LOOK = {
  materials: {
    yd_stone: { of: 'courses', tint: 0xf4eadc },
    yd_stone_warm: { of: 'courses', tint: 0xe6cfae },
    yd_frieze: { of: 'flutes', tint: 0xecdfcc },
    yd_roof: { of: 'plaster', tint: 0xc9b089, scale: 3.2 },
    yd_cobble: { of: 'cobbles', tint: 0xf2e8d8 },
    yd_slab: { of: 'concrete_floor', tint: 0xe9cfa2, surface: 'concrete' },
    yd_sand: { of: 'grit', tint: 0xf6dfbc },
    yd_wood: { of: 'boards', tint: 0xeadcc4 },
    yd_wood_dark: { of: 'boards', tint: 0xa58c6c },
  },
  keys: { ground: 'yd_cobble', side: 'yd_stone', wall: 'yd_stone', band: 'yd_frieze', cap: 'yd_stone_warm', lid: 'yd_roof', wood: 'yd_wood', woodDark: 'yd_wood_dark', iron: 'metal_dark', apron: 'yd_sand' },
  // A window to so many metres of a house's wall.
  windows: { every: 7.5 },
  sky: { hour: 14.2, weather: { coverage: 0.1, turbidity: 2.2 } },
  // The cook's light for this place: the sky's fill warmed, what the sunlit sand sends back, its lamps.
  light: { sky: [1.55, 1.2, 0.85], bounce: [0.62, 0.46, 0.28], lamp: [9, 5.5, 2.4], reach: 11 },
};

const SLAB = 'yd_slab', SAND = 'yd_sand';
const COVER = 3.2;
// The heights most floors lie at: the common level, the west side's upper level, the east site.
const L = 3.25, U = 4.05, E = 5.7;

/** The floors: polygons of [east, north], level (`y`) or sloped (`y: { from, to }`, each [east, north, up]). */
export const GROUND = [
  // ------------------------------------------------------------------ the west site and its yard
  { at: [[-44, 40], [-37, 40], [-37, 41], [-38, 41], [-38, 44], [-41, 44], [-41, 47], [-43, 47], [-43, 46], [-44, 46]], y: U, key: SLAB },
  { at: [[-44, 29], [-34, 29], [-34, 40], [-44, 40]], y: U, key: SLAB },
  { at: [[-34, 29], [-27, 29], [-27, 40], [-34, 40]], y: L, key: SLAB },
  { at: [[-27, 28], [-23, 28], [-23, 30], [-24, 30], [-24, 39], [-27, 39]], y: 3.6, key: SLAB },
  { at: [[-44, 26], [-41, 26], [-41, 29], [-44, 29]], y: L },
  { at: [[-37, 26], [-35, 26], [-35, 29], [-37, 29]], y: L },
  { at: [[-41, 26], [-37, 26], [-37, 29], [-41, 29]], y: { from: [-39, 29, U], to: [-39, 26.5, L] } },
  { at: [[-44, 14], [-24, 14], [-24, 19], [-22, 19], [-22, 20], [-20, 20], [-20, 21], [-18, 21], [-18, 25], [-24, 25], [-24, 28], [-27, 28], [-27, 29], [-35, 29], [-35, 26], [-44, 26], [-44, 25], [-45, 25], [-45, 19], [-44, 19]], y: L },
  // its doors' way down to the low start, and the ramp north of it
  { at: [[-18, 18], [-15, 18], [-15, 25], [-18, 25]], y: { from: [-18, 21, L], to: [-15, 21, 1.63] } },
  { at: [[-15, 19], [-10, 19], [-10, 25], [-15, 25]], y: 1.63 },
  { at: [[-23, 25], [-19, 25], [-19, 30], [-23, 30]], y: 3.6 },
  { at: [[-19, 25], [-10, 25], [-10, 29], [-19, 29]], y: { from: [-19, 27, 3.5], to: [-10, 27, 0] } },
  { at: [[-20, 29], [-13, 29], [-12, 31], [-12, 34], [-13, 35], [-16, 33], [-20, 32]], y: { from: [-20, 31, U], to: [-12, 31, 1] } },
  { at: [[-12, 27], [3, 27], [3, 30], [-2, 30], [-2, 31], [-12, 31]], y: 0.4 },
  { at: [[-12, 31], [-5, 31], [-5, 33], [-12, 33]], y: { from: [-8, 31, 0.4], to: [-8, 32.5, 1] } },
  // ------------------------------------------------------------------ the low start and the middle lane
  { at: [[-15, 18], [-5, 18], [-5, 17], [11, 17], [11, 18], [16, 18], [16, 25], [17, 25], [17, 31], [11, 31], [11, 25], [3, 25], [3, 27], [-10, 27], [-10, 19], [-15, 19]], y: 0 },
  { at: [[-4, -3], [3, -3], [3, 17], [-4, 17]], y: 0 },
  { at: [[-4, -7], [3, -7], [3, -3], [-4, -3]], y: { from: [0, -3, 0], to: [0, -7, 2.24] }, steps: true },
  { at: [[-4, -10], [3, -10], [3, -7], [-4, -7]], y: 2.24 },
  { at: [[-4, -13], [3, -13], [3, -10], [-4, -10]], y: { from: [0, -10, 2.24], to: [0, -13, L] } },
  { at: [[-4, -21], [7, -21], [7, -13], [-4, -13]], y: L },
  // the raised walk beside the middle, the short stair, the walk to the east site
  { at: [[3, -13], [7, -13], [7, 2], [3, 2]], y: L, key: SLAB },
  { at: [[3, 2], [20, 2], [20, 5], [23, 5], [23, 7], [11, 7], [11, 6], [10, 6], [10, 8], [3, 8]], y: L, key: SLAB },
  { at: [[16, 7], [19, 7], [19, 12], [16, 12]], y: { from: [17.5, 7, L], to: [17.5, 12, E] }, key: SLAB, steps: true },
  { at: [[19, 7], [23, 7], [23, 12], [19, 12]], y: L, key: SLAB },
  { at: [[16, 12], [23, 12], [23, 28], [17, 28], [17, 25], [16, 25]], y: E, key: SLAB },
  // ------------------------------------------------------------------ the east site
  { at: [[17, 28], [41, 28], [41, 38], [34, 38], [34, 37], [29, 37], [29, 36], [22, 36], [22, 37], [16, 37], [16, 31], [17, 31]], y: E, key: SLAB },
  { at: [[34, 36], [41, 36], [41, 39], [34, 39]], y: { from: [37, 36.5, E], to: [37, 38.5, 6.5] }, key: SLAB },
  { at: [[35, 38], [49, 38], [49, 45], [37, 45], [37, 43], [35, 43]], y: 6.5, key: SLAB },
  { at: [[42, 25], [50, 25], [50, 38], [42, 38]], y: { from: [45, 25, L], to: [45, 38, 6.5] }, key: SAND },
  // the low yard under the site, and its ways up
  { at: [[23, 18], [26, 18], [26, 20], [32, 20], [32, 25], [23, 25]], y: 1, key: SAND },
  { at: [[26, 18], [33, 18], [33, 20], [26, 20]], y: 2.6, key: SAND },
  { at: [[28, 25], [35, 25], [35, 27], [28, 27]], y: { from: [28, 26, 1.25], to: [34, 26, L] }, key: SAND },
  { at: [[29, 20], [33, 20], [33, 25], [29, 25]], y: { from: [29, 22, 1], to: [33, 22, L] }, key: SAND },
  // ------------------------------------------------------------------ the long lane and the pit
  { at: [[33, 18], [49, 18], [49, 25], [33, 25]], y: L, key: SAND },
  { at: [[42, -5], [50, -5], [50, 19], [42, 19]], y: L, key: SAND },
  { at: [[49, 20], [54, 20], [54, 25], [49, 25]], y: 3.66, key: SAND },
  { at: [[50, 12], [55, 12], [55, 20], [50, 20]], y: { from: [52, 20, 3.66], to: [52, 13, 4.8] }, key: SAND },
  { at: [[22, -14], [50, -14], [50, -5], [48, -5], [48, -3], [29, -3], [29, -2], [25, -2], [25, -4], [23, -4], [23, -5], [22, -5]], y: L, key: SAND },
  { at: [[46, -13], [51, -13], [51, -10], [46, -10]], y: { from: [46, -11.5, L], to: [51, -11.5, 4.88] }, key: SAND },
  { at: [[50, -25], [55, -25], [55, -6], [50, -6]], y: 4.88, key: SAND },
  { at: [[42, -21], [48, -21], [48, -14], [42, -14]], y: 1.12, key: SAND },
  { at: [[48, -27], [50, -27], [50, -14], [48, -14]], y: { from: [49, -14, L], to: [49, -27, -1.63] }, key: SAND },
  { at: [[42, -27], [48, -27], [48, -21], [42, -21]], y: { from: [45, -21, 1.12], to: [45, -27, -1.63] }, key: SAND },
  { at: [[42, -35], [50, -35], [50, -27], [42, -27]], y: -1.63, key: SAND },
  // the doors' covered way from the south, and the side yard beside it
  { at: [[22, -28], [29, -28], [29, -14], [22, -14]], y: L, roof: COVER },
  { at: [[34, -28], [42, -28], [42, -14], [34, -14]], y: L, key: SAND },
  { at: [[37, -30], [39, -30], [39, -28], [37, -28]], y: L, key: SAND },
  // ------------------------------------------------------------------ the south: outside, the crossing, the high start
  { at: [[-10, -27], [-10, -21], [13, -21], [13, -22], [20, -22], [20, -28], [11, -28], [11, -27], [1, -27], [1, -28], [0, -28], [0, -29], [-6, -29], [-6, -28], [-7, -28], [-7, -27]], y: L },
  { at: [[-4, -47], [0, -47], [0, -29], [-4, -29]], y: L },
  { at: [[-4, -49], [20, -49], [20, -46], [-4, -46]], y: L },
  { at: [[13, -46], [20, -46], [20, -43], [29, -43], [29, -27], [20, -27], [20, -28], [13, -28]], y: L, key: SAND },
  { at: [[10, -57], [20, -57], [20, -49], [10, -49]], y: L, key: SAND },
  { at: [[-2, -60], [4, -60], [4, -50], [-2, -50]], y: { from: [-2, -55, 6.5], to: [4, -55, 4.67] }, key: SAND },
  { at: [[4, -60], [8, -60], [8, -50], [4, -50]], y: 4.67, key: SAND },
  { at: [[8, -57], [11, -57], [11, -50], [8, -50]], y: { from: [8, -54, 4.67], to: [11, -54, L] }, key: SAND },
  { at: [[-48, -60], [-2, -60], [-2, -50], [-15, -50], [-15, -41], [-34, -41], [-34, -43], [-36, -43], [-36, -49], [-43, -49], [-43, -47], [-48, -47]], y: 6.5, key: SAND },
  { at: [[-33, -41], [-26, -41], [-26, -37], [-33, -37]], y: 6.5, key: SAND },
  // the ramp from the high start down to the west lane
  { at: [[-43, -49], [-36, -49], [-36, -46], [-43, -46]], y: { from: [-40, -49, 6.5], to: [-40, -46, 5.6] } },
  { at: [[-43, -46], [-36, -46], [-36, -42], [-43, -42]], y: 5.6 },
  { at: [[-43, -42], [-36, -42], [-36, -37], [-43, -37]], y: { from: [-40, -42, 5.6], to: [-40, -37, 3.6] }, steps: true },
  { at: [[-43, -37], [-36, -37], [-36, -33], [-43, -33]], y: { from: [-40, -37, 3.6], to: [-40, -34, L] } },
  { at: [[-43, -33], [-31, -33], [-31, -32], [-27, -32], [-27, -30], [-26, -30], [-26, -22], [-43, -22]], y: L },
  { at: [[-36, -38], [-29, -38], [-29, -32], [-31, -32], [-31, -33], [-36, -33]], y: 3.6 },
  // ------------------------------------------------------------------ the west side's covered ways, on two heights
  { at: [[-43, -22], [-26, -22], [-26, -20], [-43, -20]], y: { from: [-35, -20, U], to: [-35, -22, L] }, roof: COVER },
  { at: [[-43, -20], [-26, -20], [-26, -16], [-37, -16], [-38, -17], [-38, -18], [-40, -18], [-40, -19], [-43, -19]], y: U, roof: COVER },
  { at: [[-35, -16], [-31, -16], [-31, -9], [-35, -9]], y: U, roof: COVER },
  { at: [[-36, -9], [-29, -9], [-29, -8], [-22, -8], [-22, -4], [-37, -4], [-37, -8], [-36, -8]], y: U, roof: COVER },
  { at: [[-46, -4], [-24, -4], [-24, -1], [-46, -1]], y: U, roof: COVER },
  { at: [[-43, -1], [-32, -1], [-32, L], [-43, L]], y: U, roof: COVER },
  { at: [[-43, L], [-39, L], [-39, 12], [-43, 12]], y: U, roof: COVER },
  { at: [[-44, 12], [-38, 12], [-38, 14], [-44, 14]], y: { from: [-41, 14, L], to: [-41, 12, U] } },
  { at: [[-36, 14], [-36, 12], [-35, 12], [-35, 7], [-31, 7], [-31, 10], [-29, 11], [-28, 12], [-27, 12], [-27, 14]], y: L },
  // the stair between the two heights, and the lower way out to the middle
  { at: [[-22, -7], [-17, -7], [-17, -4], [-22, -4]], y: { from: [-22, -5.5, U], to: [-17, -5.5, 1.5] }, roof: COVER + 2.5, steps: true },
  { at: [[-18, -7], [-16, -7], [-16, -3], [-19, -3], [-19, -4], [-18, -4]], y: 1.5, roof: COVER },
  { at: [[-19, -3], [-16, -3], [-16, -1], [-19, -1]], y: { from: [-17.5, -3, 1.5], to: [-17.5, -1, 0.4] }, roof: COVER },
  { at: [[-22, -1], [-13, -1], [-13, 1], [-11, 1], [-11, -1], [-4, -1], [-4, 6], [-18, 6], [-18, U], [-22, U]], y: 0.4, roof: COVER },
];

/** Walls along a line, with what is cut in them: `holes` of `{ at, w, h, sill, arch }`, `at` in metres from the first point. */
export const WALLS = [
  // the middle lane's doorway to the low start
  { line: [[-4, 16.6], [3, 16.6]], h: 4.6, holes: [{ at: 3.5, w: 3.4, h: 3.3, arch: 0.5 }] },
  // the west site's window opening, in the wall between its ledge and the ramp's head
  { line: [[-23.2, 25], [-23.2, 30]], h: 4.6, holes: [{ at: 3.5, w: 2.4, h: 1.7, sill: 1.1 }] },
  // the west site's doorway
  { line: [[-18.2, 21], [-18.2, 25]], h: 4.6, holes: [{ at: 2, w: 3, h: 3.2, arch: 0.5 }] },
  // the doors of the long lane, at both ends of their covered way
  { line: [[22, -14.2], [29, -14.2]], h: 4.6, holes: [{ at: 3.5, w: 3.6, h: 3.1, arch: 0.4 }] },
  { line: [[22, -25], [29, -25]], h: 4.6, holes: [{ at: 3.5, w: 3.6, h: 3.1, arch: 0.4 }] },
  // the raised walk's parapet over the middle lane, and the rail over the pit
  { line: [[3.3, -13], [3.3, 7]], y: L, h: 0.8, t: 0.4, crown: false },
  { line: [[41.7, -27], [41.7, -14.5]], y: L, h: 0.8, t: 0.4, crown: false },
  // the wall that parts the high start from the yard west of it
  { line: [[-14, -60], [-14, -50]], h: 5.5, t: 1.4 },
];


/** Doors of boards standing open: `facing` is the way the leaves swing, in degrees from north toward east. */
export const DOORS = [
  { at: [-0.5, 17], facing: 0, w: 3.4, h: 3.2 },
  { at: [-17.8, 23], facing: 90, w: 3, h: 3.1 },
  { at: [25.5, -13.8], facing: 0, w: 3.6, h: 3 },
  { at: [25.5, -24.6], facing: 0, w: 3.6, h: 3 },
];

/** Crates: [east, north, side, turn in degrees, up]. Without `up` a crate stands on the floor. */
export const CRATES = [
  // the west site: on its platform, on its floor, the stack by the yard
  [-43, 39, 2, 0], [-33, 39, 1.75, 0], [-32, 31, 2.2, 0], [-43, 30, 2, 0], [-42.4, 28, 1.7, 10],
  [-36.4, 27.5, 2.7, 0], [-27, 33, 1.75, 5], [-25.2, 34.6, 1.8, 0], [-25.2, 34.6, 1.3, 15, 3.6 + 1.8],
  // the low start: the tall stack, the two by it
  [5, 24, 2, 0], [5, 24, 2, 12, 2], [6.9, 23.6, 1.6, 0], [10.5, 24.5, 2, 0], [7, 18.2, 2.3, 0],
  // the middle lane's crate, the lower way's
  [1.5, 2.6, 2.1, 0], [-20, 5, 1.6, 0],
  // the east site and its ramp
  [37, 28.8, 1.4, 0], [41, 31, 1.4, 0], [41, 29.5, 1.3, 8], [39.3, 30.4, 1.5, 12], [39.3, 30.4, 1.2, 30, E + 1.5], [43, 27, 1.8, 0],
  // the long lane: the big stack by the doors, the doors' way, the side yard
  [30.6, -4.4, 2.75, 0], [33, -4.3, 2, 8], [27.6, -17.5, 2.6, 0], [35.5, -26.5, 2.7, 0],
  // outside the doors: the tall stack; the crossing; the west lane
  [19.5, -43.5, 2.4, 0], [19.5, -43.5, 2.3, 15, L + 2.4], [2, -26, 1.75, 0], [-3, -29.4, 1.9, 0], [-3, -36.6, 2, 0], [-3, -38.4, 1.7, 6],
];

/** The tops of the blocks of houses, where they are not `PLACE.wall` over the floor beside them. A wall rises to its block. */
export const BLOCKS = [
  // the tall houses along the west and the north-west
  { at: [[-58, 30], [-45, 30], [-45, 64], [-58, 64]], top: 10 },
  { at: [[-56, 6], [-45, 6], [-45, 30], [-56, 30]], top: 12 },
  { at: [[-37, 40], [-25, 40], [-25, 52], [-37, 52]], top: 10.5 },
  // the long wall of houses north of the west site's ramp and the low start
  { at: [[-27, 35], [11, 35], [11, 43], [-27, 43]], top: 12 },
  { at: [[-25, 31], [-13, 35], [-25, 36]], top: 11 },
  { at: [[3, 31], [11, 31], [11, 36], [3, 36]], top: 9 },
  // the houses east of the middle lane
  { at: [[3, 8], [16, 8], [16, 17], [3, 17]], top: 9.5 },
  // the low wall between the east site and the yard under it, and the one along the ramp
  { at: [[23, 25], [36, 25], [36, 28], [23, 28]], top: 6.5 },
  { at: [[41, 25], [42, 25], [42, 37], [41, 37]], top: 6.6 },
  // the high start: the tower by the ramp, the tall houses south and east of it
  { at: [[-36, -41], [-33, -41], [-33, -37], [-36, -37]], top: 10 },
  { at: [[-18, -50], [-13, -50], [-13, -40], [-18, -40]], top: 12 },
  { at: [[-50, -66], [0, -66], [0, -60], [-50, -60]], top: 12 },
];

/** Lamps under the covered ways: [east, north]. */
export const LAMPS = [
  [-34, -18], [-33, -12.5], [-30, -6], [-38, -2.5], [-41, 1], [-41, 8], [-19.5, -5.5], [-12, 2.5], [-7, 2.5], [25.5, -17.5], [25.5, -22],
];

/** The reference's props: ['palm', east, north, scale] or [id, east, north, turn, scale]. */
export const PROPS = [
  ['palm', -41, -56, 1.05],
  ['palm', -12, -58.5, 0.95],
  ['palm', 47, 43, 1],
  ['palm', 13.5, 23.5, 0.9],
  ['palm', 52.5, 22.5, 0.95],
];
