/**
 * OLD QUARTER, a place of Pocket Duty.
 *
 * Drawn by hand, floor by floor, in the place's own coordinates: metres east and north of its middle,
 * heights in metres above its lowest floor (maps/README.md). North is up in the plan below.
 *
 *      -55     -40    -27      -10    0   +10     +22    +35    +45    +58
 *  +50                             +--north strip (7.3)--+
 *                              +---+----+ +-+            |
 *  +40                         | NORTH  | |l|   +--------+
 *                              | YARD   +-+a+---+ north street (6.5) --+
 *  +30                         | (7.3)  | |n|   +-----+ cellar +-ramp-+--+
 *                              +--------+ |e|         | ramp   | EAST    |
 *  +20                               +----+ |         +--------+ START   |
 *                                   / yard  |    +-west yard-+st| (5.3)  |
 *  +10                             / (6.5)--+    | (7.3)     +--+--------+
 *                                 / lane          +--+arch+--+  | walk   |
 *    0                         +-+ /                  |    |    | rooms  |
 *                              | |/         +---------+----+----+ (7.3)  |
 *  -10              +---ramp---+ +          |  hall (7.3)  open  +-------+
 *       +-------+   | +--------+neck        |  +-----+   +------+
 *  -20  | LOW   +---+ | yard  | cross street (5.7) ramp  | PORCH| yard |trees
 *       | START | cov.| (3.25)+--+---------+ +--+  | west | (7.3)| (7.3)|
 *  -30  | (1.6) +-----+--+ ROOMS |  house  | |si|  | yard |      |      |
 *       +--+ramp+     |a | (8.1) +---------+ |de|  |      +------+------+
 *  -40     |    |     |l +=bridge= south street | +walk---+ gallery +--pit--+
 *          |    |     |l | ROOMS |  (3.3-5.7)  +-+ (8.9) | (10.2)  | (6.0) |
 *  -50     +----+-----+e +-------+-passage (6.5)-+--stair-+---------+-------+
 *          | south alley (3.25)  |
 *          +---------------------+
 */

/** The map's name, its turn in the world (which side the sun is on), its walls' height, the starts. */
export const PLACE = {
  id: 'quarter',
  title: 'Old Quarter',
  turn: 0.42,
  wall: 7,
  // [east, north, facing in degrees from north toward east, tag]. The first is the player's.
  starts: [
    [-52, -21, 110, 'low start'],
    [-54.5, -18, 100, 'low start'],
    [-50, -27, 90, 'low start'],
    [-55, -24, 100, 'low start'],
    [-29, -21, 90, 'alley yard'],
    [-8, -19, 90, 'cross street'],
    [3, 16, 180, 'bent lane'],
    [-3, 39, 150, 'north yard'],
    [26.5, -12, 180, 'hall'],
    [48, -22, 270, 'east porch'],
    [30, 36, 270, 'north street'],
    [51, 22, 250, 'east start'],
    [48, 27, 240, 'east start'],
    [53.5, 16, 270, 'east start'],
  ],
};

// The heights most floors lie at.
const LOW = 1.6, ALLEY = 3.25, MID = 5.7, LANE = 6.5, SITE = 7.3, EAST = 5.3, ROOMS = 8.1, WALK = 8.9, GALLERY = 10.2;
// The clear height under a roof: a room, a hall.
const ROOM = 3.2, HALL = 4;
// Floors that are not cobbles, and walls that are not limewash (./look.js).
const TILE = 'qt_tile', BRICK = 'qt_brick', PLANK = 'qt_plank', EARTH = 'qt_earth';
const ROSE = 'qt_rose', OCHRE = 'qt_ochre', SAGE = 'qt_sage', RUBBLE = 'qt_rubble';

/** The floors: polygons of [east, north], level (`y`) or sloped (`y: { from, to }`, each [east, north, up]). */
export const GROUND = [
  // ------------------------------------------------------------------ the low start and its two ways out
  { at: [[-58.5, -23.5], [-58.5, -17.5], [-56, -17.5], [-56, -11], [-48, -11], [-48, -14.5], [-46.5, -14.5], [-46.5, -24.5], [-46, -24.5], [-46, -30.5], [-56, -30.5], [-56, -23.5]], y: LOW, wall: ROSE },
  // the ramp south, and the alley it reaches: west, then along the south wall
  { at: [[-53, -40], [-46.5, -40], [-46.5, -30.5], [-53, -30.5]], y: { from: [-50, -30.5, LOW], to: [-50, -40, ALLEY] }, wall: RUBBLE },
  { at: [[-53, -53], [-27, -53], [-27, -46.5], [-46.5, -46.5], [-46.5, -40], [-53, -40]], y: ALLEY, wall: RUBBLE },
  // the covered way east through a house, up to the alley yard
  { at: [[-46, -30], [-43, -30], [-43, -25], [-46, -25]], y: { from: [-46, -27.5, LOW], to: [-43, -27.5, 2.45] }, roof: ROOM },
  { at: [[-43, -30], [-40, -30], [-40, -25], [-43, -25]], y: 2.45, roof: ROOM },
  { at: [[-43, -25], [-40, -25], [-40, -22.5], [-43, -22.5]], y: { from: [-41.5, -25, 2.45], to: [-41.5, -22.5, ALLEY] }, roof: ROOM },
  { at: [[-43, -22.5], [-33.5, -22.5], [-33.5, -18], [-40, -18], [-40, -17], [-43, -17]], y: ALLEY, roof: ROOM },
  // ------------------------------------------------------------------ the alley, its yard, the stair to a door
  { at: [[-33.5, -46.5], [-27, -46.5], [-27, -25.5], [-31.5, -25.5], [-31.5, -43.5], [-33.5, -43.5]], y: ALLEY },
  { at: [[-33.5, -43.5], [-31.5, -43.5], [-31.5, -40], [-33.5, -40]], y: { from: [-32.5, -43.5, ALLEY], to: [-32.5, -40.3, 4.9] }, steps: true },
  { at: [[-33.5, -40], [-31.5, -40], [-31.5, -36.5], [-33.5, -36.5]], y: 4.9 },
  { at: [[-33.5, -25.5], [-27, -25.5], [-27, -21.5], [-23.5, -21.5], [-23.5, -20], [-21.5, -20], [-21.5, -10.5], [-25.5, -10.5], [-25.5, -16.5], [-33.5, -16.5]], y: ALLEY, wall: OCHRE },
  // the stair in the yard's corner: two flights in the open, a landing and a flight inside, up to the rooms
  { at: [[-23.5, -23.5], [-21.5, -23.5], [-21.5, -20], [-23.5, -20]], y: { from: [-22.5, -20, ALLEY], to: [-22.5, -22, 4] }, steps: true },
  { at: [[-26.5, -23.5], [-23.5, -23.5], [-23.5, -21.5], [-26.5, -21.5]], y: { from: [-23.5, -22.5, 4], to: [-25.5, -22.5, 4.9] }, steps: true },
  { at: [[-26.5, -25.5], [-23.5, -25.5], [-23.5, -23.5], [-26.5, -23.5]], y: 4.9, roof: ROOM },
  { at: [[-23.5, -25.5], [-17.5, -25.5], [-17.5, -23.5], [-23.5, -23.5]], y: { from: [-23.5, -24.5, 4.9], to: [-17.5, -24.5, ROOMS] }, steps: true, roof: ROOM },
  // ------------------------------------------------------------------ the rooms upstairs, on both sides of the south street
  { at: [[-26.5, -32.5], [-26.5, -26], [-17.5, -26], [-17.5, -23.5], [-8, -23.5], [-8, -27.5], [-6, -27.5], [-6, -34], [-14.5, -34], [-14.5, -32.5]], y: ROOMS, roof: ROOM, key: PLANK, side: ROSE },
  // (the room over the street: a floor over a floor, half a metre wider than its walls on each side)
  { at: [[-23.5, -40.5], [-18, -40.5], [-18, -32.5], [-23.5, -32.5]], y: ROOMS, over: true, rail: 0, key: PLANK, side: ROSE },
  { at: [[-26.5, -49.5], [-26.5, -40.5], [-16, -40.5], [-16, -43.5], [-12.5, -43.5], [-12.5, -47], [-16, -47], [-16, -49.5]], y: ROOMS, roof: ROOM, key: PLANK, side: OCHRE },
  // their stair down east to the passage, the passage and its mouth on the south street
  { at: [[-12.5, -47], [-9.5, -47], [-9.5, -43.5], [-12.5, -43.5]], y: { from: [-9.5, -45.25, LANE], to: [-12.5, -45.25, ROOMS] }, steps: true, roof: ROOM },
  { at: [[-9.5, -47], [-4, -47], [-4, -43.5], [-9.5, -43.5]], y: LANE, roof: ROOM },
  { at: [[-4, -49.5], [-1.5, -49.5], [-1.5, -41.5], [-4, -41.5]], y: LANE, roof: ROOM },
  { at: [[-4, -41.5], [-1, -41.5], [-1, -39.5], [-4, -39.5]], y: LANE },
  { at: [[-1, -41], [0.5, -41], [0.5, -39.5], [-1, -39.5]], y: { from: [0.5, -40.25, 5.2], to: [-1, -40.25, LANE] }, steps: true },
  { at: [[-4, -52.5], [10, -52.5], [10, -49.5], [-4, -49.5]], y: LANE, roof: ROOM },
  // ------------------------------------------------------------------ the south street, under the room, up to the side street
  { at: [[-27, -40.5], [-14.5, -40.5], [-14.5, -39.5], [0.5, -39.5], [0.5, -41], [7.5, -41], [7.5, -33.5], [-6, -33.5], [-6, -34], [-14.5, -34], [-14.5, -32.5], [-27, -32.5]], y: { from: [-27, -37, ALLEY], to: [7.5, -37, MID] }, wall: SAGE },
  // the sunken path north of it, up to a ledge under the cross street
  { at: [[-5.5, -33.8], [-3.5, -33.8], [-3.5, -21], [-5.5, -21]], y: { from: [-4.5, -21, 4.2], to: [-4.5, -33.5, 4.9] }, wall: RUBBLE },
  // ------------------------------------------------------------------ the middle: the ramp from the alley yard, the cross street
  { at: [[-21.5, -14], [-14, -14], [-14, -10.5], [-21.5, -10.5]], y: { from: [-21.5, -12, ALLEY], to: [-14, -12, MID] }, wall: RUBBLE },
  { at: [[-14, -14], [-10, -14], [-10, -16.5], [-5.5, -16.5], [-5.5, -10.5], [-14, -10.5]], y: MID },
  { at: [[-10, -23.5], [-6.5, -23.5], [-6.5, -21], [14, -21], [14, -23], [15, -23], [15, -16.5], [-10, -16.5]], y: MID },
  // the sunken path beside it, down from the street and north to a dead end under the way to the neck
  { at: [[-3, -16.5], [3.5, -16.5], [3.5, -14.5], [-3, -14.5]], y: { from: [3, -15.5, MID], to: [0.5, -15.5, 4.5] } },
  { at: [[-5.5, -16.5], [-3, -16.5], [-3, -10.5], [-5.5, -10.5]], y: 4.5 },
  // the cross street's ramp up east, the side street south with its kerb
  { at: [[15, -23], [22.5, -23], [22.5, -16.5], [15, -16.5]], y: { from: [15, -19.5, MID], to: [21.5, -19.5, SITE] } },
  { at: [[7.5, -41], [12, -41], [12, -21], [7.5, -21]], y: MID, wall: SAGE },
  { at: [[12, -41], [14, -41], [14, -21], [12, -21]], y: 6.1, wall: SAGE },
  { at: [[7.5, -44.5], [14, -44.5], [14, -41], [7.5, -41]], y: 6.1, wall: SAGE },
  { at: [[11.2, -47], [13.6, -47], [13.6, -44.5], [11.2, -44.5]], y: 6.1, roof: 3 },
  { at: [[10, -52.5], [15, -52.5], [15, -47], [10, -47]], y: 6.1, roof: ROOM },
  // the stair east of it, and the stair up to the gallery
  { at: [[15, -52.5], [17, -52.5], [17, -49.5], [15, -49.5]], y: { from: [15, -51, 6.1], to: [17, -51, SITE] }, steps: true, roof: ROOM },
  // (the stair's well: one roof over its landing and its flight)
  { at: [[17, -52.5], [20.5, -52.5], [20.5, -49.5], [17, -49.5]], y: SITE, roof: GALLERY + ROOM - SITE },
  { at: [[17.5, -50.5], [20, -50.5], [20, -45], [17.5, -45]], y: { from: [18.75, -50.5, SITE], to: [18.75, -45, GALLERY] }, steps: true, roof: ROOM },
  // ------------------------------------------------------------------ the neck, the bent lane, its yard, its head
  { at: [[-9.5, -10.5], [-6, -10.5], [-6, -5], [-9.5, -5]], y: MID },
  { at: [[-10.5, -5], [-4.5, -5], [-4.5, 2], [-10.5, 2]], y: MID, wall: ROSE },
  { at: [[-10.5, 2], [-4.5, 2], [0.5, 12], [-5.5, 12]], y: { from: [-7.5, 2, MID], to: [-7.5, 12, LANE] }, wall: ROSE },
  { at: [[-5.5, 12], [14, 12], [14, 15.5], [12, 15.5], [12, 22], [-0.5, 22]], y: LANE },
  { at: [[6.5, 22], [11, 22], [11, 27], [12, 27], [12, 33.5], [6, 33.5], [6, 27], [6.5, 27]], y: LANE, wall: OCHRE },
  // ------------------------------------------------------------------ the north street, the way round the north, the north yard
  { at: [[6, 33.5], [45.5, 33.5], [45.5, 40], [17, 40], [17, 38], [12, 38], [12, 41.5], [6, 41.5]], y: LANE },
  { at: [[17, 40], [22, 40], [22, 43], [17, 43]], y: LANE },
  { at: [[18, 43], [21, 43], [21, 44.5], [18, 44.5]], y: { from: [19.5, 43, LANE], to: [19.5, 44.5, SITE] }, steps: true },
  { at: [[-1, 46.5], [18, 46.5], [18, 44.5], [21, 44.5], [21, 46.5], [22, 46.5], [22, 51], [15.5, 51], [15.5, 49.5], [12, 49.5], [12, 51], [2.5, 51], [2.5, 49.5], [1, 49.5], [1, 48], [-1, 48]], y: SITE, wall: RUBBLE },
  { at: [[-9, 28.5], [4.5, 28.5], [4.5, 35], [6, 35], [6, 41.5], [4.5, 41.5], [4.5, 46.5], [-4, 46.5], [-4, 42], [-7.5, 42], [-7.5, 46.5], [-9, 46.5]], y: SITE, wall: OCHRE },
  { at: [[4.5, 31.5], [6, 31.5], [6, 35], [4.5, 35]], y: { from: [4.5, 33, SITE], to: [6, 33, LANE] }, steps: true },
  // ------------------------------------------------------------------ the east start: its yard, its loggia, the ramp from the north street
  { at: [[45.5, 33.5], [47.5, 33.5], [47.5, 36], [45.5, 36]], y: 6.4 },
  { at: [[45.5, 31.5], [55.5, 31.5], [55.5, 33.5], [45.5, 33.5]], y: { from: [46, 32.5, 6.4], to: [55.5, 32.5, EAST] } },
  { at: [[43.5, 11], [56.5, 11], [56.5, 31], [55.5, 31], [55.5, 31.5], [45.5, 31.5], [45.5, 33], [39, 33], [39, 25.5], [34.5, 25.5], [34.5, 23.5], [45.5, 23.5], [45.5, 16.5], [43.5, 16.5]], y: EAST, wall: SAGE },
  { at: [[56.5, 11], [58.5, 11], [58.5, 31], [56.5, 31]], y: EAST, roof: 4.5 },
  // the cellar ramp west of it
  { at: [[30, 25.5], [39, 25.5], [39, 30.5], [30, 30.5]], y: { from: [39, 28, EAST], to: [30, 28, 2.6] }, wall: RUBBLE },
  { at: [[28, 23.5], [34.5, 23.5], [34.5, 25.5], [30, 25.5], [30, 30.5], [28, 30.5]], y: 2.6, wall: RUBBLE },
  // the stair up west to the west yard, the yard and its arch south to the hall
  { at: [[41, 11], [43.5, 11], [43.5, 16.5], [41, 16.5]], y: { from: [41, 13.5, SITE], to: [43.5, 13.5, EAST] }, steps: true },
  { at: [[30, 5.5], [39, 5.5], [39, 10], [41, 10], [41, 17], [30, 17]], y: SITE, wall: ROSE },
  { at: [[33.5, -1.5], [38, -1.5], [38, 5.5], [33.5, 5.5]], y: SITE, wall: RUBBLE },
  // the raised walk south of the start, the rooms it leads through
  { at: [[45.5, 7.5], [58.5, 7.5], [58.5, 11], [41, 11], [41, 10], [45.5, 10]], y: SITE },
  { at: [[49.5, 2], [59.5, 2], [59.5, 7.5], [49.5, 7.5]], y: SITE, roof: ROOM, key: TILE },
  { at: [[51.25, -7], [59.5, -7], [59.5, 2], [51.25, 2]], y: SITE, roof: ROOM, key: TILE },
  // ------------------------------------------------------------------ the hall, the open way east of it
  { at: [[22.5, -8.5], [40.5, -8.5], [40.5, -1.5], [22.5, -1.5]], y: SITE, roof: HALL, key: TILE },
  { at: [[22.5, -16], [30.5, -16], [30.5, -8.5], [22.5, -8.5]], y: SITE, roof: HALL, key: TILE },
  { at: [[40.5, -8.5], [51.25, -8.5], [51.25, -1.5], [40.5, -1.5]], y: SITE, wall: OCHRE },
  { at: [[45, -15.5], [51, -15.5], [51, -8.5], [45, -8.5]], y: SITE, wall: OCHRE },
  // ------------------------------------------------------------------ the east site: the west yard, the way south, the porch, the yard
  { at: [[22.5, -35], [30.5, -35], [30.5, -16], [22.5, -16]], y: SITE },
  { at: [[30.5, -35], [51.5, -35], [51.5, -29.5], [30.5, -29.5]], y: SITE, wall: ROSE },
  { at: [[28.5, -37], [51.5, -37], [51.5, -35], [28.5, -35]], y: SITE, wall: ROSE },
  { at: [[35, -29.5], [44.5, -29.5], [44.5, -15.5], [35, -15.5]], y: SITE, roof: HALL, key: BRICK },
  { at: [[44.5, -29.5], [51.5, -29.5], [51.5, -15.5], [44.5, -15.5]], y: SITE, wall: OCHRE },
  { at: [[51.5, -25.5], [55.5, -25.5], [55.5, -17], [51.5, -17]], y: SITE, wall: OCHRE },
  { at: [[55.5, -33.5], [57.5, -33.5], [57.5, -17], [55.5, -17]], y: SITE, roof: 2.8 },
  { at: [[51.5, -33.5], [55.5, -33.5], [55.5, -31.5], [51.5, -31.5]], y: SITE },
  // the bed of trees
  { at: [[51.5, -31.5], [55.5, -31.5], [55.5, -25.5], [51.5, -25.5]], y: 7.7, key: EARTH },
  // the steps down to the sunken garden south of the yard
  { at: [[51.5, -37], [57, -37], [57, -33.5], [51.5, -33.5]], y: { from: [54, -33.5, SITE], to: [54, -35.5, 6] }, steps: true },
  { at: [[43, -46.5], [57, -46.5], [57, -37], [43, -37]], y: 6, key: EARTH, wall: RUBBLE },
  // ------------------------------------------------------------------ the raised walk and the gallery over the yard
  { at: [[18.5, -27.5], [22.5, -27.5], [22.5, -25.5], [18.5, -25.5]], y: { from: [22, -26.5, SITE], to: [19, -26.5, WALK] }, steps: true },
  { at: [[14.5, -40.5], [18.5, -40.5], [18.5, -25.5], [16.5, -25.5], [16.5, -34], [14.5, -34]], y: WALK, roof: 3, key: PLANK },
  { at: [[16.5, -45], [38, -45], [38, -40.5], [16.5, -40.5]], y: GALLERY, roof: ROOM, key: PLANK },
  { at: [[36, -40.5], [43, -40.5], [43, -37], [36, -37]], y: GALLERY, key: PLANK, wall: ROSE, side: ROSE },
];

/** The tops of what stands beside the floors, where it is not `PLACE.wall` over the floor beside it. */
export const BLOCKS = [
  // the shed in the north yard's corner
  { at: [[-7.5, 42], [-4, 42], [-4, 46.5], [-7.5, 46.5]], top: 10.5 },
];

/** Walls along a line, with what is cut in them: `holes` of `{ at, w, h, sill, arch }`, `at` in metres from the first point. */
export const WALLS = [
  // the room over the south street: its two sides with their windows, and the houses' walls under it
  { line: [[-23, -40.5], [-23, -32.5]], y: ROOMS, h: ROOM, t: 0.4, low: 0, crown: false, key: ROSE, holes: [{ at: 2.4, w: 1.5, h: 1.3, sill: 1, frame: true }, { at: 5.6, w: 1.5, h: 1.3, sill: 1, frame: true }] },
  { line: [[-18.5, -40.5], [-18.5, -32.5]], y: ROOMS, h: ROOM, t: 0.4, low: 0, crown: false, key: ROSE, holes: [{ at: 2.4, w: 1.5, h: 1.3, sill: 1, frame: true }, { at: 5.6, w: 1.5, h: 1.3, sill: 1, frame: true }] },
  { line: [[-23.5, -32.5], [-18, -32.5]], y: 3.4, h: 4.5, crown: false, key: ROSE },
  { line: [[-23.5, -40.5], [-18, -40.5]], y: 3.4, h: 4.5, crown: false, key: OCHRE },
  // the rooms' faces on the street, with their windows
  { line: [[-26.5, -32.5], [-23.5, -32.5]], y: ROOMS, h: ROOM, t: 0.4, low: 0, crown: false, key: ROSE, holes: [{ at: 1.75, w: 1.3, h: 1.3, sill: 1, frame: true }] },
  { line: [[-18, -32.5], [-14.5, -32.5]], y: ROOMS, h: ROOM, t: 0.4, low: 0, crown: false, key: ROSE, holes: [{ at: 1.75, w: 1.3, h: 1.3, sill: 1, frame: true }] },
  { line: [[-14.5, -34], [-6, -34]], y: ROOMS, h: ROOM, t: 0.4, low: 0, crown: false, key: ROSE, holes: [{ at: 2.2, w: 1.3, h: 1.3, sill: 1, frame: true }, { at: 6.2, w: 1.3, h: 1.3, sill: 1, frame: true }] },
  { line: [[-26.5, -40.5], [-23.5, -40.5]], y: ROOMS, h: ROOM, t: 0.4, low: 0, crown: false, key: OCHRE, holes: [{ at: 1.75, w: 1.3, h: 1.3, sill: 1, frame: true }] },
  { line: [[-18, -40.5], [-16, -40.5]], y: ROOMS, h: ROOM, t: 0.4, low: 0, crown: false, key: OCHRE, holes: [{ at: 1, w: 1.1, h: 1.3, sill: 1, frame: true }] },
  // the stair in the alley yard: the door at its landing, the wall between its flights
  { line: [[-26.5, -23.5], [-23.5, -23.5]], y: 4.9, h: ROOM, crown: false, holes: [{ at: 1.1, w: 1.6, h: 2.3, frame: true }] },
  { line: [[-23.5, -23.5], [-21.5, -23.5]], y: 4, h: 7.3, crown: false },
  // the covered way out of the low start: an arch at each mouth
  { line: [[-46, -30], [-46, -25]], y: LOW, h: ROOM, crown: false, key: ROSE, holes: [{ at: 2.5, w: 3.2, h: 2.8, arch: 0.6 }] },
  { line: [[-33.5, -22.5], [-33.5, -18]], y: ALLEY, h: ROOM, crown: false, key: OCHRE, holes: [{ at: 2.25, w: 3, h: 2.8, arch: 0.6 }] },
  // the arch over the ramp to the middle, the arch at the neck
  { line: [[-18, -14], [-18, -10.5]], y: 4.3, h: 4.6, t: 0.7, key: RUBBLE, holes: [{ at: 1.75, w: 2.8, h: 3.2, arch: 0.6 }] },
  { line: [[-9.5, -10.5], [-6, -10.5]], y: MID, h: 5, t: 0.7, key: ROSE, holes: [{ at: 1.75, w: 2.8, h: 3.4, arch: 0.6 }] },
  // the passage south of the rooms: its door on the street
  { line: [[-4, -41.5], [-1.5, -41.5]], y: LANE, h: ROOM, crown: false, holes: [{ at: 1.25, w: 1.6, h: 2.4, arch: 0.3 }] },
  // the gallery: its door onto the deck, the deck's rails over the yard and over the garden
  { line: [[36, -40.5], [38, -40.5]], y: GALLERY, h: ROOM, crown: false, key: ROSE, holes: [{ at: 1, w: 1.5, h: 2.3, frame: true }] },
  { line: [[36, -37], [43, -37]], y: GALLERY, h: 1, t: 0.25, crown: false, key: ROSE },
  { line: [[43, -40.5], [43, -37]], y: GALLERY, h: 1, t: 0.25, crown: false, key: ROSE },
  // the hall: its arches south onto the west yard, its arch east, the arch from the north street to the east start
  { line: [[22.5, -16], [30.5, -16]], y: SITE, h: HALL, t: 0.6, crown: false, holes: [{ at: 2.1, w: 2.9, h: 3.2, arch: 0.6 }, { at: 5.9, w: 2.9, h: 3.2, arch: 0.6 }] },
  { line: [[40.5, -8.5], [40.5, -1.5]], y: SITE, h: HALL, t: 0.6, crown: false, holes: [{ at: 3.5, w: 4.6, h: 3.4, arch: 0.6 }] },
  { line: [[45.5, 33.5], [45.5, 36]], y: 6.4, h: 5, t: 0.6, holes: [{ at: 1.25, w: 2.1, h: 3, arch: 0.6 }] },
  // the rooms south of the east start: the door from the raised walk, the wall between the two, the door to the open way
  { line: [[49.5, 7.5], [58.5, 7.5]], y: SITE, h: ROOM, crown: false, holes: [{ at: 2.75, w: 1.6, h: 2.3, frame: true }, { at: 6.5, w: 1.4, h: 1.2, sill: 1, frame: true }] },
  { line: [[51.25, 2], [59.5, 2]], y: SITE, h: ROOM, t: 0.3, crown: false, holes: [{ at: 6.25, w: 3, h: 2.5 }] },
  { line: [[51.25, -7], [51.25, 2]], y: SITE, h: ROOM, crown: false, key: OCHRE, holes: [{ at: 1.75, w: 1.6, h: 2.3, frame: true }, { at: 5.5, w: 1.4, h: 1.2, sill: 1, frame: true }] },
];

/** Solids of any footprint: buttresses, posts, the stubs of walls in the hall. */
export const MASSES = [
  // the buttresses along the south alley's wall
  ...[-52.3, -47.6, -42.9, -38.2, -33.5].map((e) => ({ at: [[e - 0.55, -53], [e + 0.55, -53], [e + 0.55, -52], [e - 0.55, -52]], y: ALLEY, top: ALLEY + 4.2, key: RUBBLE })),
  // the porch's posts, the loggia's at the east start, the arcade's east of the yard
  ...[-16.2, -22.5, -28.8].map((n) => ({ at: [[44.2, n - 0.3], [44.8, n - 0.3], [44.8, n + 0.3], [44.2, n + 0.3]], y: SITE, top: SITE + HALL, key: 'qt_wood_dark' })),
  ...[11.4, 16.2, 21, 25.8, 30.6].map((n) => ({ at: [[56.3, n - 0.25], [56.8, n - 0.25], [56.8, n + 0.25], [56.3, n + 0.25]], y: EAST, top: EAST + 4.5, key: 'qt_wood_dark' })),
  ...[-17.3, -21.3, -25.3, -29.3, -33.2].map((n) => ({ at: [[55.4, n - 0.2], [55.8, n - 0.2], [55.8, n + 0.2], [55.4, n + 0.2]], y: SITE, top: SITE + 2.8, key: 'qt_wood_dark' })),
  // the roof of the room over the south street (its floor is a floor over a floor, which carries none)
  { at: [[-23.2, -40.5], [-18.3, -40.5], [-18.3, -32.5], [-23.2, -32.5]], y: ROOMS + ROOM, top: ROOMS + 7, key: ROSE },
  // the hall's stubs of walls
  { at: [[36.5, -4], [36.9, -4], [36.9, -1.5], [36.5, -1.5]], y: SITE, top: SITE + HALL },
  { at: [[24.3, -7.5], [24.7, -7.5], [24.7, -5], [24.3, -5]], y: SITE, top: SITE + HALL },
  { at: [[37.3, -8.5], [37.7, -8.5], [37.7, -6], [37.3, -6]], y: SITE, top: SITE + HALL },
  { at: [[27, -13.5], [28.6, -13.5], [28.6, -13.1], [27, -13.1]], y: SITE, top: SITE + HALL },
];

/** Doors of boards: `facing` is the way the leaves swing, in degrees from north toward east; `open` in degrees. */
export const DOORS = [
  { at: [-46, -27.5], facing: 270, w: 3.2, h: 2.7 },
  { at: [-25.4, -23.5], facing: 0, w: 1.6, h: 2.3, leaves: 1 },
  { at: [37, -40.5], facing: 0, w: 1.5, h: 2.3, leaves: 1 },
  { at: [52.25, 7.5], facing: 180, w: 1.6, h: 2.3, leaves: 1 },
  { at: [51.25, -5.25], facing: 90, w: 1.6, h: 2.3, leaves: 1, hinge: 1 },
  // shut: the door at the head of the alley's stair, the cellar's under the east start
  { at: [-33.42, -38.2], facing: 90, w: 1.3, h: 2.3, leaves: 1, open: 2 },
  { at: [28.08, 27], facing: 90, w: 4.4, h: 3, open: 2 },
];

/** Crates: [east, north, side, turn in degrees, up]. Without `up` a crate stands on the floor. */
export const CRATES = [
  // the low start, the south alley
  [-55.3, -27.2, 1.5, 0], [-55.2, -27.3, 1.35, 20, LOW + 1.5], [-54.9, -29.1, 1.4, 12],
  [-49.9, -51.8, 1.5, 0], [-48.3, -51.7, 1.7, 8], [-36.5, -47.4, 1.6, 0],
  // the alley yard, the south street under the room and east of it
  [-24.8, -11.4, 1.3, 0],
  [-21.6, -39.6, 1.6, 0], [-19.9, -39.6, 1.6, 6], [-10.9, -38.6, 1.5, 0], [-9.3, -38.5, 1.7, 10], [-3.9, -38.6, 1.5, 0], [6.6, -34.5, 1.6, 0],
  // the side street and its kerb
  [8.6, -43.7, 1.3, 0], [10.2, -43.6, 1.6, 0], [13, -30, 1.3, 0], [13, -30, 1.1, 20, 6.1 + 1.3],
  // the bent lane and its yard
  [-8.4, 5, 1.7, 25], [0.2, 18.3, 2, 30], [-1.4, 16.6, 1.9, 30], [0.5, 15.9, 1.4, 30], [5.9, 13, 1.4, 0], [7.3, 13, 1.4, 0], [11.1, 21.1, 1.5, 0],
  // the north yard: the two set cornerwise, the stack in its middle, those along its south wall
  [-5.3, 34.3, 2, 45], [-4.4, 32.5, 2, 45], [0, 39.2, 2.2, 0], [0, 39.2, 1.9, 15, SITE + 2.2], [-1.7, 40.3, 1.6, 0], [1.6, 38.4, 1.8, 0], [0.3, 37.3, 1.3, 0],
  [-7, 32.9, 1.4, 0], [-2, 29.3, 1.5, 0], [0.9, 29.2, 1.3, 0], [2.6, 29.7, 2.2, 0], [3.6, 31.3, 1.5, 10],
  // the north strip, the north street
  [8.9, 47.5, 2, 0], [25.2, 39.1, 1.5, 0], [27.1, 39.1, 1.5, 0], [29, 39.2, 1.1, 0], [23, 34.3, 1.5, 0], [35.5, 34.2, 1.3, 0], [36.8, 34.2, 1.2, 12],
  // the east start: the stack in the yard, the two by the raised walk's wall, one on the walk
  [47.3, 22, 1.6, 0], [47.4, 20.4, 1.5, 0], [47.3, 21.9, 1.2, 25, EAST + 1.6], [51.6, 11.7, 1.2, 0], [53.2, 11.8, 1.6, 0], [53.5, 8.4, 1.2, 15],
  // the west yard of the east site
  [23.3, -29.3, 1.3, 0], [23.4, -30.7, 1.4, 0],
  // the porch
  [36.3, -17.5, 1.5, 15], [39.8, -16.4, 1.6, 30], [41.8, -16.3, 1.6, 30], [40.3, -18.5, 1.2, 0], [39.3, -22, 1.6, 0], [41.2, -25.4, 1.6, 0], [40.4, -27.1, 1.6, 0], [42.1, -27.1, 1.6, 0], [36.3, -27.2, 1.5, 30],
  // along the yard's south wall: the two stacks under the deck, which reach it
  [31.8, -35.9, 1.6, 0], [39.75, -36.1, 1.65, 0], [39.75, -36.1, 1.6, 8, SITE + 1.65], [41.45, -36.15, 1.45, 0], [41.45, -36.15, 1.4, -6, SITE + 1.45], [47.8, -35.9, 1.7, 0], [49.9, -36.3, 0.8, 0],
];

/** Lamps under the roofs: [east, north]. */
export const LAMPS = [
  [-44.5, -27.5], [-41.5, -24], [-38, -20.2],
  [-25, -24.5], [-20.5, -24.5], [-22, -29], [-12, -28], [-10, -31], [-20.75, -36.5], [-21, -44.5], [-14.5, -45.2],
  [-7, -45.2], [-2.75, -46], [3, -51], [12.5, -50], [18.75, -51.3],
  [22, -42.75], [29, -42.75], [35, -42.75], [17.5, -30], [16.5, -37.5],
  [26.5, -5], [33, -5], [26.5, -12.5], [39.75, -19], [39.75, -26],
  [54.5, 4.75], [55.4, -2.5], [57.5, 16], [57.5, 26], [56.5, -22], [56.5, -29],
];

/** A site's mark on its floor: a square of painted bars with a bar across. */
export const MARKS = [
  { at: [-3.3, 36.4], size: 7 },
  { at: [39.8, -22.2], size: 8 },
];

/** The reference's props: ['palm', east, north, scale] or [id, east, north, turn, scale]. */
export const PROPS = [
  // the bed of trees east of the yard, one more in the sunken garden
  ['palm', 52.7, -27.1, 0.78],
  ['palm', 54.4, -27.3, 0.84],
  ['palm', 52.8, -30.1, 0.82],
  ['palm', 54.4, -29.9, 0.74],
  ['palm', 53.5, -43.5, 0.9],
  ['shrub', 46.5, -45, 0, 1.2],
  ['shrub', 49.5, -39.2, 40, 1],
  ['shrub', 55.6, -41, 80, 1.1],
  ['weeds', 51, -43, 0, 1.2],
  ['weeds', 45.5, -40.5, 30, 1.1],
  // barrels in the hall, under the loggia, in the porch and by the passage's mouth
  ['barrel_wood', 23.3, -2.4, 0, 1.1],
  ['barrel_wood', 24.4, -2.3, 40, 1.1],
  ['barrel_wood', 23.4, -14.9, 10, 1.1],
  ['barrel_wood', 39.5, -7.6, 0, 1.1],
  ['barrel_wood', 57.8, 28.4, 0, 1.3],
  ['barrel_wood', 57.8, 29.9, 30, 1.3],
  ['barrel_wood', 35.7, -24, 0, 1.1],
  ['barrel_wood', 35.8, -25.1, 50, 1.1],
  ['barrel_wood', -2, -40.9, 0, 1],
  // a table and chairs under the loggia, pots at doors
  ['table', 57.5, 20.5, 90, 1],
  ['chair', 57.5, 19.1, 0, 1],
  ['chair', 57.6, 21.9, 180, 1],
  ['planter', -47.3, -24.2, 0, 1],
  ['planter', -32.6, -17.2, 0, 1],
  ['planter', 12.8, 34.2, 0, 1],
  ['planter', 44.6, 30.7, 0, 1],
  ['planter', 31, 16.2, 0, 1],
  ['planter', 45.6, -14.8, 0, 1],
];
