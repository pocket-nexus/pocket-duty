/**
 * DEPOT, a place of Pocket Duty.
 *
 * Drawn by hand, floor by floor, in the place's own coordinates: metres east and north of its middle,
 * heights in metres above its lowest floor (maps/README.md). North is up in the plan below.
 *
 *      -49  -40      -27  -17 -12        0                    36  41
 *  +51                |    |             +---- stair ----+
 *  +47   +----+       | r  |   +--------+ back walk       +---+
 *        |    |       | a  |   | room   | gallery over it     |
 *  +44   | f  |       | i  |   | (8.5)  +---------------------+
 *        | a  |       | s  |   +--+ bal-|  NORTH DOCK         |
 *  +31   | r  +-------+ e  +------+ cony| (3.25 west, 0.8 east)
 *        |            | d  gate   +=====+=====================+ side
 *  +21   | w  yard    +----+ FRONT D  THE LANE (0) under the   | door
 *        | e  beyond  |pier| YARD   +  walked roof (13)   stack| alley
 *  +14   | s  +-------+----+      +--+ SOUTH DOCK, gallery    | + the
 *        | t  |       |    | strip|C1+------------------------+ stair
 *   +8   |    | block |    | (a   +                           +---+
 *        | l  |       |pier| solid)    C2        the open ground  |
 *   -2   | a  |       +----+      +------------------+        +---+
 *        | n  |       |ramp|      |  THE BLOCK       | east   |
 *  -15   | e  +---+   |    +------+  (16.5)          | street | 13 m
 *  -21   +----+ . +---+  THE STREET: the start, the lorry     +---+
 *  -24                |road| pavement, the dock, the room      |
 *  -37                +----+
 */

/** The map's name, its turn in the world (which side the sun is on), its walls' height, the starts. */
export const PLACE = {
  id: 'depot',
  title: 'Depot',
  turn: 2.2,
  wall: 9,
  // [east, north, facing in degrees from north toward east, tag]. The first is the player's.
  starts: [
    [-3, -17.5, 0, 'street'],
    [10, -19.5, 0, 'street'],
    [33.8, -21, 320, 'by the lorry'],
    [20, -8, 0, 'east street'],
    [-9, -8, 0, 'west of the block'],
    [-22, -44, 0, 'south road'],
    [-6, 22, 90, 'front yard'],
    [-34, 25, 90, 'beyond the raised road'],
    [39.8, 22, 0, 'alley'],
    [28.5, 41, 200, 'north dock'],
    [33, 38, 220, 'north dock'],
    [25.5, 33.5, 180, 'north dock'],
    [32.5, 45.5, 250, 'back walk'],
    [14.7, 49, 180, 'stair foot'],
  ],
};

// The surfaces the floors and solids name (look.js).
const TAR = 'dp_tarmac', PAVE = 'dp_pave', DECK = 'dp_deck', SHEET = 'dp_sheet', ROOFING = 'dp_roofing', STEEL = 'dp_steel';
const RUST = 'dp_can_rust', TEAL = 'dp_can_teal', OCHRE = 'dp_can_ochre', BLOCK = 'dp_block';
// The heights most floors lie at: the pavements, the docks at their west and east ends, the raised road,
// the hall's upper storey, its galleries, the roof a player walks, the roof of the hall's taller part.
const PV = 0.41, DK = 3.25, DE = 0.81, RD = 6.5, UP = 8.53, GA = 7.72, RF = 13, TOP = 15;
// The undersides the hall's two roofs give: what a lamp hangs from and a wall stops at.
const UNDER = TOP - 0.3;

/** A rectangle's corners from its west, south, east and north sides. */
const box = (w, s, e, n) => [[w, s], [e, s], [e, n], [w, n]];

// The roof's slopes: the bay over the south-east, down to the south; the east side, down to the alley.
const BAY = { from: [28, 14.2, 10.1], to: [28, 20, RF] };
const EAST = { from: [31.5, 25, RF], to: [36.9, 25, 10.1] };
const roof = (at, y) => ({ at, y, over: true, key: ROOFING, rail: 0.3, railKey: STEEL });

/**
 * The floors: polygons of [east, north], level (`y`) or sloped (`y: { from, to }`, each [east, north, up]).
 * The floors that stand over others come first, from the highest down: the walk grid takes the ground
 * under each in this order.
 */
export const GROUND = [
  // ------------------------------------------------------------------ the roof a player walks, over the lane and the south dock
  roof([[0.4, 16.5], [24.5, 16.5], [24.5, 20], [31.5, 20], [31.5, 30.95], [0.4, 30.95]], RF),
  // (its south edge, around the opening over the gallery)
  roof(box(0.4, 14.2, 21, 16.5), RF),
  roof(box(21, 14.2, 23.4, 15.2), RF),
  roof(box(23.4, 14.2, 24.5, 16.5), RF),
  // the bay that slopes to the south, around its two openings
  roof([[24.5, 16.5], [34.76, 16.5], [31.5, 20], [24.5, 20]], BAY),
  roof(box(24.5, 14.2, 25.2, 16.5), BAY),
  roof(box(25.2, 14.2, 27.4, 15.2), BAY),
  roof(box(27.4, 14.2, 27.9, 16.5), BAY),
  roof(box(27.9, 14.2, 30, 15.2), BAY),
  roof([[30, 14.2], [36.9, 14.2], [34.76, 16.5], [30, 16.5]], BAY),
  // the east side, down to the stair's head in the alley
  roof([[31.5, 20], [34.76, 16.5], [36.9, 14.2], [36.9, 30.95], [31.5, 30.95]], EAST),
  // ------------------------------------------------------------------ the roof of the hall's taller part: nobody walks it
  { at: [[0.6, 30.95], [23.5, 30.95], [23.5, 43.9], [30, 43.9], [30, 47.1], [13.1, 47.1], [13.1, 47.7], [0.6, 47.7], [0.6, 44.5], [-4.65, 44.5], [-4.65, 41.05], [-9.5, 41.05], [-9.5, 33.6], [0.6, 33.6]], y: TOP, over: true, key: ROOFING, rail: 0 },
  // ------------------------------------------------------------------ what hangs under the roofs: two girders, the ducts
  { at: box(3.4, 20.9, 35, 21.3), y: 12.4, over: true, thick: 2.8, rail: 0, key: STEEL, side: STEEL },
  { at: box(3.4, 28.3, 30.6, 28.7), y: 12.4, over: true, thick: 2.8, rail: 0, key: STEEL, side: STEEL },
  { at: box(4.7, 35.9, 17.8, 37.1), y: 13.2, over: true, thick: 0.9, rail: 0, key: SHEET, side: SHEET },
  { at: box(17.8, 30.95, 19.2, 43.7), y: 13.2, over: true, thick: 0.9, rail: 0, key: SHEET, side: SHEET },
  // ------------------------------------------------------------------ the raised road's deck over the yard beyond
  { at: box(-26.75, -2, -17, 31.2), y: RD, over: true, thick: 0.8, rail: 0.8, key: TAR },
  // ------------------------------------------------------------------ the hall's upper storey and its galleries
  { at: box(1.2, 31.2, 7.4, 43.9), y: UP, over: true, key: DECK, railKey: STEEL },
  { at: box(1.2, 43.9, 30, 47.1), y: UP, over: true, key: DECK, railKey: STEEL },
  { at: box(1.2, 30.6, 3, 31.2), y: UP, over: true, key: DECK, railKey: STEEL },
  { at: box(1.2, 29.2, 3, 30.6), y: { from: [2.1, 29.2, GA], to: [2.1, 30.6, UP] }, over: true, key: DECK, railKey: STEEL, steps: true },
  { at: box(1.2, 15, 3, 29.2), y: GA, over: true, key: DECK, railKey: STEEL },
  { at: box(3, 15, 30, 16.7), y: GA, over: true, key: DECK, railKey: STEEL },

  // ------------------------------------------------------------------ the hall: the lane through it, its two doors
  { at: box(1.2, 21.1, 36.1, 30.6), y: 0 },
  { at: box(0.4, 23.9, 1.2, 27.2), y: 0 },
  { at: box(36.1, 29.4, 36.9, 30.6), y: 0 },
  // the south dock: high at the west, a ramp, low at the east, and a short ramp of ours up from the lane
  { at: box(1.2, 15, 16.3, 21.1), y: DK, key: PAVE },
  { at: box(16.3, 15, 23.5, 21.1), y: { from: [16.3, 18, DK], to: [23.5, 18, DE] }, key: PAVE },
  { at: [[23.5, 15], [36.1, 15], [36.1, 19.5], [34.6, 19.5], [34.6, 21.1], [23.5, 21.1]], y: DE, key: PAVE },
  { at: box(34.6, 19.5, 36.1, 21.1), y: { from: [35.3, 21.1, 0], to: [35.3, 19.5, DE] }, key: PAVE },
  // the north dock, the same; its east end has no floor over it, so its roof is its own
  { at: box(1.2, 30.6, 16.3, 43.9), y: DK, key: PAVE },
  { at: box(16.3, 30.6, 23.5, 43.9), y: { from: [16.3, 37, DK], to: [23.5, 37, DE] }, key: PAVE },
  { at: [[23.5, 30.6], [27.2, 30.6], [27.2, 32.2], [30.2, 32.2], [30.2, 30.6], [36.1, 30.6], [36.1, 43.9], [23.5, 43.9]], y: DE, key: PAVE, roof: UNDER - DE, crown: false },
  { at: box(27.2, 30.6, 30.2, 32.2), y: { from: [28.7, 30.6, 0], to: [28.7, 32.2, DE] }, key: PAVE, roof: UNDER - DE, crown: false },
  // the back walk under the north gallery, the stair at its end up to the upper storey
  { at: box(1.2, 43.9, 30, 47.1), y: DK, key: PAVE },
  { at: box(30, 43.9, 35, 47.1), y: DK, key: PAVE, roof: UNDER - DK, crown: false },
  { at: box(13.1, 47.1, 16.3, 50.9), y: DK, key: PAVE, roof: UNDER - DK, crown: false },
  { at: box(16.3, 47.1, 26.5, 50.9), y: { from: [16.3, 49, DK], to: [26.5, 49, UP] }, key: PAVE, roof: UNDER - UP, crown: false, steps: true },
  { at: box(26.5, 47.1, 30, 50.9), y: UP, key: DECK, roof: UNDER - UP, crown: false },
  // the room behind the upper storey's door and window
  { at: [[-8.9, 34.2], [1.2, 34.2], [1.2, 43.9], [-4.05, 43.9], [-4.05, 40.45], [-8.9, 40.45]], y: UP, key: DECK },

  // ------------------------------------------------------------------ the front yard, the alley, the open ground south of the hall
  { at: box(-12.4, 14.2, 0.4, 30.3), y: 0 },
  { at: box(36.9, 14.2, 41.4, 30.85), y: 0 },
  { at: box(-12.4, 8.1, 41.4, 14.2), y: 0 },
  { at: box(-12.4, -1.66, 36.6, 8.1), y: 0 },
  { at: box(-12.4, -14.6, -5.6, -1.66), y: 0 },
  { at: box(-5.6, -9.8, -4.1, -6.5), y: 0, roof: 3 },
  { at: box(13.8, -4.9, 36.6, -1.66), y: 0 },
  { at: box(13.8, -14.6, 26.8, -4.9), y: 0 },
  // the stair of ours up the hall's east wall, where the place has its ladder to the roof
  { at: box(36.9, 19.3, 38.3, 28.5), y: { from: [37.6, 28.5, 0], to: [37.6, 19.3, 10.1] }, key: PAVE, steps: true, walls: false },
  { at: box(36.9, 17.9, 38.3, 19.3), y: 10.1, key: PAVE, walls: false },
  // ------------------------------------------------------------------ the street the player starts in
  { at: box(-26.75, -24.3, 36.6, -14.6), y: 0 },
  { at: box(36.6, -24.3, 39.8, -16.3), y: 0, roof: 3.6 },
  { at: [[-17.3, -27.7], [-17.3, -24.3], [16.5, -24.3], [16.5, -27.7], [-2, -27.7], [-2, -26.2], [-10.5, -26.2], [-10.5, -27.7]], y: PV, key: PAVE },
  { at: box(-10.5, -27.7, -2, -26.2), y: 0.81, key: PAVE },
  { at: box(-9.3, -31, -2.8, -27.7), y: 1.22, key: PAVE, roof: 3.4 },
  { at: box(16.5, -27.7, 24, -24.3), y: 0 },
  { at: box(17, -30.9, 23.5, -27.7), y: 0, key: PAVE, roof: 3 },
  { at: box(24, -27.7, 36.6, -24.3), y: PV, key: PAVE },
  // the road south of it, under the wall that crosses over it
  { at: box(-26.75, -54.5, -17.3, -24.3), y: 0 },
  // ------------------------------------------------------------------ the raised road: the ramp, the road north of the yard
  { at: box(-26.75, -14.6, -21, -9.5), y: 0 },
  { at: box(-21, -14.6, -17, -9.5), y: { from: [-19, -14.6, 0], to: [-19, -9.5, 2.75] } },
  { at: box(-26.75, -9.5, -17, -2.6), y: { from: [-22, -9.5, 2.75], to: [-22, -2.6, RD] } },
  { at: box(-26.75, -2.6, -17, -2), y: RD },
  { at: box(-26.75, 31.2, -17, 53.9), y: RD },
  // the strip between the raised road and the yard (a solid stands on it), and the way under the road
  { at: box(-17, -14.6, -12.4, 20.8), y: 0 },
  { at: box(-26.75, 5.7, -17, 13.8), y: 0 },
  // ------------------------------------------------------------------ beyond the raised road
  { at: [[-48.7, -21], [-40.4, -21], [-40.4, 3], [-42, 3], [-42, 21], [-26.75, 21], [-26.75, 22], [-17, 22], [-17, 20.8], [-12.4, 20.8], [-12.4, 30.3], [-17, 30.3], [-17, 30.8], [-40.4, 30.8], [-40.4, 47], [-48.7, 47]], y: 0 },
  { at: box(-42, 3, -40, 21), y: PV, key: PAVE },
  { at: box(-40, 18, -26.75, 21), y: PV, key: PAVE },
  { at: box(-40, 11.5, -29.9, 18), y: 0 },
  { at: box(-29.9, -54.5, -26.75, 18), y: PV, key: PAVE },
  { at: box(-39.5, -21, -29.9, -18), y: PV, key: PAVE },
];

/** Walls along a line, with what is cut in them: `holes` of `{ at, w, h, sill }`, `at` in metres from the first point. */
export const WALLS = [
  // the hall's big door in its west wall, and the side door on the alley
  { line: [[0.8, 23.6], [0.8, 27.5]], y: 0, h: RF - 0.3, t: 0.8, crown: false, holes: [{ at: 1.95, w: 3.3, h: 4.2 }] },
  { line: [[36.5, 29.1], [36.5, 30.9]], y: 0, h: 9.8, t: 0.8, crown: false, holes: [{ at: 0.9, w: 1.2, h: 2.2 }] },
  // the wall of the hall's taller part, over the roof a player walks
  { line: [[0.4, 30.75], [31.5, 30.75]], y: RF - 0.3, h: TOP - RF + 0.3, t: 0.4, low: 0, crown: false },
  { line: [[31.5, 30.75], [36.9, 30.75]], y: 9.8, h: TOP - 9.8, t: 0.4, low: 0, crown: false },
  // the side of the roof's bay
  { line: [[24.5, 14.2], [24.5, 20]], y: 10.1, h: RF - 10.1, t: 0.3, low: 0, crown: false },
  // the room's wall on the upper storey: its window and its door
  { line: [[1, 34.2], [1, 43.9]], y: DK, h: UNDER - DK, t: 0.4, crown: false, holes: [{ at: 1.6, w: 2.6, h: 1.6, sill: UP - DK + 1, glass: true, frame: true }, { at: 4.9, w: 1.7, h: 2.3, sill: UP - DK }] },
  // the wall between the back walk and the stair
  { line: [[16.3, 47.1], [26.5, 47.1]], y: DK, h: UNDER - DK, t: 0.3, crown: false },
  { line: [[26.5, 47.1], [30, 47.1]], y: DK, h: UP - 0.3 - DK, t: 0.3, crown: false },
  // the yard's fence and its gate, standing ajar
  { line: [[-12.4, 20.8], [-12.4, 25.7]], h: 3.7, t: 0.3, key: SHEET, crown: false },
  { line: [[-12.3, 30.1], [-9.4, 25.9]], h: 3, t: 0.12, key: SHEET, crown: false },
  // the wall that crosses over the south road
  { line: [[-30.5, -37.5], [-16.7, -37.5]], y: 4.2, h: 6.3, t: 0.6, low: 0 },
  // the ramp's side walls
  { line: [[-17.1, -14.6], [-17.1, -9.5]], y: 0, h: 3.8, t: 0.3 },
  { line: [[-17.1, -9.5], [-17.1, -2]], y: 0, h: 7.5, t: 0.3 },
  { line: [[-26.65, -9.5], [-26.65, -2]], y: 0, h: 7.5, t: 0.3 },
  // the room off the street
  { line: [[17, -27.7], [23.5, -27.7]], h: 3, t: 0.4, crown: false, holes: [{ at: 2.4, w: 1.7, h: 2.2 }] },
];

/** Doors: `facing` is the way a doorway looks, in degrees from north toward east. */
export const DOORS = [
  { at: [0.8, 25.55], y: 0, facing: 270, w: 3.3, h: 4.2, kind: 'roller', raised: 0.62, key: SHEET },
  { at: [36.5, 30], y: 0, facing: 90, w: 1.2, h: 2.2, leaves: 1, hinge: 1, key: STEEL },
  { at: [1, 39.1], y: UP, facing: 90, w: 1.7, h: 2.3, leaves: 1, key: STEEL },
  { at: [19.4, -27.7], facing: 0, w: 1.7, h: 2.2, leaves: 1, key: STEEL },
];

/** A stack of freight boxes on a footprint, from `y`: one solid a box, each `high`, in the colours given. */
const stack = (at, y, high, ...keys) => keys.map((key, i) => ({ at, y: y + i * high, top: y + (i + 1) * high, key }));

/** Solids of any footprint, from `y` to `top`. */
export const MASSES = [
  // the strip between the raised road and the yard: a shed of sheet where the place has a fenced strip
  { at: box(-16.9, -14.5, -12.5, 20.7), top: 3.75, key: SHEET },
  // the freight boxes in the yard: three high by the hall's corner, two high south of it
  ...stack(box(-5.6, 11.4, 0.3, 14.1), 0, 3.18, RUST, TEAL, RUST),
  ...stack(box(-10.5, 3.25, -4, 6.45), 0, 3.25, TEAL, OCHRE),
  // in the hall: three high at the lane's east end, two on the dock beside them, one on the south dock
  ...stack(box(30.8, 24.4, 34.1, 30.5), 0, 3.12, OCHRE, RUST, TEAL),
  ...stack(box(32.1, 30.7, 34.5, 33.4), DE, 3.25, TEAL, RUST),
  ...stack(box(3.6, 16.9, 10, 20), DK, 3.25, RUST),
  // the duct's trunk down to the upper storey, and its hood on the roof
  { at: box(3.3, 35.3, 4.7, 38), y: UP, top: 12.3, key: SHEET },
  { at: box(17.8, 29.5, 19.2, 30.55), y: RF, top: 13.9, key: SHEET },
  // the posts under the north gallery
  { at: box(9, 43.9, 9.6, 44.5), y: DK, top: UP - 0.3, key: STEEL },
  { at: box(14.6, 43.9, 15.2, 44.5), y: DK, top: UP - 0.3, key: STEEL },
  { at: box(20.3, 43.9, 20.9, 44.5), y: DK, top: UP - 0.3, key: STEEL },
  { at: box(26, 43.9, 26.6, 44.5), y: DK, top: UP - 0.3, key: STEEL },
  // the lorry in the street, a wreck of ours: its bed, its box, its cab
  { at: box(24.6, -19.3, 31.8, -16), y: 0.55, top: 1, key: STEEL },
  { at: box(26.9, -19.5, 31.8, -15.8), y: 1, top: 3, key: SHEET },
  { at: box(24.5, -19.2, 26.5, -16.1), y: 1, top: 2.25, key: OCHRE },
  { at: box(25.1, -19.4, 26, -18.9), top: 0.9, key: BLOCK },
  { at: box(25.1, -16.4, 26, -15.9), top: 0.9, key: BLOCK },
  { at: box(29.6, -19.4, 31.2, -18.9), top: 0.9, key: BLOCK },
  { at: box(29.6, -16.4, 31.2, -15.9), top: 0.9, key: BLOCK },
];

/** Crates: [east, north, side, turn in degrees, up]. Without `up` a crate stands on the highest floor there. */
export const CRATES = [
  // on the pavement by the ramp, by the east street's wall
  [-28.3, -11.8, 1.6, 0], [-28.6, -10.3, 1.1, 12], [26, -8.9, 1.2, 0],
  // the north dock: the tall one and the low one beside it; the two under the upper storey
  [27.2, 37.8, 2.4, 0, DE], [27.8, 35.9, 1.2, 8, DE], [5.6, 39.8, 1.65, 0, DK], [4, 36.6, 1.65, 6, DK],
  // the south dock's corner: stacked
  [35.2, 15.9, 1.6, 0, DE], [35.2, 15.9, 1.5, 10, DE + 1.6], [35.2, 15.9, 1.4, 4, DE + 3.1], [34, 17.3, 1.6, 0, DE], [34, 17.3, 1.6, 8, DE + 1.6], [32.4, 17.8, 1.7, 0, DE],
];

/** The tops of the blocks beside the floors, where they are not `PLACE.wall` over the floor. A wall rises to its block. */
export const BLOCKS = [
  // the hall's taller part and the wing west of it; the blocks beside the raised road north of the yard
  { at: box(-17, 30.3, 42, 56), top: TOP },
  { at: box(-40, 30.8, -26.75, 56), top: 12 },
  // the block the street runs round, the blocks east of the east street
  { at: box(-5.6, -14.6, 13.8, -1.66), top: 16.5 },
  { at: box(26.8, -14.6, 36.6, -4.9), top: 13 },
  { at: box(36.6, -16.3, 50, 8.1), top: 20 },
  { at: box(36.6, -36, 50, -16.3), top: 16.3 },
  { at: box(41.4, 8.1, 50, 31), top: 14 },
  // south of the street, west of the far lane, the block beyond the raised road
  { at: box(-17.3, -62, 36.6, -27.7), top: 11 },
  { at: box(-58, -30, -48.7, 56), top: 5 },
  { at: box(-40, -18, -30.3, 11.5), top: 13 },
  { at: box(-40, -62, -30.3, -21), top: 10 },
];

/** A mark of ours on the room's floor. */
export const MARKS = [{ at: [-2, 39.5], y: UP, size: 3 }];

/** Lamps: [east, north, up], `up` what a lamp hangs from. */
export const LAMPS = [
  // under the two girders over the lane
  [7, 21.1, 9.6], [15, 21.1, 9.6], [23, 21.1, 9.6], [31, 21.1, 9.6], [7, 28.5, 9.6], [15, 28.5, 9.6], [23, 28.5, 9.6], [29.5, 28.5, 9.6],
  // under the galleries: over the big door, at the south dock's east end
  [2.1, 25.5, GA - 0.3], [29, 15.9, GA - 0.3],
  // the north dock: under the ducts, on their rods at its east end, by the side door's ramp
  [11, 36.5, 12.3], [18.5, 33.5, 12.3], [18.5, 41, 12.3], [25.5, 35.5, 10.6], [30, 40.5, 10.6], [35.2, 30, 6.5],
  // under the upper storey and the north gallery, and the back walk's east end
  [4.3, 40.5, UP - 0.3], [9, 45.5, UP - 0.3], [20, 45.5, UP - 0.3], [32.5, 45.5, 9.5],
  // the upper storey, the room, the stair: under the taller roof
  [4.3, 37, UNDER], [5, 45.5, UNDER], [16, 45.5, UNDER], [27, 45.5, UNDER], [-2, 37, UNDER], [-2.5, 42, UNDER], [-7, 37.5, UNDER], [14.7, 49, 11.8], [21.5, 49, UNDER], [28.2, 49, UNDER],
  // outside: the alcove off the street, the room off it, the dock on the pavement
  [38.2, -20, 3.5], [20.2, -29.5, 2.9], [-5.7, -29.5, 4.5],
];
