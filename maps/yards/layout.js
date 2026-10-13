/**
 * TWIN YARDS — a map of Pocket Duty.
 *
 * Two walled yards on the north side, a court between them behind a gate, and three ways up to
 * them from the south: the street through the middle, a long open lane down the east side, and
 * a covered way down the west side with a branch that comes out on the street. A short way
 * climbs from the street to the east yard.
 *
 * The plan is written by hand in the terms of the reference's layout (web/src/world/layout.js):
 * a street, alleys, buildings, a gate and set pieces, on one level. `MAP`, `GROUND`, `WALLS`,
 * `COVERED`, `RAISED`, `RAMPS` and `COVER` are what a map of ours states besides
 * (maps/README.md). Coordinates are metres in level space: +x east, +z south.
 * Sides of a building: 0 = -Z, 1 = +X, 2 = +Z, 3 = -X.
 *
 *      -57        -46        -24      -6.5 6.5    16  22 28        46        57
 *  -57  +----------+----------+---------+---+-----+---+--+---------+---------+
 *       |      WEST YARD      |  back   | s |     |      EAST YARD           |
 *       |                     +---------+ t +-----+   (a terrace, two ramps) |
 *       |                     d  NORTH  | r |     r                          |
 *  -27  |  mouth  +-----------+  COURT  +gate+----+--+   +-------+           |
 *       |         | WA1 | WA2 |         | e |  EA1   | s |       |           |
 *       | COVERED +-----+-----+         | e +--------+ h |  EB1  |   LONG    |
 *       |   WAY   | WB1 | WB2 |         | t   short  o   |       |   LANE    |
 *    2  |         +-----+-----+           |  +----------+--+-------+         |
 *       |         =  lower way  =         |  |          |  |       |         |
 *    8  |         +-----+-----+           |  |   EC1    |  |  EB2  |         |
 *       |         | WC1 | WC2 |           |  +----------+  +-------+         |
 *   30  |  mouth  +-----+-----+           |  |   EC2    |  |  EB3  |         |
 *   36  +---------+                       |  +----------+--+-------+--doors--+
 *       |        SOUTH SIDE: the street runs on to the wall; SW1 and SE1 stand in it |
 *   57  +-----------------------------------------------------------------------------+
 */

/** The street through the middle, from the south wall to the back of the north court. */
export const STREET = {
  halfWidth: 4.5,
  kerb: 6.5,
  walkH: 0.145,
  zMin: -50,
  zMax: 56,
};

/**
 * The reference's alleys, as rects [x0, z0, x1, z1]: open ground with the junk the reference leaves in
 * an alley (crates, drums, tyres, rubble, about one thing a square metre). Only the two narrow ways.
 */
export const ALLEYS = [
  // the covered way's branch to the street
  { rect: [-46, 2, -6.5, 8], surface: 'dirt' },
  // the short way from the street up to the east yard
  { rect: [6.5, -12, 28, -6], surface: 'gravel' },
  { rect: [22, -26.5, 28, -12], surface: 'gravel' },
];

export const BUILDINGS = [
  // ------------------------------------------------ west of the street --
  { id: 'WA1', x: -36, z: -19.25, w: 20, d: 14.5, floors: 2, wallKey: 'plaster_sand', streetSide: 0, damage: 0.2, balconies: 0.3, roofProps: 3, skipSides: [1] },
  { id: 'WA2', x: -16.25, z: -19.25, w: 19.5, d: 14.5, floors: 3, setback: { from: 2, depth: 2.4, side: 1 }, wallKey: 'plaster_cream', trimKey: 'concrete', streetSide: 1, secondarySide: 0, damage: 0.25, balconies: 0.5, arches: true, roofProps: 3, skipSides: [3] },
  { id: 'WB1', x: -36, z: -5, w: 20, d: 14, floors: 2, wallKey: 'plaster_pink', streetSide: 2, shops: false, damage: 0.3, balconies: 0.2, roofProps: 2, skipSides: [1] },
  { id: 'WB2', x: -16.25, z: -5, w: 19.5, d: 14, floors: 2, wallKey: 'plaster_blue', streetSide: 1, secondarySide: 2, damage: 0.2, balconies: 0.45, roofProps: 3, skipSides: [3] },
  { id: 'WC1', x: -36, z: 19, w: 20, d: 22, floors: 3, wallKey: 'plaster_cream', streetSide: 2, shops: false, damage: 0.2, balconies: 0.3, roofProps: 4, skipSides: [1] },
  { id: 'WC2', x: -16.25, z: 19, w: 19.5, d: 22, floors: 2, setback: { from: 1, depth: 2.2, side: 1 }, wallKey: 'plaster_sand', streetSide: 1, secondarySide: 2, damage: 0.3, balconies: 0.55, doorBays: { 1: 2 }, roofProps: 3, skipSides: [3] },
  // ------------------------------------------------ east of the street --
  { id: 'EA1', x: 14.25, z: -19.25, w: 15.5, d: 14.5, floors: 2, wallKey: 'plaster_pink', streetSide: 3, secondarySide: 2, damage: 0.35, balconies: 0.4, roofProps: 2 },
  { id: 'EB1', x: 37, z: -16.25, w: 18, d: 20.5, floors: 3, wallKey: 'plaster_sand', streetSide: 0, secondarySide: 1, damage: 0.25, balconies: 0.35, roofProps: 3 },
  { id: 'EB2', x: 37, z: 4, w: 18, d: 20, floors: 2, wallKey: 'plaster_blue', streetSide: 1, damage: 0.45, ruin: true, collapse: true, balconies: 0.2, roofProps: 1, skipSides: [3] },
  { id: 'EB3', x: 37, z: 25, w: 18, d: 22, floors: 2, wallKey: 'plaster_cream', streetSide: 1, secondarySide: 2, damage: 0.2, balconies: 0.3, roofProps: 3, skipSides: [3] },
  { id: 'EC1', x: 17.25, z: 4, w: 21.5, d: 20, floors: 3, setback: { from: 2, depth: 2.6, side: 3 }, wallKey: 'plaster_cream', trimKey: 'concrete', streetSide: 3, secondarySide: 0, damage: 0.2, balconies: 0.5, arches: true, doorBays: { 3: 3 }, roofProps: 4, skipSides: [1] },
  { id: 'EC2', x: 17.25, z: 25, w: 21.5, d: 22, floors: 2, wallKey: 'plaster_sand', streetSide: 3, secondarySide: 2, damage: 0.3, balconies: 0.4, roofProps: 3, skipSides: [1] },
  // ------------------------------------------------ north and south --
  { id: 'NB1', x: -4, z: -53.5, w: 40, d: 7, floors: 3, wallKey: 'plaster_blue', streetSide: 2, damage: 0.2, balconies: 0.3, roofProps: 3, skipSides: [0] },
  { id: 'SW1', x: -31, z: 44, w: 18, d: 12, floors: 2, wallKey: 'plaster_pink', streetSide: 1, secondarySide: 0, damage: 0.25, balconies: 0.3, roofProps: 2 },
  { id: 'SE1', x: 25, z: 48, w: 18, d: 12, floors: 2, wallKey: 'plaster_blue', streetSide: 3, secondarySide: 0, damage: 0.2, balconies: 0.3, roofProps: 2 },
  // ------------------------------------------------ beyond the wall --
  // Only what stands above the perimeter wall shows: the town goes on.
  { id: 'BN1', x: -38, z: -66, w: 24, d: 12, floors: 3, wallKey: 'plaster_sand', streetSide: 2, damage: 0.15, skipSides: [0], roofProps: 2 },
  { id: 'BN2', x: 36, z: -66, w: 26, d: 12, floors: 4, wallKey: 'plaster_cream', streetSide: 2, damage: 0.15, skipSides: [0], roofProps: 2 },
  { id: 'BE1', x: 66, z: -8, w: 12, d: 26, floors: 3, wallKey: 'plaster_pink', streetSide: 3, damage: 0.15, skipSides: [1], roofProps: 2 },
  { id: 'BW1', x: -66, z: -40, w: 12, d: 22, floors: 3, wallKey: 'plaster_cream', streetSide: 1, damage: 0.15, skipSides: [3], roofProps: 2 },
  { id: 'BS1', x: -8, z: 66, w: 30, d: 12, floors: 3, wallKey: 'plaster_sand', streetSide: 0, damage: 0.15, skipSides: [2], roofProps: 2 },
];

/** The gate between the street and the north court: the reference's, moved. It faces south. */
export const GATE = {
  z: -28.1,
  depth: 3.2,
  span: 5.6,
  height: 4.9,
  outerW: 17,
  bodyH: 6.7,
  xL0: -8.6,
  xL1: -2.8,
  hL: 7.9,
  xR0: 2.8,
  xR1: 6.1,
  hR: 9.5,
  eastProud: 0.55,
  xT0: 6.1,
  xT1: 9.4,
  hT: 12.4,
  towerProud: 1.5,
};

/** Set pieces, as the reference's dressing reads them. */
export const SET_PIECES = {
  /** Market stalls: [x, z, ry, width] */
  stalls: [
    [-3.2, 40.5, 0.08, 2.4],
    [3.3, 44.0, 3.1, 2.4],
    [-10.5, 47.5, 1.5, 2.3],
    [3.1, 12.5, 3.2, 2.4],
    [-3.2, 20.0, 0.1, 2.2],
    [-14.0, -38.0, 1.6, 2.4],
    [10.5, -44.0, 4.7, 2.3],
    [-3.1, -8.0, 0.05, 2.3],
  ],
  /** Jersey barriers: [x, z, ry] */
  jerseys: [
    [-2.4, 30.5, 0.1],
    [2.6, 26.0, -0.1],
    [-0.6, 15.0, 1.5],
    [2.8, 2.0, 0.1],
    [-2.6, -3.0, 0.05],
    [0.4, -16.0, 1.55],
    [-2.8, -22.0, 0.1],
    [51.0, 22.0, 1.5],
    [53.0, 6.0, 0.2],
    [49.5, -10.0, 1.6],
    [-51.5, 31.5, 1.55],
    [-36.0, -31.0, 0.1],
    [30.0, -30.0, 1.5],
  ],
  /** Sandbag emplacements: [x, z, ry, length] */
  sandbagWalls: [
    [-3.6, 8.5, 0.0, 3.0],
    [3.6, -11.5, 0.0, 2.6],
    [-12.0, -36.5, 1.57, 2.6],
    [51.5, -22.0, 1.57, 3.2],
    [-42.0, -40.0, 0.0, 3.0],
    [20.0, -40.0, 0.0, 2.8],
    [-24.0, 36.0, 1.57, 2.8],
  ],
  /** Burnt-out vehicles: [x, z, ry, rollDeg] */
  wrecks: [
    [2.4, 21.0, 0.4, 0],
    [52.0, 14.0, 1.4, 4],
    [-2.6, -40.0, -2.6, 0],
    [40.0, 41.0, 1.2, 0],
  ],
  /** Palm trees: [x, z, scale] */
  palms: [
    [-5.5, 34.0, 1.0],
    [5.5, 38.5, 1.1],
    [-5.5, 10.5, 0.92],
    [5.6, -3.0, 1.05],
    [-5.5, -24.0, 1.0],
    [-20.0, -46.0, 0.9],
    [12.0, -32.0, 0.95],
    [-44.0, 52.0, 1.0],
    [48.0, 52.0, 0.9],
    [-30.0, -52.0, 1.0],
    [52.0, -52.0, 0.95],
  ],
  /** Street lamps: [x, z, ry] — ry points the arm across the street. */
  lamps: [
    [-5.9, 28.0, -Math.PI / 2],
    [5.9, 16.0, Math.PI / 2],
    [-5.9, 0.0, -Math.PI / 2],
    [5.9, -18.0, Math.PI / 2],
    [-5.9, -42.0, -Math.PI / 2],
    [5.9, 46.0, Math.PI / 2],
  ],
  /** Overhead cable spans: [x0, y0, z0, x1, y1, z1, sag] */
  cables: [
    [-6.4, 7.2, 24.0, 6.4, 6.6, 26.5, 1.1],
    [-6.4, 6.4, 12.0, 6.4, 6.9, 13.5, 1.3],
    [-6.4, 6.2, -9.0, 6.4, 6.6, -7.5, 1.0],
    [-6.4, 7.4, -21.0, 6.4, 7.0, -19.0, 1.2],
  ],
  /** Laundry lines with hanging cloth: [x0, y0, z0, x1, y1, z1] */
  laundry: [
    [6.35, 3.6, 16.0, 6.35, 3.75, 21.2],
    [-6.35, 3.7, 11.0, -6.35, 3.6, 15.4],
    [-6.35, 6.6, -18.5, -6.35, 6.4, -13.5],
    [6.35, 6.5, 3.0, 6.35, 6.7, 8.0],
  ],
  /** Hanging rugs / cloth on facades: [x, y, z, ry, w, h] */
  hangings: [
    [-6.45, 2.6, 18.5, Math.PI / 2, 1.5, 2.1],
    [-6.45, 2.4, -6.5, Math.PI / 2, 1.2, 1.7],
    [6.45, 2.7, 22.0, -Math.PI / 2, 1.6, 2.2],
    [6.45, 2.5, 0.5, -Math.PI / 2, 1.3, 1.9],
  ],
  /** Rubble piles: [x, z, radius, count] */
  rubble: [
    [48.5, 2.0, 2.6, 36],
    [-4.2, -14.5, 2.2, 28],
    [24.5, -22.0, 1.8, 22],
    [-50.0, -30.0, 2.0, 24],
    [34.0, 39.5, 1.8, 20],
  ],
  /** Tyre stacks: [x, z, n] */
  tyres: [
    [-5.2, 24.5, 4],
    [5.3, -8.0, 3],
    [47.5, 30.0, 4],
    [-47.5, 27.5, 3],
    [-26.0, -30.0, 4],
  ],
};

// ------------------------------------------------------------------ what a map of ours states besides

/** The map's name, the level's place in the world, and the spawn points: [x, z, yaw, tag]. The first is the player's start. */
export const MAP = {
  id: 'yards',
  title: 'Twin Yards',
  level: { yaw: 0.5877, tx: 0, tz: 0 },
  // (a yaw of 0 looks north, toward -z; the south starts look up the map, the north starts down it)
  spawns: [
    [0.4, 44.0, 0, 'south side'],
    [-38.0, 34.0, 0, 'covered way, south mouth'],
    [42.0, 45.0, 0, 'long lane, doors'],
    [-51.5, 0.0, 0, 'covered way'],
    [-28.0, 5.0, -Math.PI / 2, 'lower way'],
    [51.5, -4.0, Math.PI, 'long lane'],
    [25.0, -18.0, Math.PI, 'short way'],
    [3.0, -37.0, Math.PI, 'north court'],
    [-40.0, -44.0, Math.PI, 'west yard'],
    [44.0, -47.0, Math.PI, 'east yard'],
    [2.0, 6.0, 0, 'street'],
  ],
};

/**
 * Open ground with no junk on it, as rects [x0, z0, x1, z1] with the kit's surface: the yards, the court,
 * the long lane, the covered way and the south side. The sand's swell lies over it in drifts.
 */
export const GROUND = [
  { rect: [-57, -57, -24, -26.5], surface: 'gravel' }, // west yard
  { rect: [-24, -50, -6.5, -26.5], surface: 'dirt' }, // north court, west of the street
  { rect: [6.5, -50, 16, -26.5], surface: 'dirt' }, // north court, east of the street
  { rect: [16, -57, 57, -26.5], surface: 'dirt' }, // east yard
  { rect: [-57, -26.5, -46, 30], surface: 'dirt' }, // covered way
  { rect: [46, -26.5, 57, 36], surface: 'gravel' }, // long lane
  { rect: [-57, 30, -6.5, 57], surface: 'dirt' }, // south side, west of the street
  { rect: [6.5, 36, 57, 57], surface: 'dirt' }, // south side, east of the street
];

/** Free walls: from `a` to `b`, `h` high (3.4), standing at `y` (the ground there). A lintel is a wall that starts above a doorway. */
export const WALLS = [
  // the west yard's wall to the court, with its doorway
  { a: [-24, -57], b: [-24, -44.5], h: 4.2 },
  { a: [-24, -41], b: [-24, -26.5], h: 4.2 },
  { a: [-24, -44.5], b: [-24, -41], y: 2.7, h: 1.5 },
  // the east yard's wall to the court, with the wide way up to the terrace
  { a: [16, -57], b: [16, -47], h: 4.2 },
  { a: [16, -38], b: [16, -26.5], h: 4.2 },
  // the doors of the long lane
  { a: [46, 36], b: [49.6, 36], h: 4.6, key: 'plaster_cream' },
  { a: [53.4, 36], b: [57.6, 36], h: 4.6, key: 'plaster_cream' },
  { a: [49.6, 36], b: [53.4, 36], y: 3.0, h: 1.6, key: 'plaster_cream' },
  // the covered way's outer wall, under its roof and against the perimeter wall
  { a: [-57.45, -22], b: [-57.45, 26], h: 3.3, t: 0.7, coping: false, key: 'plaster_cream' },
];

/** Ways under a roof: `rect`, the soffit's height `y` (3.3), metres of block over it (`mass`, 3), bulbs at [x, z]. */
export const COVERED = [
  { rect: [-57.8, -22, -46, 26], y: 3.3, mass: 3.4, key: 'plaster_sand', bulbs: [[-51.5, -14], [-51.5, 2], [-51.5, 18]] },
  { rect: [-46, 2, -12, 8], y: 3.3, mass: 3.0, key: 'plaster_pink', bulbs: [[-36, 5], [-20, 5]] },
];

/** Ground above the rest: `rect`, its height `h`, the kit's material `key`, and a course of another surface on top (`top`). */
export const RAISED = [{ rect: [28, -55, 55, -36], h: 0.8, key: 'concrete', top: 'dirt' }];

/** Ramps: `rect`, the side the ramp rises toward (`up`: 'x+', 'x-', 'z+', 'z-'), the height it reaches (`h`). */
export const RAMPS = [
  { rect: [21, -50, 28, -42], up: 'x+', h: 0.8, key: 'concrete' },
  { rect: [44, -36, 53, -30], up: 'z-', h: 0.8, key: 'concrete' },
];

/** Cover: ['crate', x, z, ry, side], ['stack', x, z, n], ['block', x, z, ry, scale], ['drums', x, z, n]. */
export const COVER = [
  // the east yard's terrace
  ['crate', 36.0, -46.0, 0.2, 1.9],
  ['crate', 38.2, -45.4, 0.25, 1.4],
  ['crate', 46.0, -41.0, 1.3, 1.8],
  ['stack', 33.0, -40.0, 3],
  ['drums', 50.5, -50.0, 3],
  ['block', 41.0, -51.5, 0.1, 1.3],
  // the west yard
  ['crate', -40.0, -40.0, 0.1, 1.9],
  ['crate', -42.3, -39.4, 0.0, 1.5],
  ['crate', -32.0, -50.0, 1.4, 1.8],
  ['crate', -50.0, -47.0, 0.4, 1.7],
  ['stack', -30.5, -34.0, 3],
  ['drums', -52.0, -36.0, 3],
  ['block', -45.0, -53.0, 1.5, 1.3],
  // the north court
  ['crate', -14.0, -46.0, 0.3, 1.6],
  ['stack', 11.0, -36.0, 3],
  // the long lane
  ['crate', 54.0, -2.0, 0.1, 1.8],
  ['stack', 48.5, 12.0, 3],
  ['drums', 54.5, 27.0, 3],
  ['block', 49.0, -18.0, 0.3, 1.2],
  // the covered way and its branch
  ['stack', -55.0, 10.0, 3],
  ['crate', -48.0, -10.0, 0.2, 1.5],
  ['stack', -30.0, 3.4, 2],
  // the short way
  ['stack', 16.0, -7.4, 3],
  ['drums', 26.5, -20.0, 2],
  // the south side
  ['crate', -20.0, 52.0, 0.5, 1.7],
  ['crate', 14.0, 40.0, 0.2, 1.6],
  ['stack', -46.0, 44.0, 3],
  ['block', 44.0, 50.0, 1.2, 1.3],
];
