/**
 * COUNTING HOUSE, a place of Pocket Duty.
 *
 * Drawn by hand, floor by floor, in the place's own coordinates: metres east and north of its middle,
 * heights in metres above its lowest floor (maps/README.md). North is up in the plan below.
 *
 *      -45      -35     -24      -6    0        +15  +20      +40   +48      +59
 *  +47  +--------+----------------------------------------------------------+
 *       |        |  north yard (5.5): snow all round the house, a low wall   |
 *  +36  |        |        +-----------------------------+--------+          |
 *       |        | NW     | GLASS  |      | north passage (10)   +---+      |
 *  +18  |        | yard   | ROOM   |solid | P |  +-------+ BACK ROOM |ramp  |
 *  +17  |        +=gate===+ (10)   |      | A |  | LONG  +-----------+ down |
 *       | watch  |        +--------+------+ S +--+ ROOM (10)   | dock (3.1) |
 *   +8  | way    | SIDE   | west rooms    | S |  +------+------+--(dock under)--+
 *       | (8)    | YARD   | over the way  +---+--+ mid  | EAST HALL (10)    |
 *    0  | two    | (5.5)  | under (4.7)   | solid | room |                  |
 *       | windows| van    +--crates-+-----+-------+------+-----+------------+
 *   -8  |        |        | ledge, 2 open windows | south passage (10)
 *       +--ramp--+        +--GARAGE (6.95)--stairs+---+   +--LOBBY (10)--+
 *  -18           |        +--watch room (8)--+        |   | 3 windows    |
 *  -25  +--------+=bridge=+--ramp from the lot--+     +---+ FRONT YARD   |
 *       | west   |  THE   |                     | entrance hall (6.95)   |
 *  -36  | room   |  LOT   +---------------------+--+ porch, steps (5.5)  |
 *       +--------+ (5.5)     THE STREET (5.5)     roller   van, drift    |
 *  -44           +---------------------------------+--------+------------+
 *
 * One house on a slope of two heights: snow yards at 5.5 on every side, the rooms at 10. The ways up are
 * the garage's stairs, the entrance hall's stairs, and the crates under the two open windows of the
 * ledge. A passage runs under the west rooms from the side yard to the yard north of the wall: the
 * rooms over it are floors over a floor, and so is the east hall over the south end of the dock. Two
 * ladders climb from the yards to the ledge at the rooms' floor, each by an open window.
 */

/** The map's name, its turn in the world (which side the sun is on), its walls' height, the starts. */
export const PLACE = {
  id: 'counting',
  title: 'Counting House',
  turn: 2.2,
  wall: 8,
  // [east, north, facing in degrees from north toward east, tag]. The first is the player's.
  starts: [
    // (the title shows this one's view: the side yard, the crates' climb to the ledge ahead, the garage to the right)
    [-33.5, -6.5, 95, 'side yard'],
    [-30, -40, 20, 'the lot'],
    [-32, -34, 10, 'the lot'],
    [-28, -31, 0, 'the lot'],
    [-30, -27, 0, 'the lot'],
    [-18, -40, 90, 'the street'],
    [0, -40, 90, 'the street'],
    [26, -28, 300, 'front yard'],
    [33, 12, 250, 'long room'],
    [38, 11, 250, 'long room'],
    [31, 15, 200, 'long room'],
    [36, 13.5, 230, 'long room'],
    [53, 3, 270, 'east hall'],
    [43, 23, 200, 'back room'],
    [17.5, 31.5, 180, 'north passage'],
    [-12, 23, 160, 'glass room'],
    [3, 10.2, 90, 'middle passage'],
  ],
};

// The heights the floors lie at: the yards, the rooms, the garage and the entrance hall, the watch
// rooms, the stairs' landings, the passage under the west rooms, the dock. TOP is where every wall ends.
const Y = 5.5, F = 10, G = 6.95, U = 8, L = 8.4, T = 4.7, P = 3.1, TOP = 13.5;
const SNOW = 'ch_snow', ROAD = 'ch_road', CARPET = 'ch_carpet', SLAB = 'ch_slab', ASPHALT = 'ch_asphalt';
const ROOF = 'ch_roof', PLASTER = 'ch_plaster', CEILING = 'ch_ceiling', CONCRETE = 'ch_concrete', BRICK = 'ch_brick', BLOCK = 'ch_block';

/** A rectangle from its west, south, east and north. */
const R = (w, s, e, n) => [[w, s], [e, s], [e, n], [w, n]];
/** A room of the house: a floor at 10 under a ceiling at 3.05, plastered, with carpet laid on it (CARPETS, below). */
const room = (at, more = {}) => ({ at, y: F, key: SLAB, carpet: true, roof: 3.05, walls: TOP - F, wall: PLASTER, roofKey: CEILING, crown: false, ...more });
/** A doorway through a wall between two rooms: the same floor under a lintel. */
const door = (at, more = {}) => room(at, { roof: 2.3, ...more });
/** A hall of bare slab under a roof: `y` is its floor, or its slope. */
const hall = (at, y, more = {}) => ({ at, y, key: SLAB, roof: 3.4, walls: TOP - (typeof y === 'number' ? y : Math.max(y.from[2], y.to[2])), wall: CONCRETE, roofKey: CONCRETE, crown: false, ...more });

/** The floors: polygons of [east, north], level (`y`) or sloped (`y: { from, to }`, each [east, north, up]). */
export const GROUND = [
  // ------------------------------------------------------------------ the lot, the bridge over its north end, the street
  { at: R(-34, -44, -26, -25), y: Y, key: ROAD, wall: CONCRETE },
  { at: R(-34, -25, -26, -21), y: Y, key: ROAD, roof: 4.6, wall: CONCRETE, roofKey: CONCRETE },
  { at: [[-26, -44], [-12, -44], [-12, -45], [-6, -45], [-6, -36], [-26, -36]], y: Y, key: ROAD, wall: CONCRETE },
  { at: [[-6, -45], [-4.5, -45], [-4.5, -44], [14.3, -44], [14.3, -36], [-6, -36]], y: Y, key: ROAD, wall: CONCRETE },
  // ------------------------------------------------------------------ the side yard, its drift up to the crates, the wall north of it (WALLS)
  { at: [[-35, -21], [-24, -21], [-24, -18], [-10.5, -18], [-10.5, -15.5], [-14, -15.5], [-14, -10.5], [-10.5, -10.5], [-10.5, -8], [-7.5, -8], [-7.5, -2.5], [-24, -2.5], [-24, 5.5], [-26.5, 5.5], [-26.5, 8], [-24, 8], [-24, 12], [-24.5, 12], [-24.5, 17.5], [-35, 17.5], [-35, 8], [-37.5, 8], [-37.5, -13], [-35, -13]], y: Y },
  { at: [[-17.5, -2.5], [-17.5, -7], [-14, -8], [-7.5, -8], [-7.5, -2.5]], y: { from: [-17.5, -5, Y], to: [-12.5, -5, G] } },
  // ------------------------------------------------------------------ the yards north and east of the house, the ramp down to the dock
  { at: [[-35, 17.5], [-24, 17.5], [-24, 19.5], [-26.5, 19.5], [-26.5, 22.5], [-24, 22.5], [-24, 30.5], [-18, 30.5], [-18, 47], [-35, 47]], y: Y },
  { at: R(-18, 36, 12, 47), y: Y },
  { at: R(12, 36, 42, 47), y: Y },
  { at: R(42, 30, 59, 47), y: Y },
  { at: [[48, 20], [50, 20], [50, 28], [57.5, 28], [57.5, 19.5], [59, 19.5], [59, 30], [48, 30]], y: Y },
  { at: R(50, 19.5, 57.5, 28), y: { from: [53.5, 28, Y], to: [53.5, 19.5, P] }, key: ASPHALT, side: CONCRETE },
  { at: R(50, 10, 57.5, 19.5), y: P, key: ASPHALT, roof: 6.4, walls: TOP - P, wall: CONCRETE, roofKey: CONCRETE, crown: false },
  // (the dock's south end lies under the east hall: listed here, before the hall's floor over it)
  { at: R(50, 6.5, 57.5, 10), y: P, key: ASPHALT, walls: TOP - P, wall: CONCRETE, crown: false },
  // ------------------------------------------------------------------ the front yard, its drift, the covered corner
  { at: [[14.3, -44], [16.5, -44], [16.5, -46], [33, -46], [33, -36], [37, -36], [37, -25.5], [33.5, -25.5], [33.5, -19], [19, -19], [19, -26.5], [23, -26.5], [23, -31.5], [19, -31.5], [19, -34.5], [16.5, -34.5], [16.5, -36], [14.3, -36]], y: Y, wall: BRICK },
  { at: R(27, -45.5, 33, -37), y: { from: [27, -41, Y], to: [33, -41, 7.15] }, side: SNOW },
  { at: R(33, -45.5, 37.5, -40.5), y: 7.15, key: SLAB, roof: 3.4, walls: TOP - 7.15, wall: BRICK, roofKey: CONCRETE, crown: false },
  { at: R(33, -40.5, 37.5, -36), y: { from: [35, -40.5, 7.15], to: [35, -36, Y] }, key: SLAB, roof: 3.4, walls: TOP - 7.15, wall: BRICK, roofKey: CONCRETE, crown: false },
  // ------------------------------------------------------------------ the room west of the lot, the ramp up from it, the watch way over the side yard
  { at: R(-45, -31.5, -35, -24.5), y: Y, key: ROAD, wall: CONCRETE },
  { at: R(-35, -29, -34, -27), y: Y, key: ROAD, roof: 2.5, wall: CONCRETE, roofKey: CONCRETE },
  hall(R(-41.5, -24.5, -38.5, -16), { from: [-40, -24.5, Y], to: [-40, -16.5, U] }),
  hall(R(-42.5, -16, -37.5, 7), U, { side: BLOCK }),
  // ------------------------------------------------------------------ the ramp east of the lot, the watch room over the side yard
  { at: R(-26, -29.5, -24.5, -27), y: Y, key: ROAD, roof: 2.5, wall: CONCRETE, roofKey: CONCRETE },
  hall(R(-24.5, -29.5, -22.5, -26.5), Y, { roof: 5.9 }),
  hall(R(-22.5, -29.5, -15.5, -26.5), { from: [-22.5, -28, Y], to: [-15.5, -28, U] }),
  hall(R(-15.5, -29.5, -11, -26.5), U),
  hall(R(-14.5, -26.5, -11.5, -23.5), U),
  hall(R(-22.5, -23.5, -11, -18), U, { side: BLOCK }),
  // ------------------------------------------------------------------ the garage, and its stairs up to the rooms
  { at: R(-14, -15.5, -10.5, -10.5), y: { from: [-14, -13, Y], to: [-10.5, -13, G] }, key: ASPHALT, side: CONCRETE },
  hall(R(-10.5, -15.5, 1.5, -10.5), G, { key: ASPHALT }),
  hall(R(-3.5, -19, 0, -15.5), { from: [-1.75, -15.5, G], to: [-1.75, -19, L] }, { steps: true }),
  hall(R(-3.5, -21.5, 4.5, -19), L),
  hall(R(1.5, -19, 4.5, -14), L),
  hall(R(1.5, -14, 4.5, -8.5), { from: [3, -14, L], to: [3, -9.5, F] }, { steps: true, roof: 3.05 }),
  // ------------------------------------------------------------------ the entrance: the porch and its steps, the hall, its stairs
  { at: R(19.5, -31.5, 23, -26.5), y: { from: [19.5, -29, G], to: [23, -29, Y] }, key: SLAB, steps: true, side: BRICK },
  hall(R(14.5, -31.5, 19.5, -26.5), G, { wall: BRICK, roof: 3.2 }),
  hall(R(13, -30.5, 14.5, -27), G, { roof: 2.5 }),
  hall([[7.5, -31.5], [10, -31.5], [10, -32.5], [12.5, -32.5], [12.5, -31.5], [13, -31.5], [13, -25.5], [7.5, -25.5]], G),
  hall(R(10, -36, 12.5, -32.5), { from: [11.25, -36, Y], to: [11.25, -32.5, G] }, { steps: true, roof: 3 }),
  hall(R(9.5, -25.5, 13, -21), { from: [11.25, -25, G], to: [11.25, -21, L] }, { steps: true }),
  hall(R(9.5, -21, 13.5, -13.5), L),
  hall(R(13.5, -16.5, 17.5, -13.5), { from: [13.5, -15, L], to: [17.5, -15, F] }, { steps: true, roof: 3.05 }),
  // ------------------------------------------------------------------ the ledge under the two open windows
  { at: [[-24, -2.5], [-7.5, -2.5], [-7.5, -8], [-6.5, -8], [-6.5, -1.5], [-24, -1.5]], y: 9.6, key: SLAB, walls: TOP - 9.6, side: BLOCK, crown: false },
  // the ledges the two ladders reach: on the west face by the west room's window, on the north face under the glass room's
  { at: R(-24.5, 12, -24, 17.5), y: 9.6, key: SLAB, walls: TOP - 9.6, side: BLOCK, crown: false },
  { at: R(-24, 30, -18, 30.5), y: 9.6, key: SLAB, walls: TOP - 9.6, side: BLOCK, crown: false },
  // ------------------------------------------------------------------ the passage under the west rooms (before the floors over it)
  { at: R(-26.5, 5.5, -24, 8), y: { from: [-26.5, 6.75, Y], to: [-24, 6.75, T] }, side: SNOW },
  { at: R(-26.5, 19.5, -24, 22.5), y: { from: [-26.5, 21, Y], to: [-24, 21, T] }, side: SNOW },
  { at: R(-24, 5.5, -22.5, 8), y: T, key: SLAB, roof: 2.6, walls: TOP - T, wall: CONCRETE, roofKey: CONCRETE, crown: false },
  { at: R(-22.5, 5.5, -19, 8), y: T, key: SLAB, roof: 2.6, walls: 4.9, wall: CONCRETE, roofKey: CONCRETE },
  { at: R(-19, 5.5, -16.5, 6.5), y: T, key: SLAB, roof: 2.6, walls: 4.9, wall: CONCRETE, roofKey: CONCRETE },
  { at: [[-19, 6.5], [-16.5, 6.5], [-16.5, 5.5], [-15.5, 5.5], [-15.5, 8], [-19, 8]], y: T, key: SLAB, roof: 2.6, walls: 4.9, wall: CONCRETE, roofKey: CONCRETE },
  { at: R(-19, 8, -15.5, 16), y: T, key: SLAB, roof: 2.6, walls: 4.9, wall: CONCRETE, roofKey: CONCRETE },
  { at: R(-19, 16, -18.5, 19), y: T, key: SLAB, roof: 2.6, walls: 4.9, wall: CONCRETE, roofKey: CONCRETE },
  { at: R(-18.5, 16, -17, 19), y: T, key: SLAB, roof: 2.6, walls: 4.9, wall: CONCRETE, roofKey: CONCRETE },
  { at: R(-17, 16, -15.5, 19), y: T, key: SLAB, roof: 2.6, walls: 4.9, wall: CONCRETE, roofKey: CONCRETE },
  { at: R(-19, 19, -15.5, 22.5), y: T, key: SLAB, roof: 2.6, walls: 4.9, wall: CONCRETE, roofKey: CONCRETE },
  { at: R(-22.5, 19.5, -19, 22.5), y: T, key: SLAB, roof: 2.6, walls: 4.9, wall: CONCRETE, roofKey: CONCRETE },
  { at: R(-24, 19.5, -22.5, 22.5), y: T, key: SLAB, roof: 2.6, walls: TOP - T, wall: CONCRETE, roofKey: CONCRETE, crown: false },
  // The rooms' floor over the passage: slabs with an underside, round the two piers of the rooms. Soldiers have both floors.
  room(R(-22.5, 19, -15, 23), { over: true, rail: 0, side: CONCRETE }),
  room([[-19.5, 8.5], [-15, 8.5], [-15, 19], [-17, 19], [-17, 16], [-18.5, 16], [-18.5, 19], [-19.5, 19]], { over: true, rail: 0, side: CONCRETE }),
  room(R(-22.5, 6.5, -15, 8.5), { over: true, rail: 0, side: CONCRETE }),
  room(R(-22.5, 4.5, -19, 6.5), { over: true, rail: 0, side: CONCRETE }),
  room(R(-16.5, 4.5, -15, 6.5), { over: true, rail: 0, side: CONCRETE }),
  room(R(-19, 4.5, -16.5, 5.5), { over: true, rail: 0, side: CONCRETE }),
  // ------------------------------------------------------------------ the west rooms: the glass room, the rooms south of it
  room([[-15, 19], [-6, 19], [-6, 27.5], [-9, 27.5], [-9, 28], [-10, 28], [-10, 29.5], [-13, 29.5], [-13, 28], [-14, 28], [-14, 27.5], [-17.5, 27.5], [-17.5, 30], [-24, 30], [-24, 23], [-15, 23]], { side: BLOCK }),
  room([[-15, 15.5], [-9.5, 15.5], [-9.5, 17.5], [-6, 17.5], [-6, 19], [-15, 19]]),
  room(R(-21, 15.5, -19.5, 19)),
  room(R(-24, 12, -19.5, 15.5), { side: BLOCK }),
  room(R(-15, 12.5, -8.5, 15.5)),
  room(R(-21, 10, -19.5, 12)),
  room(R(-22.5, 8.5, -19.5, 10)),
  room(R(-22.5, -0.5, -16.5, 4.5)),
  room([[-16, -1.5], [-12, -1.5], [-12, 8], [-15, 8], [-15, 4.5], [-16, 4.5]]),
  room(R(-12, 1.5, -6.5, 8)),
  // ------------------------------------------------------------------ the passages: middle, south, the two between them, the north one
  room(R(-15, 8, 20.5, 12.5)),
  room(R(0.5, 12.5, 5.5, 16.5)),
  room(R(0.5, -2.5, 5.5, 8)),
  room(R(-6.5, -7.5, 28, -2.5), { side: BLOCK }),
  room(R(0.5, -8.5, 5.5, -7.5)),
  room([[16, -2.5], [19.5, -2.5], [19.5, 8], [16, 8], [16, 5.5], [15, 5.5], [15, 0], [16, 0]]),
  room(R(15, 12.5, 20, 29.5)),
  room(R(13, 17, 15, 24.5)),
  room(R(13.5, 29.5, 40, 34.5)),
  room(R(22.5, -2.5, 27.5, 8.5)),
  room(R(22.5, -12.5, 28, -7.5)),
  // ------------------------------------------------------------------ the lobby over the front yard
  room([[17.5, -16.5], [21.5, -16.5], [21.5, -19], [31, -19], [31, -16.5], [32, -16.5], [32, -13.5], [31, -13.5], [31, -12.5], [21.5, -12.5], [21.5, -13.5], [17.5, -13.5]], { side: BRICK }),
  // ------------------------------------------------------------------ the long room, the back room, the way between them
  room([[22.5, 8.5], [29, 8.5], [29, 9], [40.5, 9], [40.5, 15.5], [41.5, 15.5], [41.5, 18], [36.5, 18], [36.5, 15.5], [35.5, 15.5], [35.5, 17], [29, 17], [29, 14], [25.5, 14], [22.5, 11]]),
  room(R(30.5, 17, 32.5, 22.5)),
  room([[32.5, 20.5], [34, 20.5], [34, 19], [46.5, 19], [46.5, 29], [41, 29], [41, 27.5], [38, 27.5], [38, 29.5], [35, 29.5], [35, 27.5], [34, 27.5], [34, 22.5], [32.5, 22.5]]),
  // ------------------------------------------------------------------ the middle room, the south-east room, its closet
  door(R(27.5, 2.5, 29, 5.5)),
  room(R(29, 1.5, 39, 6)),
  door(R(34.5, -1, 38, 1.5)),
  room(R(34, -10.5, 42, -1)),
  door(R(33, -11.5, 35, -10.5), { roof: 2 }),
  room(R(32.5, -14.5, 38, -11.5)),
  // ------------------------------------------------------------------ the east hall and its bays
  door(R(39, 2.5, 40.5, 5.5)),
  door(R(40.5, 10.5, 43.5, 12.5)),
  door(R(42, -5, 43.5, -3)),
  room([[40.5, 1], [43.5, 1], [43.5, -3], [60, -3], [60, 10], [58, 10], [58, 6], [49.5, 6], [49.5, 14], [43.5, 14], [43.5, 7.5], [40.5, 7.5]]),
  // (its floor over the dock's south end: a slab; soldiers have both floors)
  room(R(49.5, 6, 58, 10), { over: true, rail: 0, side: CONCRETE }),
  room(R(43.5, -7, 50, -3)),
];

/** Walls along a line, with what is cut in them: `holes` of `{ at, w, h, sill, arch, frame, glass }`, `at` in metres from the first point. */
export const WALLS = [
  // ---- the wall between the side yard and the yard north of it: the way round it is the passage under the west rooms
  { line: [[-35, 17.5], [-24.5, 17.5]], y: Y, h: 8, t: 1, key: BLOCK },
  // ---- windows. The glass room's, west and north: panes, but for the one over the north ladder, which is open.
  { line: [[-24, 23], [-24, 30]], y: F, h: 3.5, t: 0.4, key: CONCRETE, holes: [{ at: 1.9, w: 2.6, h: 1.9, sill: 0.45, frame: true, glass: true }, { at: 5.1, w: 2.6, h: 1.9, sill: 0.45, frame: true, glass: true }] },
  { line: [[-24, 30], [-17.5, 30]], y: F, h: 3.5, t: 0.4, key: CONCRETE, holes: [{ at: 1.75, w: 2.4, h: 1.9, sill: 0.45, frame: true, glass: true }, { at: 4.75, w: 2.4, h: 1.9, sill: 0.45, frame: true }] },
  // The west room's window over the side yard: open, with its frame.
  { line: [[-24, 12], [-24, 15.5]], y: F, h: 3.5, t: 0.4, key: CONCRETE, holes: [{ at: 1.75, w: 2.6, h: 1.9, sill: 0.45, frame: true }] },
  // The two windows of the ledge, which a player climbs through: open, with their frames.
  { line: [[-16, -1.5], [-12, -1.5]], y: F, h: 3.5, t: 0.4, key: CONCRETE, holes: [{ at: 1.6, w: 2.5, h: 1.9, sill: 0.45, frame: true }] },
  { line: [[-6.5, -7.5], [-6.5, -2.5]], y: F, h: 3.5, t: 0.4, key: CONCRETE, holes: [{ at: 2.5, w: 3.6, h: 1.9, sill: 0.45, frame: true }] },
  // The lobby's three over the front yard: two open (the first over the container a player climbs), one pane.
  { line: [[21.5, -19], [31, -19]], y: F, h: 3.5, t: 0.4, key: BRICK, holes: [{ at: 1.7, w: 2.6, h: 1.9, sill: 0.45, frame: true }, { at: 4.75, w: 2.6, h: 1.9, sill: 0.45, frame: true }, { at: 7.8, w: 2.6, h: 1.9, sill: 0.45, frame: true, glass: true }] },
  // The watch room's and the watch way's, over the side yard: open, with their frames.
  { line: [[-22.5, -18], [-11, -18]], y: U, h: TOP - U, t: 0.4, key: BLOCK, holes: [{ at: 2, w: 3, h: 1.6, sill: 0.8, frame: true }, { at: 5.75, w: 3, h: 1.6, sill: 0.8, frame: true }, { at: 9.5, w: 3, h: 1.6, sill: 0.8, frame: true }] },
  { line: [[-37.5, -13], [-37.5, 7]], y: U, h: TOP - U, t: 0.4, key: BLOCK, holes: [{ at: 4.9, w: 2.5, h: 1.6, sill: 0.8, frame: true }, { at: 7.7, w: 2.5, h: 1.6, sill: 0.8, frame: true }, { at: 13.4, w: 2.5, h: 1.6, sill: 0.8, frame: true }, { at: 16.2, w: 2.5, h: 1.6, sill: 0.8, frame: true }] },
  // ---- the rooms' walls where a floor over the passage ends at one, and the wall between the west rooms and the way beside them
  { line: [[-22.5, 19], [-21, 19]], y: F, h: 3.5, t: 0.3, key: PLASTER, crown: false },
  { line: [[-16.25, 4.5], [-16.25, 12]], y: F, h: 3.5, t: 0.3, key: PLASTER, crown: false },
  // ---- doorways of the house: a wall across a passage with its frame
  { line: [[-3, 8], [-3, 12.5]], y: F, h: 3.5, t: 0.3, key: PLASTER, crown: false, holes: [{ at: 2.25, w: 3, h: 2.3, frame: true }] },
  { line: [[0.5, -8.5], [5.5, -8.5]], y: F, h: 3.5, t: 0.3, key: PLASTER, crown: false, holes: [{ at: 2.5, w: 3, h: 2.3, frame: true }] },
  { line: [[16, 7.5], [19.5, 7.5]], y: F, h: 3.5, t: 0.3, key: PLASTER, crown: false, holes: [{ at: 1.75, w: 2.5, h: 2.3, frame: true }] },
  { line: [[16, -1], [19.5, -1]], y: F, h: 3.5, t: 0.3, key: PLASTER, crown: false, holes: [{ at: 1.75, w: 2.5, h: 2.3, frame: true }] },
  { line: [[22.5, 0], [27.5, 0]], y: F, h: 3.5, t: 0.3, key: PLASTER, crown: false, holes: [{ at: 2.5, w: 3, h: 2.3, frame: true }] },
  { line: [[17.5, -16.5], [17.5, -13.5]], y: F, h: 3.5, t: 0.3, key: PLASTER, crown: false, holes: [{ at: 1.5, w: 2.5, h: 2.3, frame: true }] },
  // ---- the garage: the wall across it, the wall beside the stairs; the dock's front
  { line: [[-5, -15.5], [-5, -10.5]], y: G, h: TOP - G, t: 0.4, key: CONCRETE, crown: false, holes: [{ at: 2.5, w: 3.5, h: 2.55 }] },
  { line: [[1.5, -15.5], [1.5, -10.5]], y: G, h: TOP - G, t: 0.4, key: CONCRETE, crown: false },
  { line: [[50, 19.5], [57.5, 19.5]], y: P, h: TOP - P, t: 0.5, key: CONCRETE, holes: [{ at: 3.75, w: 5, h: 3.8 }] },
];

/** Doors: leaves standing open (`facing`: the way they swing, in degrees from north toward east), and roller doors raised. */
export const DOORS = [
  // the street's door to the front yard, the garage's, the dock's
  { at: [14.3, -40], facing: 90, w: 7.6, h: 3.6, kind: 'roller', raised: 0.64 },
  { at: [-10.5, -13], facing: 270, w: 5, h: 3.4, kind: 'roller', raised: 0.7 },
  { at: [53.75, 19.5], facing: 0, w: 5, h: 3.8, kind: 'roller', raised: 0.6 },
  // leaves in the house's doorways
  { at: [-3, 10.25], facing: 90, w: 3, h: 2.25, y: F },
  { at: [3, -8.5], facing: 0, w: 3, h: 2.25, y: F },
  { at: [17.75, 7.5], facing: 180, w: 2.5, h: 2.25, y: F, leaves: 1, hinge: 1 },
  { at: [25, 0], facing: 0, w: 3, h: 2.25, y: F },
  { at: [28.25, 4], facing: 90, w: 2.6, h: 2.25, y: F, leaves: 1, hinge: -1 },
  { at: [39.75, 4], facing: 90, w: 2.6, h: 2.25, y: F, leaves: 1, hinge: 1 },
  { at: [34, -11], facing: 180, w: 1.8, h: 1.95, y: F, leaves: 1, hinge: -1 },
];

/**
 * The rooms' carpet: a thin solid on each room's floor. The cook lights the surface of every floor as open
 * ground under the sun and leaves its shade to the ground's shadow picture; a solid is lit by the rooms'
 * lamps, as a room's floor is.
 */
const CARPETS = GROUND.filter((g) => g.carpet).map((g) => ({ at: g.at, y: F, top: F + 0.03, key: CARPET }));

/** Solids: the wrecks and containers of the yards, the pilasters of the street, the house's furniture. */
export const MASSES = [
  ...CARPETS,
  // the van of the side yard, the containers of the crates' climb and of the street
  { at: [[-31, -9.4], [-26.4, -10.6], [-26.9, -12.4], [-31.5, -11.2]], y: Y, top: 7.4, key: 'ch_van' },
  { at: [[-31.5, -11.2], [-26.9, -12.4], [-28, -16.6], [-32.6, -15.4]], y: Y, top: 8.2, key: 'ch_van_box' },
  { at: R(-19.5, -5.5, -13.5, -2.5), y: Y, top: 9, key: 'ch_tin' },
  { at: R(-9.5, -39, -3.5, -36), y: Y, top: 9, key: 'ch_tin' },
  // the yard north of the wall: two containers
  { at: R(-34, 35.8, -27.9, 38.4), y: Y, top: 9, key: 'ch_tin' },
  { at: [[-23.6, 40.7], [-17.9, 37.4], [-19.6, 34.6], [-25.3, 37.9]], y: Y, top: 9, key: 'ch_van' },
  // the front yard: the van, the container under the lobby's windows
  { at: [[25.4, -33.4], [27.2, -32.9], [28.4, -37.3], [26.6, -37.8]], y: Y, top: 7.4, key: 'ch_van' },
  { at: [[21.3, -34.4], [25.4, -33.4], [26.6, -37.8], [22.5, -39]], y: Y, top: 8.2, key: 'ch_van_box' },
  { at: R(19.7, -22.3, 26, -19), y: Y, top: 8.75, key: 'ch_tin' },
  // the street's pilasters, on both sides
  ...[-24.6, -15.6, -2.9, 6].flatMap((w) => [
    { at: R(w, -36.7, w + 2.5, -36), y: Y, top: 10.2, key: CONCRETE, crown: true },
    { at: R(w, -44, w + 2.5, -43.3), y: Y, top: 10.2, key: CONCRETE, crown: true },
  ]),
  // the roofs: tar under drifted snow, over the house, the bridge and the watch way, in pieces no longer than 40 m
  ...[
    R(-18, 30, 12, 36), R(12, 30, 42, 36), R(-24, 19.5, 12, 30), R(12, 19.5, 48, 30), R(-24, -1.5, 4, 19.5), R(4, -1.5, 32, 19.5), R(32, -1.5, 60.6, 19.5),
    R(-6.5, -8, 27, -1.5), R(27, -8, 60.6, -1.5), R(-10.5, -18, 25, -8), R(25, -18, 60.6, -8), R(-24, -19, 4, -18), R(4, -19, 32, -18), R(32, -19, 60.6, -18),
    R(-24, -21, -2, -19), R(-2, -21, 19, -19), R(-26, -34.5, -3, -21), R(-3, -34.5, 19, -21), R(-26, -36, -5, -34.5), R(-5, -36, 16.5, -34.5), R(33.5, -25.5, 60.6, -19),
    R(-34, -25, -26, -21), R(-43.1, -13, -37.5, 7.6), R(-43.1, -24.5, -35, -13),
  ].map((at) => ({ at, y: TOP, top: TOP + 0.12, key: ROOF })),
  // the ledge round the house at the rooms' floor, north, west and east, with the snow on it (none over
  // the passage's mouth: a solid over walked ground takes it from the walk grid)
  ...[R(-18, 36, 12, 36.5), R(12, 36, 42, 36.5), R(-24.5, 23.2, -24, 30.5), R(-18.5, 30.5, -18, 36.5), R(42, 30, 42.5, 36.5), R(42.5, 30, 48, 30.5), R(47.5, 20.5, 48, 30)].flatMap((at) => [
    { at, y: 9.25, top: 9.6, key: CONCRETE },
    { at, y: 9.6, top: 9.68, key: 'ch_cap' },
  ]),
  // the two piers of the west rooms that stand over the passage
  { at: R(-19, 5.5, -16.5, 6.5), y: F - 0.3, top: TOP, key: PLASTER },
  { at: R(-18.5, 16, -17, 19), y: F - 0.3, top: TOP, key: PLASTER },
  // furniture: the glass room's table, the desks, the long room's counter, the east hall's pier
  { at: R(-11, 21, -9, 24), y: F, top: F + 0.8, key: 'ch_desk' },
  { at: R(-12, 4.5, -10, 7), y: F, top: F + 0.8, key: 'ch_desk' },
  { at: R(31, 9, 38.5, 9.8), y: F, top: F + 1, key: 'ch_desk' },
  { at: R(33.5, 14.5, 35.5, 15.5), y: F, top: F + 0.8, key: 'ch_desk' },
  { at: R(39.5, 23.5, 40.7, 27.2), y: F, top: F + 1, key: 'ch_desk' },
  { at: R(47.5, 1.5, 49, 6), y: F, top: F + 3.05, key: PLASTER },
  { at: R(44.2, 2.6, 46.2, 4.8), y: F, top: F + 0.8, key: 'ch_desk' },
  { at: R(30, 2.2, 33.5, 3.3), y: F, top: F + 0.8, key: 'ch_desk' },
  { at: R(39.6, -9.6, 41.4, -6.4), y: F, top: F + 0.8, key: 'ch_desk' },
  { at: R(-13.5, 13.4, -10.5, 14.6), y: F, top: F + 0.8, key: 'ch_desk' },
  { at: R(30, 15.8, 34.5, 16.8), y: F, top: F + 0.8, key: 'ch_desk' },
];

/**
 * The place's two ladders, from a yard up to the ledge at the rooms' floor: in the side yard by the wall, to the
 * west room's open window; north of the wall, to the glass room's open window. A player climbs them; the
 * ledges are half a metre wide and hold no node, so they are not a soldier's way.
 */
export const LADDERS = [
  { at: [-24.75, 15.9], facing: 270, h: 4.1, reach: 0.5 },
  { at: [-18.7, 30.75], facing: 0, h: 4.1, reach: 0.5 },
];

/** Crates: [east, north, side, turn in degrees, up]. Without `up` a crate stands on the floor. */
export const CRATES = [
  // the side yard's climb to the ledge: one, the next, two stacked, then the container's top
  [-22.6, -3.4, 1, 0], [-20.9, -3.3, 1.5, 0], [-20.9, -4.9, 1.5, 0], [-20.9, -4.9, 1.5, 8, Y + 1.5],
  // by the yard's north wall, and north of it under the glass room
  [-27.6, 16.2, 1, 12], [-26, 16.1, 1.5, 0], [-25, 27.6, 1.5, 0], [-25.8, 25.2, 1.3, 15], [-33, 34.7, 1.4, 0], [-31.3, 34.6, 1.2, 20],
  // the lot, the room west of it, the street
  [-33, -43, 1.5, 0], [-31.1, -42.8, 1.6, 14], [-37.5, -30.9, 1.2, 0], [-8.2, -40.1, 1.5, 12], [-6.6, -39.9, 1.4, 0], [5.2, -40.6, 1.6, 20], [4.6, -42.8, 1.4, 0], [6.1, -42.9, 1.4, 0],
  // the front yard: beside the container, a way up to it
  [27.3, -20, 1.6, 12], [27.2, -21.9, 1.5, 0], [29.2, -21.8, 1.4, 14], [30, -41, 1.4, 20],
  // in the house
  [10.2, -4.6, 1.2, 20], [11.6, 9.1, 1.2, 0], [2.6, 3.8, 1.2, 10], [-20.5, 0.5, 1.2, 0], [52.5, -1.8, 1.3, 0], [36.5, -8.8, 1.2, 15],
];

/** The tops of what stands beyond the outer walls, where they are lower than the house's: a wall rises to its block. */
export const BLOCKS = [
  // the low wall round the yards north and east of the house, of two heights so that no run is long
  { at: R(-43, 47, -18, 55), top: 7.6 },
  { at: R(-18, 47, 12, 55), top: 7.9 },
  { at: R(12, 47, 42, 55), top: 7.6 },
  { at: R(42, 47, 67, 55), top: 7.9 },
  { at: R(59, 19.5, 67, 47), top: 7.6 },
  { at: R(-43, 17.5, -35, 47), top: 7.9 },
  // south of the street and the lot, west of the room
  { at: [[-53, -52], [-20, -52], [-20, -44], [-34, -44], [-34, -31.5], [-45, -31.5], [-45, -24.5], [-53, -24.5]], top: 10.5 },
  { at: R(-20, -52, 16.5, -44), top: 10.9 },
];

/** Lamps under the roofs: [east, north]. A room has one or more; the passage under the west rooms has its own. */
export const LAMPS = [
  // under the east hall, in the dock's south end: [east, north, up]
  [53.75, 8.4, 8.9],
  // the bridge, the watch way and its ramp, the watch room and its ramp
  [-30, -23], [-40, -20], [-40, -11], [-40, -3], [-40, 4], [-19, -28], [-13, -28], [-13, -22], [-18, -21],
  // the garage and its stairs
  [-8, -13], [-1.5, -13], [0.5, -20.25], [3, -12],
  // the entrance: the street's stair, the hall, the porch, the stairs up, the lobby
  [11.25, -34], [10.2, -28.5], [17, -29], [11.25, -23], [11.5, -17], [15.5, -15], [26.5, -15.5], [25.2, -10],
  // the passages
  [-2.5, -5], [8, -5], [17.5, -5], [25, -5], [3, 2.5], [3, 14.5], [-9, 10.2], [3.5, 10.2], [11, 10.2], [17.5, 10.2], [17.7, 3], [17.5, 17], [17.5, 24], [17.5, 32], [26, 32], [34, 32], [25, 4],
  // the west rooms
  [-14, 2], [-9, 5], [-19.5, 2], [-21.8, 13.7], [-11.5, 14], [-12, 17.2], [-12, 25.5], [-19.5, 26],
  // the middle room, the south-east room, its closet, the long room, the back room, the east hall, the dock
  [34, 3.7], [38, -6], [46.5, -5], [35.2, -13], [26, 11], [33, 12.5], [38.5, 12.5], [31.5, 19.5], [37, 24], [43.5, 22], [43.8, 27], [45.5, 5.8], [52, 0.5], [56, 6.5], [46.8, 12], [53.75, 15],
  // the covered corner of the front yard, and the passage under the west rooms
  [35.2, -42], [-20.7, 6.75], [-17.25, 12], [-20.7, 21],
];
