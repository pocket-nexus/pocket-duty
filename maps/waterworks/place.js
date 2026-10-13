/**
 * WATERWORKS, a place of Pocket Duty.
 *
 * Drawn by hand, floor by floor, in the place's own coordinates: metres east and north of its middle,
 * heights in metres above its lowest floor (maps/README.md). North is up in the plan below.
 *
 * Three levels lie over one another. The ground (9) is the yards, the sheds, the hall with the upper
 * site, the lobby. Under it run the catwalks, corridors and tunnels (3.25) and, lowest, the two deep
 * rooms (0): the lower site under the hall, and the south room under the south yard.
 *
 *     -82      -30  -15        1        20        37        57      85
 *  +48                         +--ramp room--+
 *  +32                         | down, turn, |
 *  +27                    +----+--- lobby ---+----+
 *  +17        north tip --+ sheds |          | east
 *       road            / | (roof | THE HALL | room    apron (10.5)
 *   +1 ====== gate ==== yard  15) | site, 9  | door ============== east
 *   -6  west            \ +-------+ over the |                    start
 *  -11  start (9)        \| shed  | LOWER    | south-east yard
 *  -15                    \ tank  | SITE (0) +----+ stair down
 *  -22                     \      +- ducts --+    | (to 3.25)
 *  -32                      \     | SOUTH    |  south-east
 *  -40                       \____| ROOM (0) |__  building
 *  -48                            +----------+
 */

/** The map's name, its turn in the world (which side the sun is on), its walls' height, the starts. */
export const PLACE = {
  id: 'waterworks',
  title: 'Waterworks',
  turn: -0.3,
  wall: 8,
  // [east, north, facing in degrees from north toward east, tag]. The first is the player's: the yard, the
  // sheds ahead and the hall's wall over them. The title's picture and the export's exposure are that view.
  starts: [
    [-23, -2.5, 80, 'yard'],
    [-78, 4, 110, 'west start'],
    [77, 9, 262, 'east start'],
    [-72, -3, 90, 'west start'],
    [-79, -1, 90, 'west start'],
    [-76, 8, 150, 'west start'],
    [74, 4, 280, 'east start'],
    [80, 2, 290, 'east start'],
    [81, 10, 260, 'east start'],
    [-19, 6, 120, 'yard, north'],
    [-13.5, -16, 60, 'yard, by the tank'],
    [10, -30, 0, 'south yard'],
    [26, -20, 330, 'south yard, east'],
    [44, -14, 280, 'south-east yard'],
    [26, -9, 300, 'east of the hall'],
    [40, 5, 270, 'apron'],
  ],
};

// The surfaces the floors and solids name (look.js).
const CLAY = 'ww_clay', TAR = 'ww_tar', TILE = 'ww_tile', SLAB = 'ww_slab', GRATE = 'ww_grate';
const BLOCK = 'ww_block', SHEET = 'ww_sheet', ROOF = 'ww_roof', STEEL = 'ww_steel', ROCK = 'ww_rock', PAINT = 'ww_paint';
const CAN_RED = 'ww_can_red', CAN_GREEN = 'ww_can_green', TANK = 'ww_tank';
// The heights most floors lie at: the ground, the hall's platform, the east apron, the catwalks and
// tunnels, the ducts, the deep rooms, the ramp room's far floor.
const G = 9, P = 9.75, E = 10.5, C = 3.25, D = 4.25, B = 0, N = 7.25;
// The ground over what lies under it is a slab this thick: its underside is the tunnels' ceiling.
const THICK = 1.5, UNDER = G - THICK;
// The roofs: the sheds' and the east room's, the sheds' taller block, the hall's catwalk, the hall's.
const R15 = 15, R17 = 17, CAT = 17.2, RH = 21.5;

/** A rectangle's corners from its west, south, east and north sides. */
const box = (w, s, e, n) => [[w, s], [e, s], [e, n], [w, n]];
/** The ground over a floor that lies under it. */
const over = (at, y = G, more = {}) => ({ at, y, over: true, thick: THICK, rail: 0, side: SLAB, ...more });
/** A roof nobody walks. */
const roof = (at, y, more = {}) => ({ at, y, over: true, thick: 0.5, rail: 0, soldiers: false, key: ROOF, side: SHEET, ...more });
/** A floor under the ground: its walls are blocks, up to the slab over it. */
const low = (at, y, more = {}) => ({ at, y, key: SLAB, wall: BLOCK, side: BLOCK, crown: false, ...more });

/**
 * The floors: polygons of [east, north], level (`y`) or sloped (`y: { from, to }`, each [east, north, up]).
 * The floors that stand over others come first, from the highest down: the walk grid takes the ground
 * under each in this order.
 */
export const GROUND = [
  // ------------------------------------------------------------------ roofs nobody walks: the hall's two halves about its skylight, the east room's about its platform, the porch's, the lobby's, the ramp room's
  roof(box(1, -15.5, 9, 17.5), RH),
  roof(box(12.5, -15.5, 20.5, 17.5), RH),
  roof([[20.5, 8.5], [30.5, 8.5], [30.5, 17.5], [20.5, 17.5], [20.5, 14.5], [28, 14.5], [28, 11], [22.4, 11], [22.4, 9.6], [20.5, 9.6]], R15),
  roof(box(6, -21.5, 16.5, -15.5), 14),
  roof(box(-4.4, 17.5, 25.9, 27.9), R15),
  roof(box(1.4, 27.9, 20.6, 48.6), R15),
  // ------------------------------------------------------------------ the hall's catwalk under its roof, through the east wall to the platform a ladder reaches
  { at: box(2.5, 13.5, 19, 15.5), y: CAT, over: true, thick: 0.25, key: GRATE, side: STEEL, railKey: STEEL },
  { at: box(19, 13.5, 20.5, 14.5), y: CAT, over: true, thick: 0.25, rail: 0, key: GRATE, side: STEEL },
  { at: box(20.5, 11, 28, 14.5), y: CAT, over: true, thick: 0.25, key: GRATE, side: STEEL, railKey: STEEL },
  // ------------------------------------------------------------------ the sheds' roofs, which a player crosses: the taller block a ladder reaches, the long roof, the low one
  { at: [[-8, -3], [1, -3], [1, 2.9], [-0.6, 2.9], [-0.6, 4], [-8, 4]], y: R17, over: true, thick: 0.4, rail: 0.3, key: ROOF, side: SHEET, railKey: STEEL },
  { at: [[-12.5, -6.5], [1, -6.5], [1, -3], [-8, -3], [-8, 4], [1, 4], [1, 17.5], [-12.5, 17.5]], y: R15, over: true, thick: 0.4, rail: 0.3, soldiers: false, key: ROOF, side: SHEET, railKey: STEEL },
  { at: box(-12.5, -11.5, 1, -6.5), y: 12.5, over: true, thick: 0.4, rail: 0.3, soldiers: false, key: ROOF, side: SHEET, railKey: STEEL },
  // ------------------------------------------------------------------ the catwalks' bridges over the mouths of the lower rooms' tunnels
  { at: box(2.5, -3.5, 5, 0.5), y: C, over: true, thick: 0.25, key: GRATE, side: STEEL, railKey: STEEL },
  { at: box(17, -3.5, 19, 0.5), y: C, over: true, thick: 0.25, key: GRATE, side: STEEL, railKey: STEEL },
  { at: box(2.5, -31.5, 5, -27.5), y: C, over: true, thick: 0.25, key: GRATE, side: STEEL, railKey: STEEL },
  { at: box(17, -31.5, 19, -27.5), y: C, over: true, thick: 0.25, key: GRATE, side: STEEL, railKey: STEEL },

  // ------------------------------------------------------------------ the ground over what lies under it
  // the hall: its floor about the platform, its south end about the two hatches
  over(box(1, 6, 20.5, 17.5), G, { key: TILE }),
  over(box(5, 4.5, 20.5, 6), G, { key: TILE }),
  over(box(6.75, -6.5, 20.5, 4.5), G, { key: TILE }),
  over(box(1, -1.5, 2.5, 4.5), G, { key: TILE }),
  over(box(1, -6.5, 2.5, -4), G, { key: TILE }),
  over(box(1, -11.5, 20.5, -6.5), G, { key: TILE }),
  over(box(1, -15.5, 5.5, -11.5), G, { key: TILE }),
  over(box(7, -15.5, 14.5, -11.5), G, { key: TILE }),
  over(box(16, -15.5, 20.5, -11.5), G, { key: TILE }),
  over(box(5.5, -15.5, 7, -13.5), G, { key: TILE }),
  over(box(14.5, -15.5, 16, -13.5), G, { key: TILE }),
  // the platform by the west door, through the door to its landing outside, and the ways down from it
  { at: [[-1.5, -6], [1, -6], [1, -4], [2.5, -4], [2.5, -6.5], [6.75, -6.5], [6.75, 4.5], [2.5, 4.5], [2.5, -1.5], [-1.5, -1.5]], y: P, over: true, thick: P - UNDER, rail: 0, key: PAINT, side: SLAB },
  { at: box(2.5, 4.5, 5, 6), y: { from: [3.75, 4.5, P], to: [3.75, 6, G] }, over: true, thick: THICK, rail: 0, key: PAINT, side: SLAB },
  { at: box(-1.5, -1.5, 1, 0), y: { from: [-0.25, -1.5, P], to: [-0.25, 0, G] }, over: true, thick: THICK, rail: 0, key: SLAB, side: SLAB, steps: true },
  over(box(-1.5, 0, 1, 1.2), G, { key: SLAB }),
  // the lobby over the corridor, the landing over the ramp
  over(box(3.9, 17.5, 18.1, 27.1), G, { key: TILE }),
  over(box(6.9, 27.1, 14.6, 32), G, { key: SLAB }),
  // the yard over the west tunnel and over the ways up from the two deep rooms
  over(box(-15.2, -33.1, -9.8, 2.1)),
  over(box(-9.8, -4.2, -1.5, 1.2), G, { key: SLAB }),
  over(box(-9.8, -32.2, 1, -26.8)),
  // the south yard over the ducts and over the south room
  over(box(1, -21.6, 20.5, -15.5), G, { key: TAR }),
  over(box(1, -40.5, 20.5, -21.6), G, { key: TAR }),
  over(box(1, -48.6, 20.5, -40.5), G, { key: TAR, soldiers: false }),
  // east of the hall: over the ways up, the east room and its corridor, the east tunnel, the rooms at its south end
  over(box(20.5, -4.2, 31.9, 1.2), G, { key: TAR }),
  over(box(20.5, -32.2, 31.9, -26.8), G, { key: TAR }),
  over(box(20.5, 3.9, 29.1, 13.1), G, { key: SLAB }),
  over(box(29.1, 5.9, 31.9, 11.1), G, { key: SLAB }),
  over(box(31.9, -38.6, 34, 11.1), G, { key: TAR }),
  over(box(34, -38.6, 37.1, -3), G, { key: TAR }),
  over(box(34, -3, 37.1, 11.1), { from: [34, 4, G], to: [37.1, 4, E] }, { key: TAR }),
  over(box(29.4, -38.6, 31.9, -32.2), G, { key: TAR }),
  over(box(37.1, -38.6, 48, -31.9), G, { key: SLAB }),
  over(box(48, -38.6, 57.6, -31.9), G, { key: SLAB, soldiers: false }),

  // ------------------------------------------------------------------ the lower site: its floor about the ramp, the catwalks, the ramp
  low([[5, -8.5], [17, -8.5], [17, 14.5], [15, 14.5], [15, 10.5], [13.5, 10.5], [13.5, 4.5], [8.5, 4.5], [8.5, 10.5], [5, 10.5]], B, { key: TILE }),
  low(box(8.5, 4.5, 13.5, 15), { from: [11, 4.5, B], to: [11, 15, C] }),
  low(box(2.5, 0.5, 5, 17.5), C, { key: GRATE }),
  low(box(2.5, -9.5, 5, -3.5), C, { key: GRATE }),
  low(box(5, -9.5, 17, -8.5), C, { key: GRATE }),
  low(box(17, -9.5, 19, -3.5), C, { key: GRATE }),
  low(box(17, 0.5, 19, 17.5), C, { key: GRATE }),
  low(box(5, 10.5, 8.5, 15), C, { key: GRATE }),
  low(box(13.5, 10.5, 15, 15), C, { key: GRATE }),
  low(box(5, 15, 17, 17.5), C, { key: GRATE }),
  // its tunnels: level, then up to the west tunnel and to the east one
  low(box(-4.5, -3.5, 5, 0.5), B),
  low(box(-10.5, -3.5, -4.5, 0.5), { from: [-4.5, -1.5, B], to: [-10.5, -1.5, C] }, { steps: true }),
  low(box(17, -3.5, 25.5, 0.5), B),
  low(box(25.5, -3.5, 31.5, 0.5), { from: [25.5, -1.5, B], to: [31.5, -1.5, C] }, { steps: true }),
  low(box(31.5, -3.5, 32.5, 0.5), C),
  // ------------------------------------------------------------------ north of it: the corridor under the lobby, the ramp up, the ramp room
  low(box(4.5, 17.5, 17.5, 26.5), C),
  low(box(7.5, 26.5, 14, 39.5), { from: [10.75, 26.5, C], to: [10.75, 39.5, N] }),
  { at: box(2, 32, 7.5, 39.5), y: { from: [4.75, 32, G], to: [4.75, 39.5, N] }, key: SLAB, wall: BLOCK, side: BLOCK, crown: false },
  { at: box(14, 32, 20, 39.5), y: { from: [17, 32, G], to: [17, 39.5, N] }, key: SLAB, wall: BLOCK, side: BLOCK, crown: false },
  { at: [[2, 39.5], [20, 39.5], [20, 43], [15, 43], [15, 48], [7, 48], [7, 43], [2, 43]], y: N, key: SLAB, wall: BLOCK, side: BLOCK, crown: false },
  // ------------------------------------------------------------------ the west tunnel, the east room, its corridor, the east tunnel and the rooms at its end
  low(box(-14.5, -32.5, -10.5, 1.5), C),
  low(box(19, 4.5, 28.5, 12.5), C),
  low(box(28.5, 6.5, 32.5, 10.5), C),
  low(box(32.5, -32.5, 36.5, 10.5), C),
  low(box(30, -38, 57, -32.5), C),
  // the stair from those rooms up to the south-east yard, in its own well
  { at: box(50, -32.5, 53, -22), y: { from: [51.5, -32.5, C], to: [51.5, -22, G] }, key: SLAB, wall: BLOCK, side: BLOCK, crown: false, steps: true },
  // ------------------------------------------------------------------ the south room: its floor about the ramp, the catwalks, the ramp, its tunnels
  low([[5, -41.5], [8.5, -41.5], [8.5, -35.5], [13.5, -35.5], [13.5, -41.5], [17, -41.5], [17, -22], [5, -22]], B, { key: TILE }),
  low(box(8.5, -45, 13.5, -35.5), { from: [11, -35.5, B], to: [11, -45, C] }),
  low(box(2.5, -27.5, 5, -21), C, { key: GRATE }),
  low(box(2.5, -48, 5, -31.5), C, { key: GRATE }),
  low(box(17, -27.5, 19, -21), C, { key: GRATE }),
  low(box(17, -48, 19, -31.5), C, { key: GRATE }),
  low(box(5, -22, 17, -21), C, { key: GRATE }),
  low(box(5, -45, 8.5, -41.5), C, { key: GRATE }),
  low(box(13.5, -45, 17, -41.5), C, { key: GRATE }),
  low(box(5, -48, 17, -45), C, { key: GRATE }),
  low(box(-4.5, -31.5, 5, -27.5), B),
  low(box(-10.5, -31.5, -4.5, -27.5), { from: [-4.5, -29.5, B], to: [-10.5, -29.5, C] }, { steps: true }),
  low(box(17, -31.5, 25.5, -27.5), B),
  low(box(25.5, -31.5, 31.5, -27.5), { from: [25.5, -29.5, B], to: [31.5, -29.5, C] }, { steps: true }),
  low(box(31.5, -31.5, 32.5, -27.5), C),
  // ------------------------------------------------------------------ the ducts between the two rooms, as narrow ways one walks: two long, two across, a link
  low(box(5.5, -11.5, 7, -9.5), { from: [6.25, -9.5, C], to: [6.25, -11.5, D] }),
  low(box(5.5, -19, 7, -11.5), D),
  low(box(5.5, -21, 7, -19), { from: [6.25, -21, C], to: [6.25, -19, D] }),
  low(box(14.5, -11.5, 16, -9.5), { from: [15.25, -9.5, C], to: [15.25, -11.5, D] }),
  low(box(14.5, -19, 16, -11.5), D),
  low(box(14.5, -21, 16, -19), { from: [15.25, -21, C], to: [15.25, -19, D] }),
  low(box(7, -15.5, 14.5, -14), D),
  low(box(7, -18.5, 14.5, -16.5), D),
  low(box(10, -16.5, 11.5, -15.5), D),

  // ------------------------------------------------------------------ the west start and the road in its cutting, to the gate
  { at: [[-82, 10.5], [-74, 10.5], [-74, 2], [-57, 2], [-57, 1], [-51, 1], [-48, -2.5], [-43, -2.5], [-41, -1], [-38, 1], [-33, 1], [-31, -1], [-30, -1], [-30, -6.5], [-31, -6.5], [-32, -9.5], [-50, -9.5], [-54, -6.5], [-61, -6.5], [-64, -8.5], [-67, -9.5], [-70, -9.5], [-73, -8.5], [-77, -6.5], [-82, -2.5]], y: G, crown: false },
  // ------------------------------------------------------------------ the yard, west of the sheds and the west tunnel
  { at: [[-16, 17.5], [-12.5, 17.5], [-12.5, 2.1], [-15.2, 2.1], [-15.2, -22.5], [-17, -20.5], [-22, -14.5], [-24, -12.5], [-25, -10], [-28, -8.5], [-28, -6.5], [-30, -6.5], [-30, -1], [-28, -1], [-28, 1], [-27, 2.5], [-22, 8], [-16, 16]], y: G, crown: false },
  // the sheds' floors: north of the door's landing, south of it
  { at: [[-12.5, 2.1], [-9.8, 2.1], [-9.8, 1.2], [1, 1.2], [1, 17.5], [-12.5, 17.5]], y: G, key: SLAB, walls: false },
  { at: [[-9.8, -11.5], [1, -11.5], [1, -6], [-1.5, -6], [-1.5, -4.2], [-9.8, -4.2]], y: G, key: SLAB, walls: false },
  // the yard south of the sheds, about the tank, down to the cutting's edge
  { at: box(-9.8, -26.8, 1, -11.5), y: G, crown: false },
  { at: [[-9.8, -33.5], [-6, -34.5], [1, -36.5], [1, -32.2], [-9.8, -32.2]], y: G, crown: false },
  // ------------------------------------------------------------------ the lobby's two wings and its north edge, the landing's two sides
  { at: [[-4, 17.5], [3.9, 17.5], [3.9, 27.1], [6.9, 27.1], [6.9, 32], [2, 32], [2, 27.5], [-4, 27.5]], y: G, key: TILE, walls: false },
  { at: [[18.1, 17.5], [25.5, 17.5], [25.5, 27.5], [20, 27.5], [20, 32], [14.6, 32], [14.6, 27.1], [18.1, 27.1]], y: G, key: TILE, walls: false },
  // ------------------------------------------------------------------ east of the hall: the strip by its door, the room's north end, the yards to the south-east
  { at: [[20.5, 1.2], [31.9, 1.2], [31.9, 5.9], [29.1, 5.9], [29.1, 3.9], [20.5, 3.9]], y: G, key: TAR, walls: false },
  { at: [[20.5, 13.1], [29.1, 13.1], [29.1, 11.1], [34, 11.1], [34, 17.5], [20.5, 17.5]], y: G, key: SLAB, walls: false },
  { at: box(20.5, -26.8, 31.9, -4.2), y: G, key: TAR, crown: false },
  { at: [[20.5, -32.2], [29.4, -32.2], [29.4, -38.4], [24, -40.5], [20.5, -40.5]], y: G, key: TAR, crown: false },
  { at: [[37.1, -8.5], [41.4, -8.5], [48, -9], [52, -11.5], [57.5, -12], [57.5, -22], [53, -22], [50, -22], [48, -22], [48, -31.9], [37.1, -31.9]], y: G, key: TAR, crown: false },
  // ------------------------------------------------------------------ the east apron, the road in its cutting, the east start
  { at: box(34, 11.1, 37.1, 17.5), y: { from: [34, 15, G], to: [37.1, 15, E] }, key: TAR, crown: false },
  { at: box(37.1, -8.5, 41.4, -3), y: { from: [39, -3, E], to: [39, -8.5, G] }, key: TAR, crown: false },
  { at: [[37.1, -3], [37.1, 17.5], [43, 17.5], [43, 15.5], [48, 16.5], [55, 16.5], [56, 15.5], [59, 14.5], [62, 13.5], [68, 13.5], [70, 14.5], [79, 14.5], [83, 13.5], [85, 10.5], [84.5, 0], [83, -5.5], [76, -5.5], [75, 0.5], [71, 4], [69, 5], [63, 6], [61, 8.5], [59, 9.5], [51, 9.5], [47, 7.5], [45, 5], [43.5, 2], [41, -1], [41.4, -3]], y: E, key: TAR, crown: false },
];

/** A wall of blocks on the ground. */
const wall = (line, h, more = {}) => ({ line, y: G, h, t: 0.5, key: BLOCK, crown: false, ...more });

/** Walls along a line, with what is cut in them: `holes` of `{ at, w, h, sill }`, `at` in metres from the first point. */
export const WALLS = [
  // ------------------------------------------------------------------ the hall: west (the way from the sheds, the door on the platform), east (its door, the catwalk's way out), north and south (the big doorways)
  wall([[1.75, -15.5], [1.75, 17.5]], RH - G - 0.5, { t: 1.5, key: SHEET, holes: [{ at: 6.75, w: 1.6, h: 2.5 }, { at: 12.75, w: 2.5, h: 2.7, sill: P - G }] }),
  wall([[19.75, -15.5], [19.75, 17.5]], RH - G - 0.5, { t: 1.5, key: SHEET, holes: [{ at: 15, w: 3, h: 2.8 }, { at: 29.5, w: 1.4, h: 2.4, sill: CAT - G }] }),
  wall([[2.5, 16.5], [19, 16.5]], RH - G - 0.5, { t: 2, key: SHEET, holes: [{ at: 8.5, w: 5, h: 3.8 }] }),
  wall([[2.5, -14.75], [19, -14.75]], RH - G - 0.5, { t: 1.5, key: SHEET, holes: [{ at: 8.5, w: 5, h: 3.8 }] }),
  // the rail along the platform's edge
  wall([[6.6, -6.5], [6.6, 0.8]], 1.75, { t: 0.2, key: STEEL }),
  // ------------------------------------------------------------------ the lobby: its outer walls, the two walls inside it
  wall([[-4.2, 17.5], [-4.2, 27.7]], R15 - G - 0.5),
  wall([[-4.2, 27.7], [2, 27.7]], R15 - G - 0.5),
  wall([[20, 27.7], [25.7, 27.7]], R15 - G - 0.5),
  wall([[25.7, 17.5], [25.7, 27.7]], R15 - G - 0.5),
  wall([[1.5, 17.5], [1.5, 24]], R15 - G - 0.5, { t: 1 }),
  wall([[20.25, 17.5], [20.25, 24]], R15 - G - 0.5),
  // the ramp room's landing: its side walls, the rail over the ramp
  wall([[1.75, 27.7], [1.75, 32]], R15 - G - 0.5),
  wall([[20.25, 27.7], [20.25, 32]], R15 - G - 0.5),
  wall([[7.5, 31.9], [14, 31.9]], 1, { t: 0.2, key: STEEL }),
  // ------------------------------------------------------------------ the sheds: west, north, the two partitions with their doors, the wall across with its door, the low shed's walls
  wall([[-12.25, -0.5], [-12.25, 17.5]], R15 - G - 0.4),
  wall([[-12.5, 17.25], [-4.2, 17.25]], R15 - G - 0.4),
  wall([[-5.75, 7.5], [-5.75, 17], ], R15 - G - 0.4, { holes: [{ at: 5.25, w: 2.4, h: 2.6 }] }),
  wall([[-12.5, 7], [1, 7]], R15 - G - 0.4, { t: 1, holes: [{ at: 3.25, w: 2.5, h: 2.6 }] }),
  wall([[-12.5, -6], [1, -6]], R15 - G - 0.4, { t: 1, holes: [{ at: 9.25, w: 2.5, h: 2.6 }] }),
  wall([[-12.25, -11.5], [-12.25, -6.5]], 3.1),
  wall([[-12.5, -11.25], [1, -11.25]], 3.1),
  // ------------------------------------------------------------------ east of the hall: the east room's walls, the block south of the door
  wall([[20.5, 8.75], [25.5, 8.75]], R15 - G - 0.5),
  wall([[30, 3.5], [30, 17]], R15 - G - 0.5, { t: 1, holes: [{ at: 8, w: 2, h: 2.6 }] }),
  wall([[25.5, 16.75], [30.5, 16.75]], R15 - G - 0.5),
  wall([[24.5, 3.75], [30.5, 3.75]], R15 - G - 0.5),
  wall([[21, 3.75], [24.5, 3.75]], 1.25),
];

/** Doors: `facing` is the way a doorway looks, in degrees from north toward east. */
export const DOORS = [
  { at: [1.75, -2.75], y: P, facing: 270, w: 2.5, h: 2.7, leaves: 2, key: STEEL, open: 100 },
  { at: [19.75, -0.5], y: G, facing: 90, w: 3, h: 2.8, leaves: 2, key: STEEL, open: 100 },
  { at: [11, 16.5], y: G, facing: 0, w: 5, h: 3.8, kind: 'roller', raised: 0.7, key: SHEET },
  { at: [11, -14.75], y: G, facing: 180, w: 5, h: 3.8, kind: 'roller', raised: 0.8, key: SHEET },
];

/** A stack of freight boxes on a footprint, from `y`: one solid a box, each `high`, in the colours given. */
const stack = (at, y, high, ...keys) => keys.map((key, i) => ({ at, y: y + i * high, top: y + (i + 1) * high, key }));

/** Solids of any footprint, from `y` to `top`. */
export const MASSES = [
  // ------------------------------------------------------------------ the tank south of the sheds, and its rim
  { at: [[-8.5, -14.5], [-6.5, -12.5], [0.5, -12.5], [0.5, -25.5], [-3.5, -25.5], [-8.5, -20.5]], y: G, top: 18, key: TANK },
  // freight boxes in the yard by the sheds, in the south yard, east of the hall
  ...stack(box(-18.2, -13, -15.4, -7.5), G, 3, CAN_GREEN),
  ...stack(box(-20.2, -11, -18.6, -7.2), G, 2, CAN_RED),
  ...stack(box(7.3, -35, 13.7, -31.6), G, 3, CAN_RED),
  ...stack(box(28, -19, 30.5, -11.2), G, 3, CAN_GREEN),
  // the block of wall east of the hall, south of its door
  { at: box(20.5, -6.5, 23, -4), y: G, top: 14, key: BLOCK },
  // ------------------------------------------------------------------ the rock and the walls that stand on the slabs' far parts, where the ground ends before what lies under it does
  { at: [[1, -36.5], [9, -39.5], [12, -40.5], [20.5, -40.5], [20.5, -48.6], [1, -48.6]], y: G, top: 17, key: ROCK },
  { at: [[-15.2, -22.5], [-15, -26.5], [-14, -29.5], [-10.5, -33.1], [-15.2, -33.1]], y: G, top: 17, key: ROCK },
  { at: [[29.4, -38.6], [48, -38.6], [48, -35.5], [32, -35.5], [32, -37.5], [29.4, -38.4]], y: G, top: 17, key: BLOCK },
  { at: box(48, -38.6, 57.6, -31.9), y: G, top: 17, key: BLOCK },
  // ------------------------------------------------------------------ in the lower site: the block on its floor; under the lobby: the pier in the corridor
  { at: box(8, -6.5, 10.5, -4.5), y: B, top: C, key: BLOCK },
  { at: box(7, 21.5, 15, 23.5), y: C, top: UNDER, key: BLOCK },
];

/** Ladders: `facing` is the way a ladder looks, away from what it climbs. */
export const LADDERS = [
  // in the sheds, up their shaft to the taller roof
  { at: [0.2, 3], facing: 0, y: G, h: R17 - G },
  // east of the hall, up to the catwalk's platform
  { at: [21.4, 10.9], facing: 180, y: G, h: CAT - G },
  // the two hatches in the hall's south end, down into the ducts
  { at: [6.9, -12.8], facing: 270, y: D, h: G - D },
  { at: [15.9, -12.8], facing: 270, y: D, h: G - D },
];

/** Crates: [east, north, side, turn in degrees, up]. Without `up` a crate stands on the floor. */
export const CRATES = [
  // the yard
  [-20.5, 3.5, 2.4, 10], [-23.2, -4.6, 1.6, 0], [-13.8, 4, 1.6, 0],
  // the sheds
  [-11, 14.8, 1.5, 0], [-2.2, 9.2, 1.6, 8], [-7.6, -9.6, 1.6, 0],
  // the hall, about its site
  [9.5, 1.5, 2.4, 0], [14.6, 8.8, 1.6, 0], [14.6, 8.8, 1.6, 12, G + 1.6], [16.4, 8.6, 1.6, 0], [8.2, -10.2, 1.6, 0], [17.4, -8.6, 2.4, 6],
  // the lower site
  [15.4, -6.9, 1.6, 0], [6.2, 7.6, 1.6, 0], [6.2, 5.9, 1.6, 10],
  // the lobby and the ramp room
  [3.2, 28.6, 2.4, 0], [9, 44.6, 1.6, 0], [10.7, 44.8, 1.2, 14],
  // the south yard, the south-east yard, the apron
  [3.4, -18.8, 2.4, 0], [24, -34.5, 2.4, 8], [42.6, -19.2, 2.4, 0], [45.2, -19.6, 1.6, 12], [39.5, 12.2, 2.4, 0],
];

/** A site's mark, of our own, on its floor: the upper site in the hall, the lower under it. */
export const MARKS = [
  { at: [12.5, 4.5], size: 5, y: G },
  { at: [11.5, -1.5], size: 5, y: B },
];

/** Lamps: [east, north, up]. */
export const LAMPS = [
  // the lower site, its catwalks, its tunnels
  [8, -4, 6.8], [14, -4, 6.8], [8, 8, 6.8], [15.5, 8, 6.8], [3.7, 13, 6.6], [18, 13, 6.6], [11, 16.3, 6.6],
  [-2, -1.5, 3.6], [-8, -1.5, 5.6], [22, -1.5, 3.6], [29, -1.5, 5.6],
  // the corridor under the lobby, the ramp
  [6, 20, 6.6], [16, 20, 6.6], [11, 25, 6.6], [10.75, 30, 7], [10.75, 36, 10.5], [4.5, 41.5, 11], [17.5, 41.5, 11], [11, 46, 11],
  // the west tunnel, the east room, its corridor, the east tunnel, the rooms at its end, the stair
  [-12.5, -8, 6.6], [-12.5, -18, 6.6], [-12.5, -28, 6.6],
  [23.5, 8.5, 6.6], [30.5, 8.5, 6.6], [34.5, 4, 6.6], [34.5, -8, 6.6], [34.5, -20, 6.6], [34.5, -30, 6.6], [40, -35, 6.6], [47, -35, 6.6], [54, -35, 6.6], [51.5, -28, 8],
  // the south room, its tunnels
  [8, -26, 6.8], [14, -26, 6.8], [11, -33, 6.8], [3.7, -40, 6.6], [18, -40, 6.6], [11, -46.5, 6.6], [-2, -29.5, 3.6], [-8, -29.5, 5.6], [22, -29.5, 3.6], [29, -29.5, 5.6],
  // the ducts
  [6.25, -15, 6.6], [15.25, -15, 6.6], [10.75, -17.5, 6.6],
  // the hall under its roof, the lobby, the sheds, the east room
  [5, 10, 17], [16, 10, 17], [5, -10, 17], [16, -10, 17],
  [0, 22, 14], [11, 22, 14], [22, 22, 14],
  [-9, 12, 14], [-2.5, 12, 14], [-6, 1, 14], [-5, -8.7, 11.6],
  [25.5, 13, 14], [25, 6.3, 14],
];
