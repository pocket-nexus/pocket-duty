/**
 * CAUSEWAY, a place of Pocket Duty.
 *
 * Drawn by hand, floor by floor, in the place's own coordinates: metres east and north of its middle,
 * heights in metres above its lowest floor (maps/README.md). North is up in the plan below.
 *
 *      -88   -75    -58  -52     -36      -19   -3  +4     +16        +36    +49  +58      +75   +88
 *  +49                                                                  far pool (0) +----------+
 *  +34                                                                  +---+ arm    | landing  |
 *  +31        +--------+--------+                                       |   | (0.6)  +--+----+--+
 *             | north  | WEST   |      +---------+--THE HALL--+------+  |   |   ramp |  | ramp|
 *  +21        | part   | SITE   +------+ west    |  rope      | east |  +---+--------+  +--+--+
 *             | (5.5)  | (8.1)   pass. | room    |  bridge    | room |  water court B (0.8)|  | corridor
 *  +13  +-----+        |        +--+---+ (8.9)   |  over the  | (8.9)+--+   channel (0)    |  | (6.5)
 *       |annex+--------+---++---+  |   +--+      |  pool (0)  | link |  water court A      +--+-----+
 *   +4  |(5.1)| WEST       ||cor.| |gal-  |      |            +--+---+ stair  +------+ +---+ EAST  +---+
 *       +-----+ START      |+----+-+lery  +------+            |  EAST COURT (8.1)    | |   | START |   |
 *   -1        | (5.5)      || MID-WEST COURT (8.1)            +--+--+----------------+ |rm.| (6.5) |east
 *             |            ||          +--THE CROSSING (8.1) over the river--+ link    |   |       |room
 *  -12        +--stair-----++---+------+--stair to the river--+--------------+---+     +---+ramp---+---+
 *             | SOUTH-WEST      |      |  bank | river (0)| terraces | SOUTH-|
 *  -24        | COURT (8.1) +---+      |       |          | 3.7 4.9  | EAST  |
 *             |             | tunnel: two rooms +---------+ 7.7      | COURT |
 *  -36        +-------------+----------+ THE PLATFORM (8.1) over it  | (8.1) |
 *  -44                                 +-----------------------------+-------+
 *  -49                                        far pool (0)
 */

/** The map's name, its turn in the world (which side the sun is on), its walls' height, the starts. */
export const PLACE = {
  id: 'causeway',
  title: 'Causeway',
  turn: 0.35,
  wall: 7.2,
  // [east, north, facing in degrees from north toward east, tag]. The first is the player's.
  starts: [
    [-13, 16.5, 90, 'bridge'],
    [-66, 4, 90, 'west start'],
    [-70, -3, 80, 'west start'],
    [-63, 9, 100, 'west start'],
    [-62, 24, 90, 'west start, north'],
    [-66, -24, 90, 'south-west court'],
    [-44, 22, 180, 'west site'],
    [-40, -8, 90, 'mid-west court'],
    [66, 2, 270, 'east start'],
    [70, -6, 280, 'east start'],
    [63, 7, 260, 'east start'],
    [69, 16, 0, 'east start, north'],
    [45, 0, 270, 'east room'],
    [24, 2, 270, 'east court'],
    [11, -30, 0, 'south-east court'],
    [-4, -40, 0, 'platform'],
  ],
};

// The heights most floors lie at: the common level, the raised walk, the two rooms by the bridge, the west
// start and its annex, the east start; a wall's top over the courts, the top of the mass they are cut in.
const M = 8.13, H = 8.53, GA = 8.94, WS = 5.5, AN = 5.08, ES = 6.5, TOP = 15.33, CAP = 18.69;
// The river: its banks, the shelves beside its bed (the bed is 0), the water's level.
const BANK = 0.81, SH = 0.42, WL = 0.65;
// A roofed way's clear height, and the walls that carry the mass over it.
const CLEAR = 4.1;

/** A rectangle's corners from its west, south, east and north sides. */
const box = (w, s, e, n) => [[w, s], [e, s], [e, n], [w, n]];
/** A roofed way at the common level or another: the mass stands over it to its top. */
const way = (at, y = M) => ({ at, y, roof: CLEAR, walls: CAP - (typeof y === 'number' ? y : Math.max(y.from[2], y.to[2])), crown: false });
/** A stretch of the river's bed or its banks in the open: the courts' walls stand over it. */
const bed = (at, y) => ({ at, y, walls: TOP - y });

/**
 * The floors: polygons of [east, north], level (`y`) or sloped (`y: { from, to }`, each [east, north, up]).
 * The floors that stand over others come first, from the highest down.
 */
export const GROUND = [
  // ------------------------------------------------------------------ the hall's roof around its opening: nobody walks it
  { at: [[-18.9, -8.2], [7.6, -8.2], [7.6, -2.4], [3.9, -2.4], [3.9, 2.5], [-18.9, 2.5]], y: CAP, over: true, thick: 1, rail: 0, walls: 0, soldiers: false },
  { at: box(-18.9, 2.5, -14, 18.7), y: CAP, over: true, thick: 1, rail: 0, walls: 0, soldiers: false },
  { at: box(-4, 2.5, 3.9, 18.7), y: CAP, over: true, thick: 1, rail: 0, walls: 0, soldiers: false },
  { at: box(-18.9, 18.7, 3.9, 24), y: CAP, over: true, thick: 1, rail: 0, walls: 0, soldiers: false },
  // ------------------------------------------------------------------ the rope bridge, sagging to its middle; the east room over the channel
  { at: box(-16.5, 15, -6.75, 18), y: { from: [-16, 16.5, GA], to: [-6.75, 16.5, 8.5] }, over: true, thick: 0.12, key: 'cw_wood', side: 'cw_wood_dark', railKey: 'cw_wood_dark' },
  { at: box(-6.75, 15, 3, 18), y: { from: [-6.75, 16.5, 8.5], to: [2.5, 16.5, GA] }, over: true, thick: 0.12, key: 'cw_wood', side: 'cw_wood_dark', railKey: 'cw_wood_dark' },
  { at: box(3, 13, 16, 21), y: GA, over: true, thick: 0.8, rail: 0, roof: CLEAR, walls: CAP - GA, crown: false },
  // ------------------------------------------------------------------ the crossing over the river, and the platform over it in the south
  { at: box(-19.8, -14.5, 6.5, -8.2), y: M, over: true, thick: 0.8, rail: 0, roof: CLEAR, walls: CAP - M, crown: false },
  { at: box(-17.3, -44.3, 5, -35.5), y: M, over: true, thick: 0.9, rail: 0.9 },

  // ------------------------------------------------------------------ the west start: its court, the annex, the north part, the two stairs up
  { at: [[-88, -2], [-75, -2], [-75, 5], [-81, 5], [-81, 17], [-88, 17]], y: AN, walls: TOP - AN },
  { at: [[-75, -11.5], [-69.5, -11.5], [-69.5, -7], [-64, -7], [-64, -11.5], [-58, -11.5], [-58, 14.8], [-75, 14.8]], y: WS, walls: TOP - WS },
  { at: [[-69, 14.8], [-52.5, 14.8], [-52.5, 21.25], [-57, 21.25], [-57, 27.25], [-52.5, 27.25], [-52.5, 31], [-69, 31]], y: WS, walls: TOP - WS },
  { at: box(-57, 21.25, -52.5, 27.25), y: { from: [-57, 24.25, WS], to: [-53, 24.25, M] }, steps: true, walls: TOP - M },
  { at: box(-69.5, -11.5, -64, -7), y: { from: [-66.75, -7, WS], to: [-66.75, -11, M] }, steps: true, walls: TOP - M },
  // ------------------------------------------------------------------ the south-west court, the tunnel of two rooms east of it
  { at: [[-75, -36], [-52, -36], [-52, -24], [-58, -24], [-58, -11.5], [-75, -11.5]], y: M },
  way(box(-52, -34.3, -49.5, -27.7)),
  way(box(-49.5, -36, -33, -26.2)),
  way(box(-33, -34.3, -30, -27.7)),
  way(box(-30, -42, -19.8, -26.2)),
  way(box(-19.8, -41.5, -17.3, -36)),
  // ------------------------------------------------------------------ the west site, its passages south and east
  { at: box(-52.5, 11, -36, 31), y: M },
  way(box(-46, 4, -39, 11)),
  way(box(-36, 13.5, -34, 17), { from: [-36, 15.25, M], to: [-34.4, 15.25, GA] }),
  // ------------------------------------------------------------------ the mid-west court: its raised walk, its floor, the way north
  { at: [[-51, -18], [-36, -18], [-36, -14], [-47, -14], [-47, 4], [-51, 4]], y: H },
  { at: [[-47, -14], [-36, -14], [-36, -14.5], [-26, -14.5], [-26, -18], [-24.5, -18], [-24.5, -14.5], [-19.8, -14.5], [-19.8, -1], [-36, -1], [-36, 4], [-47, 4]], y: M },
  way(box(-30, -1, -26, 1), { from: [-28, -1, M], to: [-28, 0.6, GA] }),
  // the stair down to the river, along the foot of the crossing
  { at: box(-24.5, -18, -17.5, -14.5), y: { from: [-24.5, -16.25, M], to: [-17.5, -16.25, BANK] }, steps: true, walls: TOP - M },
  // ------------------------------------------------------------------ the gallery and the west room by the bridge, the ledge the bridge hangs from
  way(box(-33, 1, -24, 13), GA),
  way(box(-34, 13, -18.6, 21), GA),
  { at: box(-18.6, 14.5, -16.5, 18.5), y: GA },

  // ------------------------------------------------------------------ the river: the pool in the hall, the bed through the south-east court and under the platform
  bed([[-17.5, -44.3], [-13, -44.3], [-13, 23], [-18, 23], [-18, 18.5], [-16.5, 18.5], [-16.5, 14.5], [-18, 14.5], [-18, -8.2], [-17.5, -8.2]], BANK),
  bed(box(-13, -44.3, -11, 19), SH),
  bed(box(-11, -44.3, -3, 19), 0),
  bed(box(-3, -44.3, -1, 13), SH),
  bed([[-1, -44.3], [0.5, -44.3], [0.5, -16], [6.5, -16], [6.5, -14.5], [7, -14.5], [7, -3], [2, -3], [2, 11], [-1, 11]], BANK),
  bed(box(-13, 19, -3, 21), SH),
  bed(box(-13, 21, 3, 23), BANK),
  bed(box(-1, 11, 3, 13), SH),
  // the channel under the east room, east into the water courts
  bed(box(-3, 13, 16, 18.5), 0),
  bed(box(-3, 18.5, 16, 21), SH),
  // the far pool south of the platform's wall
  bed(box(-17.5, -49, 5, -44.3), 0),
  // ------------------------------------------------------------------ the terraces down the river's east bank, with a ramp of ours along the court's wall
  { at: box(0.5, -21.5, 4.9, -16), y: 3.66 },
  { at: box(4.9, -21.5, 6.5, -16), y: { from: [5.7, -16, BANK], to: [5.7, -21, 3.66] } },
  { at: box(0.5, -24, 6.5, -21.5), y: { from: [3.5, -21.5, 3.66], to: [3.5, -24, 4.88] } },
  { at: box(0.5, -26.5, 6.5, -24), y: 4.88 },
  { at: box(0.5, -32.5, 4.9, -26.5), y: 7.72 },
  { at: box(4.9, -32.5, 6.5, -26.5), y: { from: [5.7, -26.5, 4.88], to: [5.7, -32, 7.72] } },
  { at: box(0.5, -35.5, 6.5, -32.5), y: M },
  // ------------------------------------------------------------------ the south-east court
  { at: [[5, -44.3], [16.5, -44.3], [16.5, -8.2], [6.5, -8.2], [6.5, -35.5], [5, -35.5]], y: M },

  // ------------------------------------------------------------------ the east court: its floor, the way south, the way east, the link to the east room
  { at: [[6, -2], [38.5, -2], [38.5, 8], [36, 8], [36, 6.5], [18, 6.5], [18, 10], [6, 10]], y: M },
  way(box(8, -8.2, 15, -2)),
  way(box(38.5, -2, 42, 5)),
  way(box(12, 10, 15, 13), { from: [13.5, 10, M], to: [13.5, 12.5, GA] }),
  // the stair down to the water court, a landing half way
  { at: box(18, 6.5, 24, 10), y: { from: [18, 8.25, M], to: [24, 8.25, 4.6] }, steps: true, walls: TOP - M },
  { at: box(24, 6.5, 27, 10), y: 4.6, walls: TOP - 4.6 },
  { at: box(27, 6.5, 32, 10), y: { from: [27, 8.25, 4.6], to: [32, 8.25, BANK] }, steps: true, walls: TOP - 4.6 },
  bed(box(32, 6.5, 36, 10), BANK),
  // ------------------------------------------------------------------ the water courts and the channel between them
  bed(box(17, 10, 36, 12), BANK),
  bed([[16, 12], [36, 12], [36, 11], [57, 11], [57, 14], [16, 14]], SH),
  bed(box(16, 14, 53, 18.5), 0),
  bed(box(16, 18.5, 53, 21), SH),
  bed(box(17, 21, 36, 25), BANK),
  bed([[39, 21], [53, 21], [53, 20], [57, 20], [57, 25], [39, 25]], BANK),
  bed(box(53, 14, 65, 20), BANK),
  // the arm north of the second court, the far pool behind it
  bed(box(47, 25, 57, 34), 0.6),
  bed(box(51, 34, 56, 44), SH),
  bed(box(41, 44, 66, 49), 0),
  // ------------------------------------------------------------------ the ramps from the water up to the east start
  { at: box(60, 20, 65, 29), y: { from: [62.5, 20, BANK], to: [62.5, 28.5, 4.06] }, walls: TOP - 4.06 },
  { at: box(60, 29, 72, 34), y: 4.06, walls: TOP - 4.06 },
  { at: box(66, 20, 72, 29), y: { from: [69, 28.5, 4.06], to: [69, 20.5, ES] }, walls: TOP - ES },
  { at: box(66, 12, 72, 20), y: ES, walls: TOP - ES },
  // ------------------------------------------------------------------ the east start: its court, the room and the ramp west of it, the way and the room east of it
  { at: box(58, -12, 75, 12), y: ES, walls: TOP - ES },
  { at: box(42, -12, 49, 8), y: M },
  way(box(49, -12, 58, -6), { from: [49, -9, M], to: [55, -9, ES] }),
  way(box(75, -6, 80, 2), ES),
  way(box(80, -14, 88, 11), ES),
];

/** Water: a surface at a height over the floor that is its bed. */
export const WATER = [
  { at: box(-13, -44.3, -1, 21), y: WL },
  { at: box(-1, 11, 3, 13), y: WL },
  { at: box(-3, 13, 16, 21), y: WL },
  { at: box(-17.5, -49, 5, -44.3), y: WL },
  { at: [[16, 12], [36, 12], [36, 11], [57, 11], [57, 14], [53, 14], [53, 21], [16, 21]], y: WL },
  { at: box(47, 25, 57, 34), y: WL },
  { at: box(51, 34, 56, 44), y: WL },
  { at: box(41, 44, 66, 49), y: WL },
];

/** Walls along a line, with what is cut in them: `holes` of `{ at, w, h, sill }`, `at` in metres from the first point. */
export const WALLS = [
  // the crossing: its north wall, its south wall with four windows over the river, a doorway at each end
  { line: [[-19.8, -8.2], [6.5, -8.2]], y: M, h: CLEAR, t: 0.6, crown: false },
  { line: [[-19.8, -14.5], [6.5, -14.5]], y: M, h: CLEAR, t: 0.6, crown: false, holes: [{ at: 4.8, w: 2.2, h: 1.7, sill: 0.9 }, { at: 9.8, w: 2.2, h: 1.7, sill: 0.9 }, { at: 14.8, w: 2.2, h: 1.7, sill: 0.9 }, { at: 19.8, w: 2.2, h: 1.7, sill: 0.9 }] },
  { line: [[-19.5, -14.5], [-19.5, -8.2]], y: M, h: CLEAR, t: 0.6, crown: false, holes: [{ at: 3.15, w: 3.2, h: 3.3 }] },
  { line: [[6.2, -14.5], [6.2, -8.2]], y: M, h: CLEAR, t: 0.6, crown: false, holes: [{ at: 3.15, w: 3.2, h: 3.3 }] },
  // the east room: its walls on the slab, the doorway the bridge comes in by, the doorway of the link
  { line: [[3.3, 13], [3.3, 21]], y: GA, h: CLEAR, t: 0.6, crown: false, holes: [{ at: 3.5, w: 3, h: 3.3 }] },
  { line: [[3, 13], [16, 13]], y: GA, h: CLEAR, t: 0.6, crown: false, holes: [{ at: 10.5, w: 3, h: 3.3 }] },
  { line: [[16, 13], [16, 21]], y: GA, h: CLEAR, t: 0.6, crown: false },
  { line: [[3, 21], [16, 21]], y: GA, h: CLEAR, t: 0.6, crown: false },
  // what hangs over the channel's two mouths under that room
  { line: [[3, 13], [3, 21]], y: 3, h: GA - 0.8 - 3, t: 0.6, crown: false },
  { line: [[16, 13], [16, 21]], y: 3, h: GA - 0.8 - 3, t: 0.6, crown: false },
  // the platform: its west wall and the doorway of the tunnel, its south wall over the river's way out
  { line: [[-17.3, -44.3], [-17.3, -35.5]], y: M, h: TOP - M, t: 0.6, holes: [{ at: 5.55, w: 5.5, h: 3.6 }] },
  { line: [[-17.3, -44.3], [5, -44.3]], y: M, h: TOP - M, t: 0.6 },
  // kerbs: along the east court over the stair to the water, along the south-west court over the west start, by the stair to the river
  { line: [[18, 6.25], [36, 6.25]], y: M, h: GA - M, t: 0.5, crown: false },
  { line: [[-75, -11.75], [-69.5, -11.75]], y: M, h: H - M, t: 0.5, crown: false },
  { line: [[-64, -11.75], [-58, -11.75]], y: M, h: H - M, t: 0.5, crown: false },
  { line: [[-24.5, -14.75], [-19.8, -14.75]], y: M, h: 1, t: 0.5, crown: false },
  // the flanks of the west start's two stairs
  { line: [[-69.75, -11.5], [-69.75, -5.5]], y: WS, h: H - WS, t: 0.5, crown: false },
  { line: [[-63.75, -11.5], [-63.75, -5.5]], y: WS, h: H - WS, t: 0.5, crown: false },
  { line: [[-58.5, 21], [-52.5, 21]], y: WS, h: H - WS, t: 0.5, crown: false },
  { line: [[-58.5, 27.5], [-52.5, 27.5]], y: WS, h: H - WS, t: 0.5, crown: false },
];

/** Doors: `facing` is the way a doorway looks, in degrees from north toward east. Their leaves stand ajar. */
export const DOORS = [
  // the tunnel's two pairs, the pair onto the platform
  { at: [-50.8, -31], y: M, facing: 90, w: 6.6, h: 3.6, open: 55 },
  { at: [-31.5, -31], y: M, facing: 90, w: 6.6, h: 3.6, open: 55 },
  { at: [-17.3, -38.75], y: M, facing: 90, w: 5.5, h: 3.6, open: 62 },
  // the pair in the way between the east court and the crossing, the crossing's own
  { at: [11.5, -4.6], y: M, facing: 0, w: 6.6, h: 3.6, open: 52 },
  { at: [-19.5, -11.35], y: M, facing: 270, w: 3.2, h: 3.3, open: 100 },
  { at: [6.2, -11.35], y: M, facing: 90, w: 3.2, h: 3.3, open: 100 },
];

/** Solids of any footprint, from `y` to `top`. */
export const MASSES = [
  // the blocks in the two starts
  { at: box(-61.35, 4.95, -59.65, 6.65), y: WS, top: 9.8, key: 'cw_ashlar' },
  { at: box(-61.35, -2.05, -59.65, -0.35), y: WS, top: 9.8, key: 'cw_ashlar' },
  { at: box(58, 2.7, 59.7, 6), y: ES, top: 9.77, key: 'cw_ashlar' },
  { at: box(58, -1.3, 59.7, 2), y: ES, top: 9.77, key: 'cw_ashlar' },
  // the pillars of the south-west court and of the mid-west court
  { at: box(-62.85, -15.05, -61.15, -13.35), y: M, top: 13, key: 'cw_ashlar', crown: true },
  { at: box(-62.85, -21.15, -61.15, -19.45), y: M, top: 13, key: 'cw_ashlar', crown: true },
  { at: box(-58.05, -26.35, -56.35, -24.65), y: M, top: 13, key: 'cw_ashlar', crown: true },
  { at: box(-50.15, -2.85, -48.45, -1.15), y: H, top: 13, key: 'cw_ashlar', crown: true },
  { at: box(-50.15, -10.85, -48.45, -9.15), y: H, top: 13, key: 'cw_ashlar', crown: true },
  { at: box(-34.65, -5.05, -32.95, -3.35), y: M, top: 13, key: 'cw_ashlar', crown: true },
  { at: box(-25.05, -5.05, -23.35, -3.35), y: M, top: 13, key: 'cw_ashlar', crown: true },
  // the buttresses of the river court: on the river's west wall, on the court's east wall, on the platform's south wall
  { at: box(-17.5, -24, -15.5, -20.5), y: 0, top: TOP },
  { at: box(-17.5, -32, -15.5, -29), y: 0, top: TOP },
  { at: box(14.7, -19.3, 16.5, -16), y: M, top: TOP },
  { at: box(14.7, -29, 16.5, -26), y: M, top: TOP },
  { at: box(14.7, -38.5, 16.5, -35.5), y: M, top: TOP },
  { at: box(-2.3, -44.3, 1, -42.3), y: M, top: TOP },
  { at: box(7.3, -44.3, 10.7, -42.3), y: M, top: TOP },
  // the pier in the east court's south wall
  { at: box(28.3, -2, 31.3, 0.2), y: M, top: TOP },
];

/** Crates: [east, north, side, turn in degrees, up]. Without `up` a crate stands on the highest floor there. */
export const CRATES = [
  // the west start, the south-west court, the west site
  [-71.3, 12.4, 3, 45, WS], [-62, 12, 3, 15, WS], [-67.5, 28.6, 3, 15, WS],
  [-72, -30.3, 3.2, 15, M],
  [-49.6, 14.4, 3.2, 15, M], [-38.5, 25.4, 3.2, 15, M], [-50.6, 29.9, 1.8, 0, M],
  // the two rooms by the bridge
  [-29.5, 14.6, 1.6, 20, GA], [-29.6, 16.2, 1.4, -10, GA],
  [8, 15, 1.7, 0, GA], [8, 16.7, 1.7, 8, GA], [8.7, 19.6, 1.5, -12, GA],
  // the east court and its bay, the room east of it
  [8.4, 8.3, 3.2, 0, M], [10.9, 8.9, 1.7, 10, M], [24.5, 4.8, 1.6, 0, M], [43.2, -5, 1.6, 0, M],
  // the south-east court: by the terraces, in its north-east corner, in its south-east corner, on the platform's parapet side
  [7.7, -19.6, 3.2, 5, M], [7.5, -21.9, 1.6, -15, M],
  [15.2, -9.7, 1.6, 0, M], [15.3, -11.4, 1.7, 8, M], [15.25, -10.5, 1.5, 12, M + 1.7],
  [12.2, -42.2, 1.7, 30, M], [13.9, -41.1, 1.7, 40, M],
  [-6.3, -36.5, 1.5, 10, M], [-4.6, -36.4, 1.5, -8, M],
  // in the crossing
  [-1.2, -9.5, 2, 0, M], [-7, -13.3, 1.5, 0, M],
];

/** The tops of the blocks beside the floors, where they are not `PLACE.wall` over the floor: the mass the courts are cut in, a step back from their walls. */
export const BLOCKS = [
  // west of the annex and of the west start, north of them and of the west site
  { at: box(-96, -3.5, -89.5, 18.5), top: CAP },
  { at: box(-96, 18.5, -70.5, 39), top: CAP },
  { at: box(-70.5, 32.5, -30, 39), top: CAP },
  { at: box(-79.5, 6.5, -76.5, 18.5), top: CAP },
  { at: box(-96, -44, -76.5, -3.5), top: CAP },
  { at: box(-76.5, -44, -19, -37.5), top: CAP },
  // between the west start and the courts east of it; south of the mid-west court
  { at: box(-56.5, -22.5, -52.5, 9.5), top: CAP },
  { at: box(-50.5, -24.5, -19.5, -19.5), top: CAP },
  { at: box(-34.5, -19.5, -27.5, -16), top: CAP },
  // the hall's south wall, over the east court's bay
  { at: box(3.9, 10.3, 16, 13), top: CAP },
  // east and south of the river court; south of the east court
  { at: box(18, -50, 30, -3.5), top: CAP },
  { at: box(-25, -58, 18, -50.5), top: CAP },
  { at: box(30, -9.5, 40.5, -3.5), top: CAP },
  // around the east start and the room west of it
  { at: box(40.5, -20, 76.5, -13.5), top: CAP },
  { at: box(50.5, -4.5, 56.5, 9.5), top: CAP },
  // around the water courts, the ramps and the far pool
  { at: box(15, 26.5, 37.5, 33), top: CAP },
  { at: box(37.5, 26.5, 45.5, 42.5), top: CAP },
  { at: box(45.5, 35.5, 49.5, 42.5), top: CAP },
  { at: box(73.5, 13.5, 82, 42.5), top: CAP },
  { at: box(57.5, 35.5, 73.5, 42.5), top: CAP },
  { at: box(39.5, 50.5, 67.5, 58), top: CAP },
  { at: box(33, 42.5, 39.5, 58), top: CAP },
  { at: box(67.5, 42.5, 82, 58), top: CAP },
];

/** Lamps: [east, north, up], `up` what a lamp hangs from. */
export const LAMPS = [
  // the way south of the west site, the tunnel's rooms and its doors
  [-42.5, 7.5, M + CLEAR],
  [-50.7, -31, M + CLEAR], [-45, -31, M + CLEAR], [-37, -31, M + CLEAR], [-31.5, -31, M + CLEAR], [-25, -30, M + CLEAR], [-25, -38, M + CLEAR], [-18.5, -38.75, M + CLEAR],
  // the gallery and the west room, the two ways into them
  [-28.5, 4, GA + CLEAR], [-28.5, 10, GA + CLEAR], [-30, 17, GA + CLEAR], [-23, 17, GA + CLEAR], [-35, 15.25, M + CLEAR + 0.4], [-28, 0, M + CLEAR + 0.4],
  // the crossing; under it, over the river
  [-16, -11.3, M + CLEAR], [-9, -11.3, M + CLEAR], [-2, -11.3, M + CLEAR], [4, -11.3, M + CLEAR],
  [-7, -11.3, M - 0.8],
  // the east room, the link, the ways off the east court
  [6.5, 17, GA + CLEAR], [12, 17, GA + CLEAR], [13.5, 11.5, GA + CLEAR - 0.3], [11.5, -6.5, M + CLEAR], [11.5, -2.8, M + CLEAR], [40.2, 1.5, M + CLEAR],
  // under the bridge's ends and the east room, over the pool and the channel
  [-15, 16.5, GA - 0.9], [1.5, 16.5, GA - 0.9], [9.5, 15.5, GA - 0.8],
  // under the platform
  [-9, -40, M - 0.9], [-3, -40, M - 0.9],
  // the east start's ramp, the way east of its court, the east room
  [51.5, -9, M + CLEAR - 0.3], [56, -9, ES + CLEAR], [77.5, -2, ES + CLEAR], [84, -9, ES + CLEAR], [84, -1.5, ES + CLEAR], [84, 6, ES + CLEAR],
];

