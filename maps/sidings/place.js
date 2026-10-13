/**
 * SIDINGS, a place of Pocket Duty.
 *
 * Drawn by hand, floor by floor, in the place's own coordinates: metres east and north of its middle,
 * heights in metres above its lowest floor (maps/README.md). North is up in the plan below.
 *
 *      -50       -26  -14        0        +10      +24  +34      +52
 *  +50   +cut+
 *  +42 +-+---+-+
 *      | N-W   |      +------------- north street (3.45) ----------+
 *  +35 | START +------+                                  +--arch---+
 *      | (3.45)  corridor  shed  +----------------------+   | east |
 *  +27 +---------+--+-------+----+      north block      |   | lane |
 *                |st| connec-    |        (18.3)         |   |      |
 *  +20           |ai| tor        |                       |   |      |
 *                |r +=deck over==+--passage--+door+--bay-+pier+      +----+
 *  +11           |  |  core |    |  T1 ======[wagon]=========|gantry|=====|
 *                | upper    |    |                           | over |     |
 *    0           | hall     |    |  T2 ==[wagon]==[wagon]====| the  |=====|   the yard (3.45)
 *                | (9.55)   |    |        box                | yard | pads|   open site on T3
 *  -10           |          [wag]|  T3 =[wagon]=========[ flat wagon: SITE ]=|
 *  -12           +--+  +----+bay-+----+arch+-----------------+------+--+--+
 *                |la|stair       |     |court     south block (15.2)   |  | way down
 *  -21       west+--+            |     +-------+ramp                   |  | to the
 *  -25       walk|  ramp up  +---+--- platform (0.9) --+hump+----------+--+ S-E start
 *  -28      (5.4)|           |  Th1 ==[wagon]======[wagon]=====   hall  |        |
 *                |           |  Th2 ====[ flat wagon: SITE ]=[wagon]=   (0)   S-E START
 *  -44           |           |  Th3 =[flat wagon]==[wagon]=======       |  (0.9)  dock
 *  -46           +-----------+------- walk over the hall's side (6.5) --+ramp     |
 *  -50           +-------------------------------------------------------+--------+
 *
 * A wagon is a floor of the track with a roof 1.14 m over it. The roof is the wagon's body: it stops a
 * player and a bullet, one sees and shoots under it between its bogies, and the walk grid holds no way
 * under it (a standing soldier needs more than 1.15 m). The rails are low walls on the sleepers.
 */

/** The map's name, its turn in the world (which side the sun is on), its walls' height, the starts. */
export const PLACE = {
  id: 'sidings',
  title: 'Sidings',
  turn: 0,
  wall: 9.75,
  // [east, north, facing in degrees from north toward east, tag]. The first is the player's.
  // It looks down the yard between its first two tracks: the wagons, the gantry, the open site beyond.
  starts: [
    [-10.5, 5.2, 88, 'yard, west end'],
    [-44, 33, 110, 'north-west start'],
    [-38.6, 32.7, 100, 'north-west start'],
    [-34.5, 29.4, 95, 'north-west start'],
    [-39, 29.4, 100, 'north-west start'],
    [-18, 23.5, 180, 'connector'],
    [0, 36.5, 90, 'north street'],
    [33.2, 22, 180, 'east lane'],
    [20, 5.2, 270, 'yard'],
    [12.5, -19.8, 0, 'court'],
    [0, -26.4, 90, 'platform'],
    [-25.7, -36, 0, 'west walk'],
    [48.3, -13, 180, 'way down'],
    [41, -38, 300, 'south-east start'],
    [40, -33, 290, 'south-east start'],
    [38.6, -27, 270, 'south-east start'],
  ],
};

// The surfaces of the floors, the walls and the wagons (maps/sidings/look.js).
const DIRT = 'sd_dirt', BALLAST = 'sd_ballast', TRACK = 'sd_track', SLAB = 'sd_slab', TAR = 'sd_tar', PLANK = 'sd_plank';
const PALE = 'sd_brick', RED = 'sd_brick_red', SOOT = 'sd_brick_soot', WHITE = 'sd_brick_white', CAST = 'sd_cast', TIN = 'sd_tin';
const SAGE = 'sd_car_sage', OXIDE = 'sd_car_oxide', SLATE = 'sd_car_slate', OCHRE = 'sd_car_ochre', DECK = 'sd_car_deck';
const IRON = 'sd_iron', RAIL = 'sd_rail', IVY = 'sd_ivy';

// The heights most floors lie at: the yard, the platform, the west walk, its landing, the upper hall, the walk over the hall's side.
const Y = 3.45, PL = 0.9, WK = 5.4, LD = 6.1, DK = 9.55, GA = 6.5;
/** The room under a wagon: one sees through it, and the walk grid closes what is lower than 1.15 m. */
const CLEAR = 1.14;

/** A rectangle from its south-west corner to its north-east one. */
const box = (e0, n0, e1, n1) => [[e0, n0], [e1, n0], [e1, n1], [e0, n1]];
/** A wagon on a track: `base` is the track's floor, `top` the wagon's roof. */
const wagon = (e0, n0, e1, n1, base, top, key) => ({ at: box(e0, n0, e1, n1), y: base, key: TRACK, roof: CLEAR, walls: top - base, roofKey: key });

// What stands over the yard between east 24.1 and 33.5: the gantry, 10.8 m clear, its top at 15.24.
const GANTRY = { roof: 10.8, walls: 11.79, roofKey: CAST };
// The hall: its bed is the lowest floor, its roof's underside at 11.4, its top at 12.19. The roof is in
// two spans, and the strip between them is open over the second track.
const HALL = { roof: 11.4, walls: 12.19, roofKey: TIN, wall: SOOT };
const OPEN = { walls: 12.19, wall: SOOT };
// The upper hall in the west building: 3.3 m clear under the building's roof at 13.2.
const UPPER = { key: PLANK, roof: 3.3, walls: 3.65, wall: WHITE, roofKey: CAST };

/** The floors: polygons of [east, north], level (`y`) or sloped (`y: { from, to }`, each [east, north, up]). */
export const GROUND = [
  // ------------------------------------------------------------------ the north-west start, its cutting, the way east
  { at: box(-51.4, 27, -39.3, 42.5), y: Y, key: DIRT, walls: 7.55 },
  { at: box(-47.4, 42.5, -43.4, 49.5), y: Y, key: BALLAST, walls: 7.55 },
  { at: box(-39.3, 27, -26.7, 35.5), y: Y, key: DIRT },
  // the north street: its pilaster in the north wall, the shed's bay cut from its south side
  { at: [[-26.7, 27], [-13.8, 27], [-13.8, 31.9], [-10.5, 31.9], [-10.5, 27], [-8.8, 27], [-8.8, 31.9], [18, 31.9], [18, 34.7], [35.4, 34.7], [35.4, 39.5], [-6.8, 39.5], [-6.8, 38.8], [-10.4, 38.8], [-10.4, 39.5], [-23.1, 39.5], [-23.1, 34.7], [-26.7, 34.7]], y: Y, key: TAR, wall: RED },
  { at: box(-13.8, 27, -10.5, 31.9), y: Y, key: SLAB, roof: 2.75, walls: 3.05, roofKey: TIN, wall: RED },
  { at: box(2.6, 27, 6.5, 31.9), y: Y, key: TAR, wall: RED },
  // the east lane, under the building at its head
  { at: box(31, 31.8, 35.4, 34.7), y: Y, key: TAR, roof: 5.2, walls: 14.85, wall: RED, roofKey: RED },
  { at: box(31, 15.1, 35.4, 31.8), y: Y, key: TAR, wall: RED },

  // ------------------------------------------------------------------ the connector, the stair hall, the way under the upper hall
  { at: box(-21.5, 20, -14.2, 27), y: Y, key: TAR, wall: WHITE },
  { at: box(-26, 24.3, -21.5, 27), y: Y, key: SLAB, roof: 4.2, wall: WHITE, roofKey: CAST },
  { at: box(-26, 10.2, -23.4, 24.3), y: { from: [-24.7, 24.3, Y], to: [-24.7, 10.2, DK] }, steps: true, key: SLAB, roof: 3.4, walls: 3.65, wall: WHITE, roofKey: CAST },
  { at: box(-23, 12, -14.2, 20), y: Y, key: SLAB, wall: WHITE },
  { at: box(-14.2, 12, -1.2, 15.1), y: Y, key: SLAB, roof: 4.4, walls: 14.85, wall: RED, roofKey: RED },
  { at: box(-4.5, 11.4, -1.3, 12), y: Y, key: SLAB, roof: 3.6, walls: 14.85, wall: RED, roofKey: RED },

  // ------------------------------------------------------------------ the upper hall, round its core
  { at: box(-26, -16.5, -23.4, 10.2), y: DK, ...UPPER },
  { at: box(-23.4, 10.2, -14.2, 12), y: DK, ...UPPER },
  // (over the way from the connector to the yard: a floor over a floor; its roof is in MASSES)
  { at: box(-23.4, 12, -14.2, 20), y: DK, key: PLANK, over: true, thick: 0.4, side: WHITE, rail: 0 },
  { at: box(-18.2, -6.6, -14.2, 10.2), y: DK, ...UPPER },
  { at: box(-23.4, -11, -14.2, -6.6), y: DK, ...UPPER },
  { at: box(-18.2, -16, -14.2, -11), y: DK, ...UPPER },
  { at: box(-19, -19.5, -14.2, -16), y: DK, ...UPPER },
  // its stair down to the west walk's landing
  { at: box(-24.5, -19.5, -19, -16.5), y: { from: [-24.5, -18, LD], to: [-19, -18, DK] }, steps: true, key: SLAB, roof: 3.25, walls: 3.65, wall: WHITE, roofKey: CAST },
  { at: box(-28, -19.5, -24.5, -16.5), y: LD, key: SLAB, roof: 6.7, walls: 7.1, wall: WHITE, roofKey: CAST },
  { at: box(-28, -21, -24.5, -19.5), y: { from: [-26.25, -21, WK], to: [-26.25, -19.5, LD] }, key: SLAB, walls: 4.6, wall: WHITE },

  // ------------------------------------------------------------------ the west walk, its ramp up from the platform
  { at: box(-28, -50, -23.4, -21), y: WK, key: SLAB, walls: 4.6, wall: PALE },
  { at: box(-23.4, -50, -18.7, -46), y: WK, key: SLAB, walls: 4.6, wall: PALE },
  { at: box(-23.4, -28, -14.6, -24.8), y: { from: [-14.6, -26.4, PL], to: [-23.4, -26.4, WK] }, steps: true, key: SLAB, wall: PALE },

  // ------------------------------------------------------------------ the yard: south margin
  { at: box(-13.3, -11.5, 24.1, -9.5), y: Y },
  { at: box(24.1, -10.8, 33.5, -9.5), y: Y, key: SLAB, ...GANTRY },
  { at: box(33.5, -10.8, 51.7, -9.5), y: Y },
  // the third track: a wagon, and the flat wagon that is the open site
  { at: box(-13.3, -9.5, -10.6, -6.6), y: Y, key: TRACK },
  wagon(-10.6, -9.5, 5.2, -6.6, Y, 8.4, OXIDE),
  { at: box(5.2, -9.5, 24.1, -6.6), y: Y, key: TRACK },
  wagon(24.1, -9.5, 33.5, -6.6, Y, 4.9, DECK),
  wagon(33.5, -9.5, 41, -6.6, Y, 4.9, DECK),
  { at: box(41, -9.5, 51.7, -6.6), y: Y, key: TRACK },
  // between the third track and the second: ballast, the apron under the gantry, the wide pad east of it
  { at: box(-13.3, -6.6, 24.1, 0.3), y: Y },
  { at: box(24.1, -6.6, 33.5, 0.3), y: Y, key: SLAB, ...GANTRY },
  { at: box(33.5, -6.6, 51.7, -5.2), y: Y },
  { at: box(33.5, -5.2, 46.1, -1.1), y: Y, key: SLAB },
  { at: box(46.1, -5.2, 50.6, -1.1), y: Y },
  { at: box(33.5, -1.1, 51.7, 0.3), y: Y },
  // the second track: two wagons, the gap between them
  { at: box(-13.3, 0.3, -2.5, 3.2), y: Y, key: TRACK },
  wagon(-2.5, 0.3, 13.5, 3.2, Y, 8.4, SAGE),
  { at: box(13.5, 0.3, 17.2, 3.2), y: Y, key: TRACK },
  wagon(17.2, 0.3, 24.1, 3.2, Y, 8.4, SLATE),
  wagon(24.1, 0.3, 33.5, 3.2, Y, 8.4, SLATE),
  { at: box(33.5, 0.3, 51.7, 3.2), y: Y, key: TRACK },
  // between the second track and the first: ballast, the apron, the narrow pad
  { at: box(-13.3, 3.2, 24.1, 7.2), y: Y },
  { at: box(24.1, 3.2, 33.5, 7.2), y: Y, key: SLAB, ...GANTRY },
  { at: box(33.5, 3.2, 51.7, 4.6), y: Y },
  { at: box(33.5, 4.6, 46.1, 5.8), y: Y, key: SLAB },
  { at: box(46.1, 4.6, 50.6, 5.8), y: Y },
  { at: box(33.5, 5.8, 51.7, 7.2), y: Y },
  // the first track: one wagon
  { at: box(-13.3, 7.2, 6.4, 10.1), y: Y, key: TRACK },
  wagon(6.4, 7.2, 22.6, 10.1, Y, 8.4, OCHRE),
  { at: box(22.6, 7.2, 24.1, 10.1), y: Y, key: TRACK },
  { at: box(24.1, 7.2, 33.5, 10.1), y: Y, key: TRACK, ...GANTRY },
  { at: box(33.5, 7.2, 51.7, 10.1), y: Y, key: TRACK },
  // north margin, the bay, the foot of the east lane
  { at: box(-13.3, 10.1, 24.1, 11.4), y: Y, wall: RED },
  { at: box(24.1, 10.1, 33.5, 11.4), y: Y, key: SLAB, ...GANTRY, wall: RED },
  { at: box(33.5, 10.1, 51.7, 11.4), y: Y },
  { at: box(10.2, 11.4, 24.1, 15.1), y: Y, wall: RED },
  { at: box(28.9, 11.4, 33.5, 15.1), y: Y, key: SLAB, ...GANTRY, wall: RED },
  { at: [[33.5, 11.4], [37.1, 11.4], [37.1, 13.9], [35.4, 13.9], [35.4, 15.1], [33.5, 15.1]], y: Y, key: TAR },

  // ------------------------------------------------------------------ south of the yard: the bay, the arch, the court, the way down
  { at: box(-14.2, -15.7, -8.5, -11.5), y: Y, key: SLAB, roof: 4.2, walls: 11.79, wall: CAST, roofKey: CAST },
  { at: box(9.5, -15.5, 15.7, -11.5), y: Y, key: SLAB, roof: 4.4, walls: 11.79, wall: CAST, roofKey: CAST },
  { at: [[9.5, -21.5], [27.3, -21.5], [27.3, -18.2], [15.7, -18.2], [15.7, -15.5], [9.5, -15.5]], y: Y, key: SLAB, wall: CAST },
  { at: box(24, -24.8, 27.3, -21.5), y: { from: [25.65, -21.5, Y], to: [25.65, -24.8, 2.6] }, key: SLAB, wall: CAST },
  // the hump on the platform: a landing with a ramp down each way
  { at: box(23.7, -28, 27.7, -24.8), y: 2.6, key: SLAB, wall: CAST },
  { at: box(19.8, -28, 23.7, -24.8), y: { from: [19.8, -26.4, PL], to: [23.7, -26.4, 2.6] }, key: SLAB, wall: CAST },
  { at: box(27.7, -28, 31.6, -24.8), y: { from: [27.7, -26.4, 2.6], to: [31.6, -26.4, PL] }, key: SLAB, wall: CAST },
  // the platform along the hall's north side
  { at: box(-14.6, -28, 19.8, -24.8), y: PL, key: SLAB, wall: CAST },
  { at: box(31.6, -28, 36.3, -24.8), y: PL, key: SLAB, wall: CAST },

  // ------------------------------------------------------------------ the hall: its first track, by the platform, and three steep
  // stairs of boards from the platform down to the bed (the place has a drop of 0.9 m and no stair)
  { at: box(-13.4, -30.9, -12.5, -28), y: 0, key: TRACK, ...HALL },
  { at: box(-12.5, -28.9, -9.5, -28), y: { from: [-11, -28, PL], to: [-11, -28.9, 0] }, steps: true, key: PLANK, roof: 10.5, walls: 11.29, roofKey: TIN, side: PLANK },
  { at: box(-12.5, -30.9, -9.5, -28.9), y: 0, key: TRACK, ...HALL },
  { at: box(-9.5, -30.9, -6.4, -28), y: 0, key: TRACK, ...HALL },
  wagon(-6.4, -30.9, 9.6, -28, 0, 4.9, SLATE),
  { at: box(9.6, -30.9, 11, -28), y: 0, key: TRACK, ...HALL },
  { at: box(11, -28.9, 15, -28), y: { from: [13, -28, PL], to: [13, -28.9, 0] }, steps: true, key: PLANK, roof: 10.5, walls: 11.29, roofKey: TIN, side: PLANK },
  { at: box(11, -30.9, 15, -28.9), y: 0, key: TRACK, ...HALL },
  { at: box(15, -30.9, 16.9, -28), y: 0, key: TRACK, ...HALL },
  wagon(16.9, -30.9, 33, -28, 0, 4.9, OXIDE),
  { at: box(33, -28.9, 35.4, -28), y: { from: [34.2, -28, PL], to: [34.2, -28.9, 0] }, steps: true, key: PLANK, roof: 10.5, walls: 11.29, roofKey: TIN, side: PLANK },
  { at: box(33, -30.9, 35.4, -28.9), y: 0, key: TRACK, ...HALL },
  { at: box(35.4, -30.9, 36.3, -28.4), y: 0, key: TRACK, roof: 4.3, walls: 12.19, wall: SOOT, roofKey: SOOT },
  // between the first track and the second: under the roof, then under its open strip
  { at: box(-13.4, -34, 35.4, -30.9), y: 0, ...HALL },
  { at: box(-13.4, -35.3, 35.4, -34), y: 0, ...OPEN },
  // the second track, under the open strip: the flat wagon that is the hall's site, and a wagon
  { at: box(-13.4, -38.2, -2.5, -35.3), y: 0, key: TRACK, ...OPEN },
  wagon(-2.5, -38.2, 13.5, -35.3, 0, 1.42, DECK),
  { at: box(13.5, -38.2, 15, -35.3), y: 0, key: TRACK, ...OPEN },
  wagon(15, -38.2, 31.2, -35.3, 0, 4.9, SAGE),
  { at: box(31.2, -38.2, 35.4, -35.3), y: 0, key: TRACK, ...OPEN },
  { at: box(35.4, -38.2, 36.3, -35.3), y: 0, key: TRACK, roof: 4.3, walls: 12.19, wall: SOOT, roofKey: SOOT },
  { at: box(-13.4, -39, 35.4, -38.2), y: 0, ...OPEN },
  { at: box(-13.4, -41.6, 35.4, -39), y: 0, ...HALL },
  // the third track: a flat wagon and a wagon
  { at: box(-13.4, -44.5, -10.5, -41.6), y: 0, key: TRACK, ...HALL },
  wagon(-10.5, -44.5, 5.6, -41.6, 0, 1.42, DECK),
  { at: box(5.6, -44.5, 7.2, -41.6), y: 0, key: TRACK, ...HALL },
  wagon(7.2, -44.5, 23.3, -41.6, 0, 4.9, OCHRE),
  { at: box(23.3, -44.5, 35.4, -41.6), y: 0, key: TRACK, ...HALL },
  { at: box(35.4, -44.5, 36.3, -41.6), y: 0, key: TRACK, roof: 4.3, walls: 12.19, wall: SOOT, roofKey: SOOT },
  { at: box(-13.4, -45.5, 35.4, -44.5), y: 0, ...HALL },

  // ------------------------------------------------------------------ the walk along the hall's south side, a steep stair at each end
  { at: box(-18.7, -49.6, -17.5, -46), y: { from: [-18.7, -47.8, WK], to: [-17.5, -47.8, GA] }, steps: true, key: SLAB, walls: 5.7, wall: SOOT },
  { at: box(-17.5, -49.6, 19.5, -45.5), y: GA, key: SLAB, walls: 5.7, wall: SOOT, side: SOOT },
  { at: box(19.5, -49.6, 24.9, -45.5), y: { from: [19.5, -47.5, GA], to: [24.9, -47.5, 4.1] }, steps: true, key: SLAB, walls: 8.1, wall: SOOT, side: SOOT },
  { at: box(24.9, -49.6, 26.9, -45.5), y: 4.1, key: SLAB, walls: 8.1, wall: SOOT, side: SOOT },
  { at: box(26.9, -49.6, 28.5, -45.5), y: { from: [26.9, -47.5, 4.1], to: [28.5, -47.5, 2.6] }, steps: true, key: SLAB, walls: 8.1, wall: SOOT, side: SOOT },
  // its foot: the ramp up from the south-east start
  { at: box(28.5, -49.6, 32.3, -45.5), y: { from: [28.5, -47.5, 2.6], to: [32.3, -47.5, PL] }, key: SLAB, walls: 7, wall: PALE, side: SOOT },
  { at: box(32.3, -49.6, 36.3, -45.5), y: PL, key: TAR, walls: 7, wall: PALE },

  // ------------------------------------------------------------------ the south-east start, its dock, the way up to the yard
  { at: [[36.3, -49.6], [51.7, -49.6], [51.7, -41.8], [45.2, -41.8], [45.2, -31.2], [51.7, -31.2], [51.7, -24.8], [44.8, -24.8], [44.8, -28.4], [42.1, -28.4], [42.1, -24.8], [36.3, -24.8]], y: PL, key: TAR, walls: 7, wall: PALE },
  { at: [[45.2, -41.8], [51.7, -41.8], [51.7, -31.2], [47.6, -31.2], [47.6, -34], [45.2, -34]], y: 2, key: SLAB, walls: 5.9, wall: PALE, side: CAST },
  { at: box(45.2, -34, 47.6, -31.2), y: { from: [45.2, -32.6, PL], to: [47.6, -32.6, 2] }, key: SLAB, walls: 7, side: CAST },
  { at: box(46.1, -24.8, 50.6, -22), y: PL, key: TAR, wall: CAST },
  { at: box(46.1, -22, 50.6, -16), y: { from: [48.35, -22, PL], to: [48.35, -16, Y] }, key: TAR, wall: CAST },
  { at: box(46.1, -16, 50.6, -10.8), y: Y, key: TAR, wall: CAST },
];

/** The tops of the blocks beside the floors, where they are not `PLACE.wall` over the floor. A wall rises to its block. */
export const BLOCKS = [
  // the north block between the street and the yard, with the gantry's pier; the block east of the lane
  { at: box(-14.2, 11.4, 31, 34.7), top: 18.3 },
  { at: box(35.4, 11.4, 54, 41), top: 16 },
  // south of the north-west start
  { at: box(-53, 24, -26.7, 27), top: 13.2 },
  // the south block between the yard and the hall; east of the way down
  { at: box(-14.2, -24.8, 46.1, -11.5), top: 15.24 },
  { at: box(50.6, -24.8, 54, -10.8), top: 15.24 },
  // the hall's ends
  { at: box(-23.4, -46, -13.4, -28), top: 12.19 },
  { at: box(35.4, -45.5, 36.3, -28), top: 12.19 },
];

/** A bogie's two blocks under a wagon from `e0` to `e1` on the track whose middle is `n`. */
const bogies = (e0, e1, n, base) => [
  { at: box(e0 + 0.6, n - 1.1, e0 + 3.8, n + 1.1), y: base, top: base + CLEAR, key: IRON },
  { at: box(e1 - 3.8, n - 1.1, e1 - 0.6, n + 1.1), y: base, top: base + CLEAR, key: IRON },
];

/**
 * What a wagon's two sides carry besides their ribs: a sill under the body, a door with its runner over
 * it, and a walk along the roof. `e0`…`e1` and `n0`…`n1` are the wagon's, `key` its paint.
 */
const fittings = (e0, n0, e1, n1, base, top, key) => {
  const sill = base + CLEAR, mid = (e0 + e1) / 2, out = [];
  for (const [a, b, c, d] of [[n0 - 0.07, n0, n0 - 0.09, n0 - 0.11], [n1, n1 + 0.07, n1 + 0.09, n1 + 0.11]]) {
    out.push({ at: box(e0, a, e1, b), y: sill, top: sill + 0.3, key: IRON });
    out.push({ at: box(mid - 1.6, Math.min(b, c), mid + 1.6, Math.max(a, c)), y: sill + 0.3, top: top - 0.55, key });
    out.push({ at: box(mid - 3.4, Math.min(b, d), mid + 1.8, Math.max(a, d)), y: top - 0.55, top: top - 0.45, key: IRON });
  }
  out.push({ at: box(e0 + 0.4, (n0 + n1) / 2 - 0.3, e1 - 0.4, (n0 + n1) / 2 + 0.3), y: top, top: top + 0.1, key: IRON });
  return out;
};

/** Solids of any footprint, from `y` to `top`. */
export const MASSES = [
  // the gantry over the wagons under it, and the cabin on its north end over the pier
  { at: box(24.1, 0.3, 33.5, 3.2), y: 14.25, top: 15.24, key: CAST },
  { at: box(24.1, -9.5, 33.5, -6.6), y: 14.25, top: 15.24, key: CAST },
  { at: box(24.1, 7.8, 28.9, 11.4), y: 15.24, top: 18.5, key: RED },
  // the roof over the upper hall's floor that lies over the way to the yard
  { at: box(-23.4, 12, -14.2, 20), y: 12.85, top: 13.2, key: CAST },
  // the hall's roof over the wagons under it
  { at: box(-6.4, -30.9, 9.6, -28), y: 11.4, top: 12.19, key: TIN },
  { at: box(16.9, -30.9, 33, -28), y: 11.4, top: 12.19, key: TIN },
  { at: box(-10.5, -44.5, 5.6, -41.6), y: 11.4, top: 12.19, key: TIN },
  { at: box(7.2, -44.5, 23.3, -41.6), y: 11.4, top: 12.19, key: TIN },
  // the hall's six pillars
  { at: box(-8.75, -33.75, -7.85, -32.85), y: 0, top: 11.4, key: CAST },
  { at: box(10.45, -33.75, 11.35, -32.85), y: 0, top: 11.4, key: CAST },
  { at: box(27.55, -33.75, 28.45, -32.85), y: 0, top: 11.4, key: CAST },
  { at: box(-8.75, -40.15, -7.85, -39.25), y: 0, top: 11.4, key: CAST },
  { at: box(10.45, -40.15, 11.35, -39.25), y: 0, top: 11.4, key: CAST },
  { at: box(27.55, -40.15, 28.45, -39.25), y: 0, top: 11.4, key: CAST },
  // the posts that carry the hall's roof along the walk
  { at: box(-8.6, -45.5, -8, -44.9), y: 0, top: 11.4, key: CAST },
  { at: box(1, -45.5, 1.6, -44.9), y: 0, top: 11.4, key: CAST },
  { at: box(10.6, -45.5, 11.2, -44.9), y: 0, top: 11.4, key: CAST },
  { at: box(20.2, -45.5, 20.8, -44.9), y: 0, top: 11.4, key: CAST },
  { at: box(29.8, -45.5, 30.4, -44.9), y: 0, top: 11.4, key: CAST },
  // the wagons' bogies: the yard's five wagons and its flat wagon, the hall's four and its two flat ones
  ...bogies(-10.6, 5.2, -8.05, Y),
  ...bogies(24.1, 41, -8.05, Y),
  ...bogies(-2.5, 13.5, 1.75, Y),
  ...bogies(17.2, 33.5, 1.75, Y),
  ...bogies(6.4, 22.6, 8.65, Y),
  ...bogies(-6.4, 9.6, -29.45, 0),
  ...bogies(16.9, 33, -29.45, 0),
  ...bogies(-2.5, 13.5, -36.75, 0),
  ...bogies(15, 31.2, -36.75, 0),
  ...bogies(-10.5, 5.6, -43.05, 0),
  ...bogies(7.2, 23.3, -43.05, 0),
  // their sills, doors and roof walks
  ...fittings(-10.6, -9.5, 5.2, -6.6, Y, 8.4, OXIDE),
  ...fittings(-2.5, 0.3, 13.5, 3.2, Y, 8.4, SAGE),
  ...fittings(17.2, 0.3, 33.5, 3.2, Y, 8.4, SLATE),
  ...fittings(6.4, 7.2, 22.6, 10.1, Y, 8.4, OCHRE),
  ...fittings(-6.4, -30.9, 9.6, -28, 0, 4.9, SLATE),
  ...fittings(16.9, -30.9, 33, -28, 0, 4.9, OXIDE),
  ...fittings(15, -38.2, 31.2, -35.3, 0, 4.9, SAGE),
  ...fittings(7.2, -44.5, 23.3, -41.6, 0, 4.9, OCHRE),
  // the wagon that stands in the west building's mouth, on the third track
  { at: box(-14.6, -9.45, -12.5, -6.65), y: Y, top: 8.4, key: SAGE },
  // a wagon's end at the head of the north-west cutting
  { at: box(-46.9, 47.9, -43.9, 49.5), y: Y + 0.9, top: Y + 4.7, key: OXIDE },
  { at: box(-46.6, 48.1, -44.2, 49.5), y: Y, top: Y + 0.9, key: IRON },
  // ivy on the east lane's east wall, in three hangs
  { at: box(35.26, 16.5, 35.4, 22.5), y: Y, top: Y + 7.4, key: IVY },
  { at: box(35.3, 22.5, 35.4, 27.5), y: Y + 1.7, top: Y + 5.6, key: IVY },
  { at: box(35.24, 27, 35.4, 31), y: Y, top: Y + 8.6, key: IVY },
  // the arms the hall's lamps hang from, on its pillars
  { at: box(-8.4, -34.6, -8.2, -33.75), y: 5.3, top: 5.42, key: IRON },
  { at: box(10.8, -34.6, 11, -33.75), y: 5.3, top: 5.42, key: IRON },
  { at: box(27.9, -34.6, 28.1, -33.75), y: 5.3, top: 5.42, key: IRON },
  { at: box(-8.4, -39.25, -8.2, -38.4), y: 5.3, top: 5.42, key: IRON },
  { at: box(10.8, -39.25, 11, -38.4), y: 5.3, top: 5.42, key: IRON },
  { at: box(27.9, -39.25, 28.1, -38.4), y: 5.3, top: 5.42, key: IRON },
  // the cabinet between the tracks
  { at: box(-2.4, -4.4, -1.2, -1.9), y: Y, top: 6.9, key: SLATE },
  // what the two sites are about: a long case on each flat wagon
  { at: box(29.4, -8.6, 35.8, -7.4), y: 4.99, top: 5.95, key: 'sd_wood' },
  { at: box(3, -37.35, 9.7, -36.15), y: 1.54, top: 2.45, key: 'sd_wood' },
];

/** A track's two rails from `e0` to `e1`, 0.75 m each side of its middle `n`. */
const rails = (e0, e1, n, base) => [
  { line: [[e0, n - 0.75], [e1, n - 0.75]], y: base, h: 0.14, t: 0.09, key: RAIL, crown: false },
  { line: [[e0, n + 0.75], [e1, n + 0.75]], y: base, h: 0.14, t: 0.09, key: RAIL, crown: false },
];

/** Walls along a line, with what is cut in them: `holes` of `{ at, w, h, sill, arch }`, `at` in metres from the first point. */
export const WALLS = [
  // the rails: the yard's three tracks, the hall's three
  ...rails(-13.3, 51.7, 8.65, Y),
  ...rails(-13.3, 51.7, 1.75, Y),
  ...rails(-13.3, 51.7, -8.05, Y),
  ...rails(-13.4, 36.3, -29.45, 0),
  ...rails(-13.4, 36.3, -36.75, 0),
  ...rails(-13.4, 36.3, -43.05, 0),
  // the rails of the track that leaves the north-west start by its cutting
  { line: [[-46.15, 31], [-46.15, 47.9]], y: Y, h: 0.14, t: 0.09, key: RAIL, crown: false },
  { line: [[-44.65, 31], [-44.65, 47.9]], y: Y, h: 0.14, t: 0.09, key: RAIL, crown: false },
  // the portal at the mouth of the north-west cutting
  { line: [[-47.4, 43.2], [-43.4, 43.2]], y: Y, h: 4.6, t: 0.7, key: CAST, holes: [{ at: 2, w: 3.2, h: 3.7 }] },
  // the upper hall's walls round its floor over the way to the yard: two glazed windows to the connector.
  // (`low`: they close nothing on the walk grid, which holds the way under them; and each stands between
  // two rows of the grid's nodes, so that no node's ray starts in its foot)
  { line: [[-23.4, 19.5], [-14.2, 19.5]], y: DK, h: 3.3, t: 0.4, low: 0, key: WHITE, crown: false, holes: [{ at: 2.6, w: 1.7, h: 1.3, sill: 1, frame: true, glass: true }, { at: 6.6, w: 1.7, h: 1.3, sill: 1, frame: true, glass: true }] },
  { line: [[-14, 15.1], [-14, 19.7]], y: DK, h: 3.3, t: 0.4, low: 0, key: WHITE, crown: false },
  { line: [[-23.55, 12], [-23.55, 19.7]], y: DK, h: 3.3, t: 0.4, low: 0, key: WHITE, crown: false },
  // the end of the upper hall's west arm, over the landing
  { line: [[-26, -16.5], [-23.4, -16.5]], y: DK, h: 3.3, t: 0.4, key: WHITE, crown: false },
  // the hall's north-east corner runs on into the south-east start; the start's south wall
  { line: [[36.3, -28.2], [39.4, -28.2]], y: PL, h: 3.6, t: 0.5, key: PALE },
  { line: [[36.3, -45.5], [47.6, -45.5]], y: PL, h: 3.2, t: 0.5, key: PALE },
  // the dock's walls
  { line: [[45.2, -31.2], [51.7, -31.2]], y: PL, h: 4.3, t: 0.4, key: PALE },
  { line: [[45.2, -41.8], [51.7, -41.8]], y: PL, h: 4.3, t: 0.4, key: PALE },
  { line: [[45.2, -41.8], [45.2, -34]], y: PL, h: 4.3, t: 0.4, key: PALE, holes: [{ at: 3.9, w: 2.6, h: 1.3, sill: 2.2 }] },
  // the rail of the walk along the hall's side, and the parapet down its slope
  { line: [[-13.4, -45.62], [19.5, -45.62]], y: GA, h: 0.95, t: 0.16, key: CAST, crown: false },
  { line: [[19.5, -45.62], [24.9, -45.62]], y: 4.1, h: 3.35, t: 0.16, key: CAST, crown: false },
  { line: [[24.9, -45.62], [28.5, -45.62]], y: 2.6, h: 2.45, t: 0.16, key: CAST, crown: false },
];

/** Crates: [east, north, side, turn in degrees, up]. Without `up` a crate stands on the floor. */
export const CRATES = [
  // the north-west start: three, and one on two of them
  [-41.3, 39.4, 1.6, 30], [-40.8, 37.9, 1.75, 15], [-41.1, 36.4, 1.7, 0], [-40.9, 37.2, 1.5, 22, Y + 1.75],
  // by the shed, in the alcove, on the north street, in the east lane and at its foot
  [-9.65, 28.2, 1.5, 0], [3.7, 28.8, 1.2, 0], [22.95, 35.5, 1.6, 0], [34.55, 26.1, 1.6, 0],
  [31.9, 16.1, 1.75, 0], [31.9, 16.1, 1.6, 10, Y + 1.75], [31.8, 14.4, 1.5, 0],
  // the bay south of the yard, the arch, the court
  [-9.4, -12.4, 1.6, 0], [10.35, -12.5, 1.6, 0], [14.9, -16.9, 1.5, 0],
  // the upper hall
  [-15.9, 17.7, 1.5, 0], [-17.3, -4.2, 1.6, 0], [-20.7, -8.6, 1.6, 0],
  // the west walk
  [-24.4, -36.5, 1.5, 0], [-27.1, -42.5, 1.6, 0],
  // the south-east start and its dock
  [37.2, -39.7, 1.7, 0], [46.4, -40.2, 1.6, 0],
];

/** The two sites' marks on the ground beside their flat wagons. */
export const MARKS = [
  { at: [36.5, -3.2], size: 3.2 },
  { at: [6.4, -32.6], size: 2.6 },
];

/** Lamps: [east, north, up]; without `up`, under the roof of the floor it hangs over. */
export const LAMPS = [
  // the upper hall and its stairs
  [-24.7, 17], [-24.7, 3], [-24.7, -10], [-16.2, 6], [-16.2, -3], [-19, -8.8], [-16.2, -13.5], [-21.5, -18],
  // the passage to the yard, the arch, the bay, the lane's head, the stair hall, the shed
  [-8, 13.5], [12.6, -13.5], [-11.3, -13.6], [33.2, 33.2], [-23.7, 25.6], [-12.1, 29.5],
  // the hall: a lamp on each pillar's arm, toward the middle track
  [-8.3, -34.45, 5.3], [10.9, -34.45, 5.3], [28, -34.45, 5.3], [-8.3, -38.55, 5.3], [10.9, -38.55, 5.3], [28, -38.55, 5.3],
  // the hall's track mouths
  [35.85, -29.6], [35.85, -36.75], [35.85, -43.05],
];

/** The reference's props: [id, east, north, turn, scale]. They stop no one. */
export const PROPS = [
  // lamp posts: the start's corner, the street, the connector, the lane, the court, the way down, the west walk
  ['lamp_post', -39.9, 35, 180, 1], ['lamp_post', -8.4, 32.5, 90, 1], ['lamp_post', 8.3, 39, 180, 1], ['lamp_post', -14.8, 25.6, 270, 1],
  ['lamp_post', 34.9, 24.2, 270, 1], ['lamp_post', 12.5, -21, 0, 1], ['lamp_post', 25.5, -18.7, 180, 1], ['lamp_post', 50.1, -19, 270, 1],
  ['lamp_post', -23.9, -37, 270, 1], ['lamp_post', -23.9, -44, 270, 1], ['lamp_post', 38, -25.3, 180, 1],
  // weeds between the sleepers and along the walls of the yard
  ['weeds', -11.5, 8.4, 20, 1.1], ['weeds', -6.2, 9.2, 140, 0.9], ['weeds', 1.5, 8.1, 60, 1.2], ['weeds', 27, 9, 200, 1], ['weeds', 38.4, 8.3, 310, 1.2], ['weeds', 46, 9.1, 80, 0.9],
  ['weeds', -8.8, 1.4, 250, 1], ['weeds', 15.2, 2.2, 30, 1.3], ['weeds', 15.9, 1.1, 170, 0.9], ['weeds', 38, 1.6, 100, 1.1], ['weeds', 47.5, 2.3, 290, 1],
  ['weeds', -12.2, -7.4, 15, 1.2], ['weeds', 9.4, -8.6, 220, 1], ['weeds', 17.3, -7.7, 75, 1.1], ['weeds', 44.5, -8.2, 330, 1.2], ['weeds', 49.6, -7.5, 130, 0.9],
  ['weeds', -12.7, 10.9, 0, 1.3], ['weeds', 4.2, 11, 95, 1], ['weeds', 21.5, -11.1, 265, 1.2], ['weeds', 44, 10.9, 45, 1.1], ['weeds', 51.2, -8.9, 190, 1.2], ['weeds', -12.8, -3, 300, 1],
  // weeds in the north-west start and its cutting, and at the hall's west end
  ['weeds', -45.4, 44.8, 40, 1.2], ['weeds', -46.5, 40.6, 160, 1], ['weeds', -50.8, 41.8, 280, 1.3], ['weeds', -50.6, 27.8, 70, 1.1], ['weeds', -44.2, 46.5, 210, 0.9],
  ['weeds', -12.6, -36.8, 110, 1.2], ['weeds', -11.4, -43.2, 240, 1], ['weeds', -12.9, -40.2, 10, 1.1], ['weeds', 34.2, -36.4, 320, 1],
  // pallets by the shed, in the bay, on the platform, at the dock
  ['pallet', -11.6, 30.8, 8, 1], ['pallet', -11.6, 30.8, 20, 1, Y + 0.14], ['pallet', -12.6, -14.6, 80, 1], ['pallet', 3.4, -25.6, 12, 1], ['pallet', 3.4, -25.6, 2, 1, PL + 0.14], ['pallet', 49.6, -39.8, 30, 1],
];

/** Ladders, drawn: `facing` is the way a ladder looks, away from what it climbs. Nobody climbs one yet. */
export const LADDERS = [
  // a ladder at an end of each wagon's side: the yard's four, the hall's four
  { at: [4.4, -9.62], facing: 180, y: Y + CLEAR, h: 3.8 },
  { at: [-1.7, 3.32], facing: 0, y: Y + CLEAR, h: 3.8 },
  { at: [18, 0.18], facing: 180, y: Y + CLEAR, h: 3.8 },
  { at: [21.8, 7.08], facing: 180, y: Y + CLEAR, h: 3.8 },
  { at: [8.8, -31.02], facing: 180, y: CLEAR, h: 3.75 },
  { at: [17.7, -31.02], facing: 180, y: CLEAR, h: 3.75 },
  { at: [30.4, -35.18], facing: 0, y: CLEAR, h: 3.75 },
  { at: [8, -41.48], facing: 0, y: CLEAR, h: 3.75 },
];
