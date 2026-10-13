/**
 * ARCADE ROW, a place of Pocket Duty.
 *
 * Drawn by hand, floor by floor, in the place's own coordinates: metres east and north of its middle,
 * heights in metres above its lowest floor (maps/README.md). North is up in the plan below.
 *
 *        -60     -43      -21   -9    0     +9    +22  +29        +48
 *  +73                           +----terrace (6.1)----+-terrace--+
 *  +66   +----------------------+a+--------------------+----------+
 *        |  NORTH-WEST YARD      r   road (4.9)  slope    LOW      |
 *  +60   +--+-----+ramp +--------c+------------+-kerb-+   START    |
 *           | west| down|        h| NORTH YARD        |stair (1.6) |
 *  +50 +----+ road| to  |         |            +gate+-+------------+
 *      |far west  | the |         +------------+lane|
 *  +43 +----+ walk| way |                      |    |
 *           |(5.7)+-----+======= road over ====+    |
 *  +28      +-----+ SUNKEN  the corridor (4.9)      |
 *                 |  WAY  +----------+pass+---------+
 *  +14            |=bridge+--tunnel--+ HALL +------- east hall (5.7) ------+ bay +--------+
 *   +4            |  (0)  |walk      +------+--------------+pass+----------+     |platform|
 *   -2            |       +----------------+                    +gate      |     | (5.7)  |
 *   -5            +---ramp up east---------+   PLAZA (4.9)      |    EAST SITE (4.9)      |
 *  -15            +--ledge--+--------------+--+lane+------------+-------------------------+
 *  -22            | rooms   |              |    |
 *  -28            +----+--lane-------------+gate+--+
 *  -32            |west|  +house+   SOUTH YARD     |
 *  -45            |lane|  +-----+       +--ramp--+-+
 *                 |    |  |tower|       |  up    | terrace
 *  -61            +----+--+-----+-steps-+--------+      |
 *                 |        SOUTH BASE (6.5), a site     |
 *  -72            +--------------------------------------+
 */

/** The map's name, its turn in the world (which side the sun is on), its walls' height, the starts. */
export const PLACE = {
  id: 'arcade',
  title: 'Arcade Row',
  turn: -0.3,
  wall: 5.7,
  // [east, north, facing in degrees from north toward east, tag]. The first is the player's.
  starts: [
    [-4, -66, 0, 'south base'],
    [8, -67, 0, 'south base'],
    [-13, -68, 20, 'south base'],
    [16, -54, 0, 'base terrace'],
    [5, -37, 0, 'south yard'],
    [-17, -40, 0, 'west lane'],
    [6, -9, 0, 'plaza'],
    [52, -8, 270, 'east site'],
    [42, 6, 180, 'east site, bay'],
    [2, 10, 0, 'hall'],
    [-25.5, 2, 0, 'sunken way'],
    [-8, 31, 90, 'corridor'],
    [-33.5, 44, 0, 'west road'],
    [-27, 65, 90, 'north-west yard'],
    [6, 54, 0, 'north yard'],
    [39, 56, 290, 'low start'],
  ],
};

const SAND = 'ar_sand', SLAB = 'ar_slab', SETT = 'ar_sett';
// The stone of the sunken way: rubble where the road and the walks stand over it, the old stone of its own walls.
const RUBBLE = 'ar_rubble', OLD = 'ar_stone_old';
// The heights most floors lie at: the common level, the halls and walks, the south base, the low start, the terrace.
const M = 4.88, K = 5.69, C = 6.5, T = 1.63, N = 6.1;
// One skyline: the walls of the common level are 5.7 m, and the walls of a floor that lies higher or lower end at the same top.
const TOP = M + 5.7, wallsOf = (y) => +(TOP - y).toFixed(2);

/** The floors: polygons of [east, north], level (`y`) or sloped (`y: { from, to }`, each [east, north, up]). */
export const GROUND = [
  // ------------------------------------------------------------------ the north-west yard and the arch
  { at: [[-43.5, 60], [-11.5, 60], [-11.5, 69.8], [-43.5, 69.8]], y: M, key: SAND },
  { at: [[-11.5, 61], [-9, 61], [-9, 66], [-11.5, 66]], y: M, roof: 4.3 },
  // ------------------------------------------------------------------ the north yard, the road that climbs, the low start
  { at: [[-9, 47.6], [28.5, 47.6], [28.5, 60], [8.7, 60], [8.7, 66.25], [-9, 66.25]], y: M, key: SAND },
  { at: [[8.7, 60], [21.7, 60], [21.7, 66.25], [8.7, 66.25]], y: { from: [8.7, 63, M], to: [21.7, 63, T] } },
  { at: [[21.7, 60], [28.5, 60], [28.5, 53.4], [34.5, 53.4], [34.5, 47.6], [47.5, 47.6], [47.5, 57], [48, 57], [48, 66.25], [21.7, 66.25]], y: T, walls: wallsOf(T) },
  { at: [[28.5, 48], [34.5, 48], [34.5, 53.4], [28.5, 53.4]], y: { from: [28.5, 50.7, M], to: [34.5, 50.7, T] }, key: SLAB, steps: true },
  // the terrace over the road's north wall, its steps, the ledge along the house
  { at: [[1, 66.25], [2.25, 66.25], [2.25, 68.5], [7.25, 68.5], [7.25, 66.25], [23.5, 66.25], [23.5, 73], [1, 73]], y: N, walls: wallsOf(N), key: SLAB },
  { at: [[2.25, 66.25], [7.25, 66.25], [7.25, 68.5], [2.25, 68.5]], y: { from: [4.75, 66.25, M], to: [4.75, 68.5, N] }, walls: wallsOf(N), key: SLAB, steps: true },
  { at: [[23.5, 66.25], [33, 66.25], [33, 68], [23.5, 68]], y: C, key: SLAB },
  { at: [[33, 66.25], [48, 66.25], [48, 73], [33, 73]], y: N, walls: wallsOf(N), key: SLAB },
  // ------------------------------------------------------------------ the west road, its raised walk, the far west
  { at: [[-36.6, 27.5], [-27.7, 27.5], [-30.7, 34], [-30, 34], [-30, 47], [-32, 47], [-32, 49.6], [-30, 49.6], [-30, 53.3], [-21, 53.3], [-21, 60], [-36.6, 60]], y: M, key: SAND, side: RUBBLE },
  { at: [[-43.5, 27.5], [-36.6, 27.5], [-36.6, 47], [-38.1, 47], [-38.1, 50], [-59.5, 50], [-59.5, 43.5], [-43.5, 43.5]], y: K, walls: wallsOf(K), key: SLAB },
  { at: [[-38.1, 47], [-36.6, 47], [-36.6, 50], [-38.1, 50]], y: { from: [-36.6, 48.5, M], to: [-38.1, 48.5, K] }, walls: wallsOf(K), key: SLAB, steps: true },
  // the ramp down into the sunken way, the landing beside it, the steps from the road
  { at: [[-30, 33.7], [-21, 33.7], [-21, 53.3], [-30, 53.3]], y: { from: [-25.5, 53.3, M], to: [-25.5, 33.7, 0] }, key: SETT, wall: OLD },
  { at: [[-30, 42.25], [-27.5, 42.25], [-27.5, 47], [-30, 47]], y: 3.25, walls: wallsOf(3.25), key: SLAB },
  { at: [[-32, 47], [-29, 47], [-29, 49.6], [-32, 49.6]], y: { from: [-32, 48.3, M], to: [-29, 48.3, 3.6] }, key: SLAB, steps: true },
  // ------------------------------------------------------------------ the corridor road, and its span over the sunken way
  { at: [[-27.7, 27.5], [-20.4, 27.5], [-20.4, 34], [-30.7, 34]], y: M, key: SAND, side: RUBBLE, over: true, rail: 0.9 },
  { at: [[-20.4, 27.5], [-1, 27.5], [-1, 28.5], [6, 28.5], [6, 27.5], [22, 27.5], [22, 34], [-20.4, 34]], y: M, key: SAND },
  { at: [[15, 34], [22, 34], [22, 45.5], [21, 45.5], [21, 47.6], [16, 47.6], [16, 45.5], [15, 45.5]], y: M, key: SAND },
  // ------------------------------------------------------------------ the sunken way, its ramp up to the plaza, the bridge
  { at: [[-30.5, -11.5], [-20.6, -11.5], [-20.6, -5], [-21, -5], [-21, 33.7], [-30, 33.7], [-27, 27.5], [-30.5, 27.5], [-30.5, 21.5], [-27.3, 21.5], [-30.5, 15.2]], y: 0, walls: wallsOf(0), key: SETT, wall: OLD },
  { at: [[-20.6, -11.5], [-1, -11.5], [-1, -5], [-20.6, -5]], y: { from: [-20.6, -8.2, 0], to: [-1, -8.2, M] }, key: SETT },
  { at: [[-30.5, 8.25], [-20.4, 8.25], [-20.4, 14.25], [-30.5, 14.25]], y: M, key: SLAB, side: RUBBLE, over: true, rail: 0.9 },
  // the walk beside the way and along the ramp, the steps into the tunnel, the tunnel
  { at: [[-21, -5], [-1, -5], [-1, -2], [-17.5, -2], [-17.5, 9.3], [-18, 9.3], [-18, 12.8], [-17.5, 12.8], [-17.5, 14.5], [-21, 14.5], [-21, 14.25], [-20.4, 14.25], [-20.4, 8.25], [-21, 8.25]], y: M, key: SLAB, side: RUBBLE },
  { at: [[-18, 9.3], [-16.5, 9.3], [-16.5, 12.8], [-18, 12.8]], y: { from: [-18, 11, M], to: [-16.5, 11, K] }, walls: wallsOf(K), key: SLAB, steps: true },
  { at: [[-16.5, 9.3], [-4, 9.3], [-4, 12.8], [-16.5, 12.8]], y: K, walls: wallsOf(K), key: SLAB, roof: 3.6 },
  // ------------------------------------------------------------------ the hall, the pass north, the east hall, the pass south
  { at: [[-4, 4], [8.5, 4], [8.5, 16], [-4, 16]], y: K, walls: wallsOf(K), key: SLAB, roof: 4.4 },
  { at: [[-1, 16], [6, 16], [6, 27], [-1, 27]], y: K, walls: wallsOf(K), key: SLAB },
  { at: [[-1, 27], [6, 27], [6, 28.5], [-1, 28.5]], y: { from: [2.5, 27, K], to: [2.5, 28.5, M] }, walls: wallsOf(K), key: SLAB, steps: true },
  { at: [[8.5, 4], [35, 4], [35, 10], [8.5, 10]], y: K, walls: wallsOf(K), key: SLAB, roof: 3.8 },
  { at: [[35, 5.25], [36.5, 5.25], [36.5, 8.25], [35, 8.25]], y: K, walls: wallsOf(K), key: SLAB, roof: 3.3 },
  { at: [[36.5, 5.25], [38, 5.25], [38, 8.25], [36.5, 8.25]], y: { from: [36.5, 6.75, K], to: [38, 6.75, M] }, walls: wallsOf(K), key: SLAB, steps: true },
  { at: [[17, -2], [21, -2], [21, 4], [17, 4]], y: K, walls: wallsOf(K), key: SLAB, roof: 3.6 },
  { at: [[17, -3.5], [21, -3.5], [21, -2], [17, -2]], y: { from: [19, -3.5, M], to: [19, -2, K] }, walls: wallsOf(K), key: SLAB, steps: true },
  // ------------------------------------------------------------------ the plaza, the ledge and the rooms west of it
  { at: [[-14, -15.2], [22, -15.2], [22, -2], [21, -2], [21, -3.5], [17, -3.5], [17, -2], [-1, -2], [-1, -11.5], [-14, -11.5]], y: M, key: SAND, side: RUBBLE },
  { at: [[-15.5, -15.2], [-14, -15.2], [-14, -11.5], [-15.5, -11.5]], y: { from: [-15.5, -13.3, K], to: [-14, -13.3, M] }, walls: wallsOf(K), key: SLAB, steps: true },
  { at: [[-27, -22], [-17.5, -22], [-17.5, -16.4], [-20.5, -16.4], [-20.5, -15.2], [-15.5, -15.2], [-15.5, -11.5], [-27, -11.5]], y: K, walls: wallsOf(K), key: SLAB, side: RUBBLE },
  { at: [[-17.5, -22], [-16, -22], [-16, -16.4], [-17.5, -16.4]], y: { from: [-17.5, -19, K], to: [-16, -19, M] }, walls: wallsOf(K), key: SLAB, steps: true },
  { at: [[-16, -22], [-5, -22], [-5, -16.4], [-16, -16.4]], y: M, key: SLAB },
  { at: [[-11, -28], [-5.5, -28], [-5.5, -22], [-11, -22]], y: M },
  // ------------------------------------------------------------------ the east site: its yard, its bay, its platform
  { at: [[22, -14], [23, -14], [23, -15.3], [29, -15.3], [29, -14], [35, -14], [35, -15.3], [42, -15.3], [42, -14], [48, -14], [48, -15.3], [55, -15.3], [55, -14], [61, -14], [61, -15.3], [68, -15.3], [68, -2], [22, -2]], y: M, key: SAND },
  { at: [[36.5, -2], [48, -2], [48, 1.5], [46.5, 1.5], [46.5, 4.5], [48, 4.5], [48, 8.25], [46.5, 8.25], [46.5, 12.75], [36.5, 12.75], [36.5, 8.25], [38, 8.25], [38, 5.25], [36.5, 5.25]], y: M, key: SAND },
  { at: [[46.5, 1.5], [48, 1.5], [48, 4.5], [46.5, 4.5]], y: { from: [46.5, 3, M], to: [48, 3, K] }, walls: wallsOf(K), key: SLAB, steps: true },
  { at: [[48, -2], [68, -2], [68, 2.2], [64, 2.2], [64, 5], [66.8, 5], [66.8, 7.2], [48, 7.2]], y: K, walls: wallsOf(K), key: SLAB },
  // ------------------------------------------------------------------ south of the plaza: the lane, the yard, the lanes about the tower
  { at: [[1, -22.5], [3, -22.5], [3, -24.5], [8, -24.5], [8, -22.5], [10.5, -22.5], [10.5, -15.2], [1, -15.2]], y: M },
  { at: [[-1.5, -44.9], [12.5, -44.9], [12.5, -29], [15, -29], [15, -24.5], [-3.5, -24.5], [-3.5, -28], [-19.5, -28], [-19.5, -32], [-1.5, -32]], y: M },
  { at: [[-19.5, -28], [-13, -28], [-13, -24.5], [-19.5, -24.5]], y: M },
  { at: [[12.5, -44.9], [21.5, -44.9], [21.5, -29], [12.5, -29]], y: { from: [17, -29, M], to: [17, -44.9, K] }, walls: wallsOf(K), key: SAND },
  { at: [[16, -29], [21.5, -29], [21.5, -24.5], [16, -24.5]], y: M, key: SAND },
  { at: [[-11.5, -44.9], [-1.5, -44.9], [-1.5, -40.5], [-11.5, -40.5]], y: M },
  { at: [[-19.5, -32], [-15, -32], [-13.5, -44.5], [-13.5, -57.5], [-17.5, -57.5], [-17.5, -55.5], [-21, -48], [-21, -36]], y: M },
  { at: [[-17.5, -60.7], [1.75, -60.7], [1.75, -57.5], [-17.5, -57.5]], y: M },
  { at: [[-1.5, -57.5], [1.75, -57.5], [1.75, -44.9], [-1.5, -44.9]], y: M },
  // ------------------------------------------------------------------ the south base, its ramp to the yard, its steps to the lane
  { at: [[1.75, -57], [9.75, -57], [9.75, -44.9], [1.75, -44.9]], y: { from: [5.75, -57, C], to: [5.75, -44.9, M] }, key: SAND },
  { at: [[-18, -72], [22, -72], [22, -44.9], [9.75, -44.9], [9.75, -57], [1.75, -57], [1.75, -60.7], [-10, -60.7], [-10, -63.7], [-14, -63.7], [-14, -60.7], [-18, -60.7]], y: C, key: SAND },
  { at: [[-14, -63.7], [-10, -63.7], [-10, -60.7], [-14, -60.7]], y: { from: [-12, -63.7, C], to: [-12, -60.7, M] }, key: SLAB, steps: true },
];

// A low wall along an edge: a parapet a player looks over.
const low = (line, y, h = 0.81, t = 0.5) => ({ line, y, h, t, crown: false });

/** Walls along a line, with what is cut in them: `holes` of `{ at, w, h, sill, arch }`, `at` in metres from the first point. */
export const WALLS = [
  // the arch, at both faces of its block
  { line: [[-11.5, 61], [-11.5, 66]], y: M, holes: [{ at: 2.5, w: 4.4, h: 4.2, arch: 0.7 }] },
  { line: [[-9, 61], [-9, 66]], y: M, holes: [{ at: 2.5, w: 4.4, h: 4.2, arch: 0.7 }] },
  // the mouths of the sunken way under the road and under the bridge
  { line: [[-27.2, 27.75], [-21, 27.75]], y: 0, key: RUBBLE, h: M - 0.3, crown: false, holes: [{ at: 3.1, w: 5.4, h: 4, arch: 0.6 }] },
  { line: [[-29.8, 33.6], [-21, 33.6]], y: 0, key: RUBBLE, h: M - 0.3, crown: false, holes: [{ at: 4.4, w: 7.6, h: 4, arch: 0.6 }] },
  { line: [[-30.5, 8.5], [-21, 8.5]], y: 0, key: RUBBLE, h: M - 0.3, crown: false, holes: [{ at: 4.75, w: 8.2, h: 4, arch: 0.6 }] },
  { line: [[-30.5, 14], [-21, 14]], y: 0, key: RUBBLE, h: M - 0.3, crown: false, holes: [{ at: 4.75, w: 8.2, h: 4, arch: 0.6 }] },
  // the tunnel's mouth, the hall's doorways, the pass south
  { line: [[-16.5, 9.3], [-16.5, 12.8]], y: K, h: TOP - K, holes: [{ at: 1.75, w: 2.9, h: 3.1, arch: 0.6 }] },
  { line: [[-1, 16], [6, 16]], y: K, h: TOP - K, holes: [{ at: 3.5, w: 4.2, h: 3.5, arch: 0.6 }] },
  { line: [[8.5, 4], [8.5, 10]], y: K, h: TOP - K, holes: [{ at: 3, w: 4, h: 3.4, arch: 0.6 }] },
  { line: [[17, -2], [21, -2]], y: K, h: TOP - K, holes: [{ at: 2, w: 3.2, h: 3.2, arch: 0.6 }] },
  // the gate between the plaza and the east site, and the gate of the south lane
  { line: [[22, -15.2], [22, -2]], y: M, t: 1, holes: [{ at: 6.7, w: 5, h: 4, arch: 0.4 }] },
  { line: [[3, -23.5], [8, -23.5]], y: M, t: 1, holes: [{ at: 2.5, w: 3.8, h: 3.6, arch: 0.6 }] },
  // parapets: the road over the ramp, the north yard's kerb, the terrace's, the walk's over the way and the ramp
  low([[-30.25, 34], [-30.25, 47]], M),
  low([[-30.25, 49.6], [-30.25, 53.3]], M),
  low([[8.7, 59.7], [28.2, 59.7]], M, 0.81, 0.6),
  low([[28.2, 53.6], [28.2, 60]], M, 0.81, 0.6),
  low([[7.25, 66.55], [23.5, 66.55]], N, 0.4, 0.6),
  low([[33, 66.55], [48, 66.55]], N, 0.4, 0.6),
  low([[-20.75, -5], [-20.75, 8.25]], M),
  low([[-20.5, -4.75], [-1, -4.75]], M),
  low([[-14, -11.75], [-1, -11.75]], M),
  low([[-27, -11.75], [-15.5, -11.75]], K),
  // the platform's kerb
  low([[48.25, -2], [48.25, 1.5]], K),
  low([[48.25, 4.5], [48.25, 7.2]], K),
  low([[48.5, -1.75], [64, -1.75]], K),
  // the south base: the walls of its ramp, its parapets over the lane and the yard
  { line: [[1.75, -60.7], [1.75, -44.9]], y: M, h: 2.44, t: 1.2, crown: false },
  low([[9.75, -57], [9.75, -45.2]], C, 0.82, 0.8),
  low([[-18, -61], [-14, -61]], C, 0.82, 0.6),
  low([[-10, -61], [1.15, -61]], C, 0.82, 0.6),
  low([[10.15, -45.2], [22, -45.2]], C, 0.82, 0.6),
];

/** Doors of boards standing open: `facing` is the way the leaves swing, in degrees from north toward east. */
export const DOORS = [
  { at: [22.6, -8.5], facing: 90, w: 5, h: 3.9, open: 115 },
  { at: [36.6, 6.75], facing: 90, w: 3, h: 3.1 },
];

/** Crates: [east, north, side, turn in degrees, up]. Without `up` a crate stands on the floor. */
export const CRATES = [
  // the north-west yard's big one; the far west's pile; the raised walk's corner
  [-19, 68.1, 3.2, 0],
  [-57.8, 48.3, 3.2, 0], [-55.2, 48.8, 1.6, 6], [-57.6, 45, 2.4, 0], [-55.1, 44.9, 2.4, 10], [-56.4, 45, 1.2, 20, K + 2.4], [-57.8, 48.3, 1.2, 15, K + 3.2],
  [-42.2, 28.8, 2.4, 0], [-42.6, 31, 1.6, 8], [-40.5, 28.4, 1.2, 0],
  // on the ramp, in the sunken way, on the bridge
  [-28.8, 41, 2.4, 0],
  [-29.5, 22.4, 1.6, 0], [-29.5, 22.4, 1.6, 10, 1.6], [-29.6, 24.2, 1.6, 4], [-22, 25, 1.6, 0], [-21.9, 22, 1.6, 0], [-21.9, 22, 1.6, 14, 1.6], [-21.9, 17, 1.6, 0],
  [-29.6, 5, 1.6, 0], [-29.6, 5, 1.6, 8, 1.6], [-29.6, 6.7, 1.6, 0], [-22.3, 6.4, 2.4, 0], [-21.9, -1, 1.6, 0], [-21.9, -1, 1.6, 12, 1.6], [-21.9, -2.9, 1.6, 0],
  [-29.2, -6.6, 2.8, 0], [-29.2, -6.6, 2.8, 6, 2.8], [-29.3, -9.6, 2.4, 0], [-29.3, -9.6, 2.4, 10, 2.4], [-27.4, -10.4, 1.6, 0], [-27.4, -10.4, 1.6, 20, 1.6],
  [-24.7, 12.5, 2.4, 0, M],
  // the corridor, the pass north, the hall, the east hall
  [-11.6, 28.2, 1.2, 0], [-9.3, 28.9, 2.6, 0], [15, 28.8, 2.4, 0], [17.4, 28.2, 1.2, 10],
  [0.3, 19.4, 2.4, 0],
  [-3, 5.2, 2.4, 0], [-3, 5.2, 1.2, 10, K + 2.4], [7.2, 14.7, 2.4, 0], [7.6, 12.6, 1.6, 0], [7.6, 12.6, 1.6, 10, K + 1.6],
  [17, 9.1, 1.6, 0], [17, 9.1, 1.6, 12, K + 1.6],
  // the plaza: against its north wall, by the gate, on the ledge
  [14.7, -3.7, 3.2, 0], [11.7, -3.9, 2.4, 0], [14.2, -3, 1.2, 10, M + 3.2], [19.7, -13.4, 3.2, 0], [-25.8, -13.3, 2.4, 0],
  // the east site: its yard, the platform, the site's own
  [29.2, -3.7, 3.2, 0], [32.2, -3.3, 2.4, 6],
  [57.2, 5.9, 2.4, 0], [65.8, 0.2, 3.2, 0], [65.8, 0.2, 1.2, 15, K + 3.2],
  [48.8, -4.7, 1.5, 0], [50.4, -4.7, 1.5, 5], [47.3, -11.2, 1.5, 0], [47.3, -12.8, 1.5, 0], [48.9, -13.2, 1.6, 0],
  [53.8, -13.1, 1.6, 0], [55.5, -13.2, 1.5, 0], [57.1, -13.2, 1.6, 8], [57.4, -11.5, 1.6, 0],
  // the low start: crates as steps up to the north yard; the north yard's
  [31, 54.6, 1.6, 0], [30.6, 56.3, 1.6, 6], [29.4, 54.6, 1.6, 0], [29.4, 54.6, 1.6, 8, T + 1.6],
  [26.5, 57.6, 3.1, 0], [24.6, 58, 1.6, 0], [26.3, 55.1, 1.6, 10],
  [-7.5, 49.2, 2.6, 0], [-7.5, 49.2, 2.6, 8, M + 2.6], [-5.6, 50.8, 2.4, 20],
  [44, 70.3, 2.4, 0],
  // the rooms, the lanes, the south yard
  [-25.8, -20.7, 2.4, 0], [-6.3, -17.8, 2.5, 0], [-6.3, -17.8, 2.4, 10, M + 2.5], [-5.9, -20.6, 1.6, 0],
  [12.9, -26.4, 3.2, 0], [11.9, -29.9, 1.6, 0], [13.6, -29.8, 1.6, 8], [12.6, -31.6, 1.2, 0],
  [-8.8, -42.4, 2.4, 0], [-10.6, -41.4, 1.6, 0],
  // the south base: its terrace, its own, the site's
  [11.5, -47, 3.2, 0], [14.4, -46.2, 1.6, 0], [16.1, -46.2, 1.6, 6], [20.6, -50.8, 1.6, 0],
  [3.8, -70.2, 3.2, 0], [6.5, -70.6, 2.4, 0],
  [9, -63.5, 1.5, 0], [12.2, -63.3, 1.6, 30], [13.9, -64.9, 1.6, 30], [12.8, -70.8, 1.5, 0], [14.4, -70.8, 1.5, 4],
];

/**
 * The tops of the blocks of houses, where they are not the common walls' top. A wall rises to its block.
 * The houses beyond the walls are of several heights: no roof runs level for more than 50 m.
 */
export const BLOCKS = [
  // the houses north of the terrace, the two tall ones that stand in it
  { at: [[-9.5, 66.25], [1, 66.25], [1, 78], [-9.5, 78]], top: 11.4 },
  { at: [[23.5, 68], [33, 68], [33, 78], [23.5, 78]], top: 11.4 },
  { at: [[1, 73], [23.5, 73], [23.5, 78], [1, 78]], top: 10 },
  { at: [[33, 73], [50, 73], [50, 78], [33, 78]], top: 10 },
  { at: [[-36, 70.6], [-22, 70.6], [-22, 81], [-36, 81]], top: 11.5 },
  // the gate posts of the lane into the north yard, and the towers at the yards' corners
  { at: [[13.5, 45.5], [16, 45.5], [16, 47.6], [13.5, 47.6]], top: 12.2 },
  { at: [[21, 45.5], [23.5, 45.5], [23.5, 47.6], [21, 47.6]], top: 12.2 },
  { at: [[-11.5, 45.4], [-9, 45.4], [-9, 47.6], [-11.5, 47.6]], top: 12.2 },
  { at: [[46.5, 45.4], [49.2, 45.4], [49.2, 47.6], [46.5, 47.6]], top: 12.2 },
  { at: [[34.3, 12.3], [36.5, 12.3], [36.5, 14.6], [34.3, 14.6]], top: 12.2 },
  { at: [[34.3, -2], [36.5, -2], [36.5, 0.4], [34.3, 0.4]], top: 12.2 },
  // the tower south of the yard, and the two towers of the south base's wall
  { at: [[-13.5, -57.5], [-1.5, -57.5], [-1.5, -44.9], [-13.5, -44.9]], top: 15.4 },
  { at: [[-7.5, -81], [-1.5, -81], [-1.5, -72], [-7.5, -72]], top: 13.5 },
  { at: [[5.5, -81], [11.5, -81], [11.5, -72], [5.5, -72]], top: 13.5 },
  // houses of other heights beyond the walls: south-west, west, north-west, north-east, east, south-east
  { at: [[-68, -81], [-44, -81], [-44, -58], [-68, -58]], top: 11.5 },
  { at: [[-44, -81], [-18.7, -81], [-18.7, -61.5], [-44, -61.5]], top: 10.75 },
  { at: [[-68, -58], [-44, -58], [-44, -30], [-68, -30]], top: 10.25 },
  { at: [[-68, -30], [-40, -30], [-40, -12], [-68, -12]], top: 11.25 },
  { at: [[-68, -12], [-48, -12], [-48, 12], [-68, 12]], top: 10 },
  { at: [[-68, 12], [-48, 12], [-48, 43], [-68, 43]], top: 11 },
  { at: [[-68, 51], [-45, 51], [-45, 81], [-68, 81]], top: 10.25 },
  { at: [[50, 44], [76, 44], [76, 81], [50, 81]], top: 10.25 },
  { at: [[26, 12.5], [47, 12.5], [47, 27.4], [26, 27.4]], top: 11.5 },
  { at: [[40, 30], [60, 30], [60, 44], [40, 44]], top: 11 },
  { at: [[60, 13], [76, 13], [76, 44], [60, 44]], top: 10 },
  { at: [[30, -25], [50, -25], [50, -15.4], [30, -15.4]], top: 11.25 },
  { at: [[50, -28], [76, -28], [76, -16.2], [50, -16.2]], top: 10 },
  { at: [[30, -58], [52, -58], [52, -30], [30, -30]], top: 11.75 },
  { at: [[52, -58], [76, -58], [76, -30], [52, -30]], top: 10.75 },
  { at: [[22.7, -81], [48, -81], [48, -60], [22.7, -60]], top: 11.5 },
  { at: [[48, -81], [76, -81], [76, -60], [48, -60]], top: 10.25 },
];

/** Solids of any footprint: the belt about the tower, level with the walls' tops beside it. */
export const MASSES = [
  { at: [[-13.68, -57.68], [-1.32, -57.68], [-1.32, -44.72], [-13.68, -44.72]], y: 10.15, top: 10.5, key: 'ar_stone_warm' },
];

/** A site's mark, of our own, on its floor. */
export const MARKS = [
  { at: [52, -9], size: 6 },
  { at: [11.4, -66.4], size: 6 },
];

/** Lamps: [east, north, up]. Without `up` a lamp hangs under the roof of the floor it is over. */
export const LAMPS = [
  // under the road and under the bridge
  [-24.3, 30.7, 4.5], [-25.5, 11.3, 4.5],
  // the arch, the tunnel, the hall, the east hall, the pass south
  [-10.2, 63.5], [-13.2, 11], [-7.5, 11], [2.2, 12.5], [2.2, 7], [13.5, 7], [21, 7], [29, 7], [19, 1],
];

/** The reference's props: ['palm', east, north, scale] or [id, east, north, turn, scale]. */
export const PROPS = [
  ['palm', -40.5, 67, 1.05],
  ['palm', 45, 50, 1],
  ['palm', -47, 46.5, 0.9],
  ['palm', 64.5, -12, 0.95],
  ['palm', 19.5, -69.5, 1],
  ['palm', -15.5, -70, 0.95],
  ['palm', 10.5, -42.5, 0.9],
];
