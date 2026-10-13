/**
 * SIDINGS: its look. A rail yard among brick blocks under a pale morning: painted brick in common bond,
 * kiln brick, board-marked concrete, pressed steel wagon sides in four paints, sleepers in crushed
 * stone, dark tar, a tin roof over the hall.
 *
 * The surfaces below are this map's own: each is a function that draws its tile, and nothing is read
 * from a picture. `LOOK.materials` names the map's palette keys over them, over the kit's
 * (web/src/pocket/placelook.js) and over the reference's library.
 */
import { surface } from '../../web/src/pocket/placelook.js';

/**
 * Brick in common bond: five courses of stretchers, then one of headers, 24 courses and 8 stretchers to
 * a tile of 2.4 m. `face` is the brick's colour between two tones, `joint` its mortar, `bare` what shows
 * where the face has flaked (the share of it by `flake`).
 */
const brick = ({ face, joint, bare, flake }) => `
  vec2 p = uv * P + uSeed * 3.1;
  float row = floor(uv.y * 24.0);
  float fy = fract(uv.y * 24.0);
  float header = step(4.5, mod(row, 6.0));
  float n = mix(8.0, 16.0, header);
  float xf = uv.x * n + 0.5 * mod(row, 2.0) + 0.25 * header;
  float fx = fract(xf);
  vec2 id = vec2(mod(floor(xf), n), row);
  vec4 rnd = owHash42(id * 1.7 + 3.0 + uSeed);
  float dx = min(fx, 1.0 - fx) / n;
  float dy = min(fy, 1.0 - fy) / 24.0;
  float mortar = min(smoothstep(0.0012, 0.0042, dx), smoothstep(0.0012, 0.0042, dy));
  float grain = owFbm01(p * 16.0, P * 16.0, 4, 0.55);
  float cloud = owFbm01(p * 1.5, P * 1.5, 3, 0.55);
  float worn = smoothstep(${(0.9 - flake).toFixed(2)}, ${(1.04 - flake).toFixed(2)}, owFbm01(p * 4.0 + rnd.xy * 2.0, P * 4.0, 3, 0.5) + (rnd.z - 0.5) * 0.3);
  h = mortar * (0.62 + (rnd.x - 0.5) * 0.14 + (grain - 0.5) * 0.05 - worn * 0.05);
  vec3 c = mix(owSRGB(vec3(${face[0]})), owSRGB(vec3(${face[1]})), cloud * 0.6 + rnd.y * 0.4);
  c = mix(c, owSRGB(vec3(${bare})) * (0.8 + 0.4 * rnd.w), worn);
  c *= 0.93 + 0.14 * grain;
  alb = mix(owSRGB(vec3(${joint})) * (0.85 + 0.3 * grain), c, mortar);
  rough = 0.93;
  metal = 0.0;
  ao = 0.72 + 0.28 * mortar;`;

/** Concrete cast in lifts of 0.6 m: the boards' seams, pits, and what the rain has run down it. */
const CAST = `
  vec2 p = uv * P + uSeed * 2.3;
  float lift = fract(uv.y * 4.0);
  float seam = smoothstep(0.0, 0.014, min(lift, 1.0 - lift));
  float board = owFbm01(vec2(p.x * 10.0, p.y * 1.5), vec2(P.x * 10.0, P.y * 1.5), 3, 0.5);
  float grain = owFbm01(p * 12.0, P * 12.0, 4, 0.55);
  float cloud = owFbm01(p * 1.25, P * 1.25, 3, 0.55);
  float run = owFbm01(vec2(p.x * 6.0, p.y * 0.5), vec2(P.x * 6.0, P.y * 0.5), 3, 0.5);
  vec4 pit = owWorley(p * 10.0, P * 10.0, 1.0);
  float pits = smoothstep(0.09, 0.02, pit.x) * step(0.78, pit.z);
  h = 0.12 + 0.5 * seam + (board - 0.5) * 0.06 + (grain - 0.5) * 0.04 - pits * 0.1;
  vec3 c = mix(owSRGB(vec3(0.47, 0.48, 0.45)), owSRGB(vec3(0.64, 0.65, 0.61)), cloud);
  c = mix(c, c * 0.7, smoothstep(0.52, 0.8, run) * 0.65);
  c *= 0.93 + 0.14 * grain;
  alb = mix(c, owSRGB(vec3(0.27, 0.27, 0.25)), (1.0 - seam) * 0.5 + pits * 0.55);
  rough = 0.9;
  metal = 0.0;
  ao = 0.8 + 0.2 * seam;`;

/**
 * A wagon's side of pressed steel: a rib down each panel of 0.4 m, a seam across at half height, paint
 * over rust. The paint is the tile's own colour (`paint`, two tones): a device draws a tile times a
 * colour a vertex that stops at one, so in the sun a surface shows its tile and a tint is lost.
 */
const car = (paint) => `
  vec2 p = uv * P + uSeed * 6.1;
  float x = fract(uv.x * 8.0);
  float rib = smoothstep(0.11, 0.03, abs(x - 0.5));
  float seamV = smoothstep(0.0, 0.03, min(x, 1.0 - x));
  float y2 = fract(uv.y * 2.0);
  float seamH = smoothstep(0.0, 0.012, min(y2, 1.0 - y2));
  float grain = owFbm01(p * 14.0, P * 14.0, 4, 0.55);
  float cloud = owFbm01(p * 1.5, P * 1.5, 3, 0.55);
  float run = owFbm01(vec2(p.x * 8.0, p.y * 0.75), vec2(P.x * 8.0, P.y * 0.75), 3, 0.5);
  float rust = smoothstep(0.56, 0.86, run * 0.6 + cloud * 0.4 + (1.0 - seamV) * 0.3 + (1.0 - seamH) * 0.3);
  vec4 chip = owWorley(p * 7.0, P * 7.0, 1.0);
  float chips = smoothstep(0.10, 0.03, chip.x) * step(0.74, chip.z);
  h = 0.42 + 0.42 * rib - 0.26 * (1.0 - seamV) - 0.2 * (1.0 - seamH) + (grain - 0.5) * 0.03;
  vec3 paint = mix(owSRGB(vec3(${paint[0]})), owSRGB(vec3(${paint[1]})), cloud) * (0.95 + 0.1 * grain);
  vec3 oxide = mix(owSRGB(vec3(0.42, 0.25, 0.17)), owSRGB(vec3(0.60, 0.38, 0.24)), grain);
  vec3 c = mix(paint, oxide, max(rust * 0.7, chips));
  alb = c * (0.78 + 0.22 * seamV * seamH);
  rough = mix(0.55, 0.9, rust);
  metal = 0.0;
  ao = 0.8 + 0.2 * seamV * seamH;`;

/** Dark plate with rivets along two lines and rust in flecks: bogies, sills, ladders, runners; and, paler, the rails. */
const plate = (tone) => `
  vec2 p = uv * P + uSeed * 9.1;
  float grain = owFbm01(p * 14.0, P * 14.0, 4, 0.55);
  float cloud = owFbm01(p * 1.5, P * 1.5, 3, 0.55);
  vec2 q = fract(uv * vec2(8.0, 2.0)) - 0.5;
  float rivet = smoothstep(0.16, 0.08, length(q * vec2(1.0, 4.0)));
  vec4 fleck = owWorley(p * 6.0, P * 6.0, 1.0);
  float rust = smoothstep(0.5, 0.8, cloud * 0.6 + fleck.z * 0.4) * 0.6;
  h = 0.45 + 0.3 * rivet + (grain - 0.5) * 0.05 - rust * 0.05;
  vec3 c = mix(owSRGB(vec3(${tone[0]})), owSRGB(vec3(${tone[1]})), cloud) * (0.92 + 0.16 * grain);
  alb = mix(c, owSRGB(vec3(${tone[2]})) * (0.8 + 0.4 * grain), rust) * (1.0 + 0.12 * rivet);
  rough = mix(0.6, 0.9, rust);
  metal = 0.0;
  ao = 0.85 + 0.15 * rivet;`;

/** A flat wagon's deck: boards across it, bolted, worn grey. */
const DECK = `
  vec2 p = uv * P + uSeed * 1.9;
  float x = uv.x * 12.0;
  float fx = fract(x);
  float id = mod(floor(x), 12.0);
  float gap = smoothstep(0.0, 0.05, min(fx, 1.0 - fx));
  float fibre = owFbm01(vec2(p.x * 24.0, p.y * 2.0) + id * 3.7, vec2(P.x * 24.0, P.y * 2.0), 3, 0.5);
  float tone = owHash11(id * 5.3 + uSeed);
  h = gap * (0.55 + (tone - 0.5) * 0.1 + (fibre - 0.5) * 0.08);
  vec3 c = mix(owSRGB(vec3(0.33, 0.30, 0.27)), owSRGB(vec3(0.50, 0.46, 0.41)), fibre * 0.6 + tone * 0.4);
  alb = mix(owSRGB(vec3(0.10, 0.09, 0.08)), c, gap);
  rough = 0.9;
  metal = 0.0;
  ao = 0.6 + 0.4 * gap;`;

/** Crushed stone: sharp pieces of three greys, dark between them. */
const STONE = `
  vec4 cell = owWorley(p * 5.0, P * 5.0, 0.95);
  float edge = owVoronoiEdge(p * 5.0, P * 5.0, 0.95);
  vec4 fine = owWorley(p * 11.0, P * 11.0, 1.0);
  float piece = smoothstep(0.02, 0.10, edge);
  float low = owFbm01(p * 0.75, P * 0.75, 3, 0.55);
  vec3 rock = mix(owSRGB(vec3(0.25, 0.245, 0.24)), owSRGB(vec3(0.42, 0.405, 0.385)), cell.z);
  rock = mix(rock, owSRGB(vec3(0.36, 0.30, 0.25)), step(0.72, cell.w) * 0.7);
  rock *= (0.84 + 0.32 * fine.z) * (0.86 + 0.28 * low);
  vec3 bed = mix(owSRGB(vec3(0.11, 0.105, 0.10)), rock, piece);
  float bedH = mix(0.16, 0.36 + 0.3 * cell.z, piece) + (fine.z - 0.5) * 0.06;`;

const BALLAST = `
  vec2 p = uv * P + uSeed * 4.9;${STONE}
  alb = bed;
  h = bedH;
  rough = 0.95;
  metal = 0.0;
  ao = mix(0.6, 1.0, piece);`;

/** Ivy on a wall: leaves of three greens over one another, dark between them. */
const IVY = `
  vec2 p = uv * P + uSeed * 7.3;
  vec4 leaf = owWorley(p * 4.0, P * 4.0, 0.9);
  vec4 small = owWorley(p * 9.0 + 3.3, P * 9.0, 1.0);
  float blade = smoothstep(0.62, 0.30, leaf.x);
  float bladeS = smoothstep(0.55, 0.25, small.x) * step(0.45, small.z);
  float vein = smoothstep(0.05, 0.0, abs(leaf.x - 0.12)) * 0.2;
  float grain = owFbm01(p * 10.0, P * 10.0, 3, 0.55);
  h = max(blade * (0.5 + 0.3 * leaf.z), bladeS * (0.7 + 0.25 * small.w)) + (grain - 0.5) * 0.04;
  vec3 a = mix(owSRGB(vec3(0.13, 0.24, 0.09)), owSRGB(vec3(0.27, 0.40, 0.14)), leaf.z);
  vec3 b = mix(owSRGB(vec3(0.20, 0.33, 0.11)), owSRGB(vec3(0.42, 0.50, 0.20)), small.w);
  vec3 c = mix(a * (1.0 - vein), b, bladeS);
  alb = mix(owSRGB(vec3(0.04, 0.07, 0.03)), c * (0.88 + 0.24 * grain), max(blade, bladeS));
  rough = 0.7;
  metal = 0.0;
  ao = 0.45 + 0.55 * max(blade, bladeS);`;

/** Trodden earth with cinders in it: what lies where no track runs. */
const EARTH = `
  vec2 p = uv * P + uSeed * 8.3;
  float low = owFbm01(p * 0.5, P * 0.5, 3, 0.55);
  float mid = owFbm01(p * 3.0, P * 3.0, 4, 0.5);
  float grain = owFbm01(p * 20.0, P * 20.0, 4, 0.6);
  vec4 cell = owWorley(p * 7.0, P * 7.0, 1.0);
  float cinder = smoothstep(0.11, 0.04, cell.x) * step(0.66, cell.z);
  h = 0.5 + (low - 0.5) * 0.06 + (mid - 0.5) * 0.05 + (grain - 0.5) * 0.03 + cinder * 0.1;
  vec3 c = mix(owSRGB(vec3(0.25, 0.22, 0.20)), owSRGB(vec3(0.37, 0.33, 0.30)), low * 0.55 + mid * 0.45);
  c *= 0.9 + 0.2 * grain;
  alb = mix(c, owSRGB(vec3(0.16, 0.155, 0.15)) * (0.8 + 0.5 * cell.w), cinder * 0.85);
  rough = 0.96;
  metal = 0.0;
  ao = 1.0 - cinder * 0.08;`;

/** A track's bed: a sleeper to each 0.6 m, across the track, in crushed stone. The rails are the plan's. */
const TRACK = `
  vec2 p = uv * P + uSeed * 5.7;${STONE}
  float x = fract(uv.x * 4.0);
  float tie = smoothstep(0.30, 0.27, abs(x - 0.5));
  float fibre = owFbm01(vec2(p.x * 24.0, p.y * 2.0), vec2(P.x * 24.0, P.y * 2.0), 3, 0.5);
  float split = smoothstep(0.72, 0.9, fibre);
  vec3 wood = mix(owSRGB(vec3(0.15, 0.12, 0.10)), owSRGB(vec3(0.28, 0.23, 0.19)), fibre) * (1.0 - 0.4 * split);
  alb = mix(bed, wood, tie);
  h = mix(bedH, 0.72 - 0.16 * split, tie);
  rough = mix(0.95, 0.86, tie);
  metal = 0.0;
  ao = mix(mix(0.6, 1.0, piece), 1.0, tie);`;

/** The band along a wall's top: a course of bricks on end, every other one standing out. */
const DENTIL = `
  vec2 p = uv * P + uSeed * 2.9;
  float x = uv.x * 16.0;
  float fx = fract(x);
  float tooth = step(0.5, fract(x * 0.5));
  float gap = smoothstep(0.0, 0.06, min(fx, 1.0 - fx));
  float grain = owFbm01(p * 14.0, P * 14.0, 4, 0.55);
  float tone = owHash11(mod(floor(x), 16.0) * 3.1 + uSeed);
  h = gap * (0.3 + 0.5 * tooth) + (grain - 0.5) * 0.04;
  vec3 c = mix(owSRGB(vec3(0.44, 0.27, 0.23)), owSRGB(vec3(0.58, 0.36, 0.30)), tone) * (0.72 + 0.28 * tooth) * (0.93 + 0.14 * grain);
  alb = mix(owSRGB(vec3(0.52, 0.49, 0.45)), c, gap);
  rough = 0.93;
  metal = 0.0;
  ao = 0.65 + 0.35 * gap * (0.6 + 0.4 * tooth);`;

// What weathers these surfaces is soot and grey grit, not sand: the kit's surface, with the reference's
// weathering (dust, streaks, splash, grime in the hollows) set for this place.
const WALL = { weather: [0.1, 0.35, 0.3, 0.2], macro: [0.085, 0.4, 0.2, 0.12], wearColor: 0xa6a39d, dustColor: 0x77746e, grimeColor: 0x2a2826 };
const METAL = { weather: [0.06, 0.3, 0.25, 0.25], macro: [0.085, 0.3, 0.2, 0.08], wearColor: 0x8c7b6c, dustColor: 0x6b6862, grimeColor: 0x231f1c };
const FLOOR = { weather: [0.04, 0, 0, 0.12], macro: [0.05, 0.4, 0.14, 0.12], dustColor: 0x6a675f, grimeColor: 0x24221f };
const own = (body, opts, mat) => {
  const made = surface(body, opts);
  return { ...made, mat: { ...made.mat, ...mat } };
};

/** The map's own surfaces, by the name a material gives as `of`. */
export const SURFACES = {
  bond: own(brick({ face: ['0.70, 0.70, 0.68', '0.83, 0.83, 0.80'], joint: '0.56, 0.56, 0.53', bare: '0.50, 0.33, 0.28', flake: 0.26 }), { metres: 2.4, relief: 0.03, seed: 23 }, WALL),
  soot: own(brick({ face: ['0.19, 0.185, 0.19', '0.31, 0.29, 0.28'], joint: '0.15, 0.15, 0.145', bare: '0.36, 0.22, 0.18', flake: 0.2 }), { metres: 2.4, relief: 0.03, seed: 29 }, { ...WALL, wearColor: 0x55524e, dustColor: 0x4a4844 }),
  kiln: own(brick({ face: ['0.40, 0.21, 0.19', '0.55, 0.30, 0.25'], joint: '0.64, 0.61, 0.56', bare: '0.30, 0.20, 0.22', flake: 0.16 }), { metres: 2.4, relief: 0.03, seed: 31 }, WALL),
  cast: own(CAST, { metres: 2.4, relief: 0.02, seed: 47 }, WALL),
  car_sage: own(car(['0.36, 0.47, 0.38', '0.46, 0.57, 0.47']), { metres: 3.2, relief: 0.05, seed: 59, surface: 'metal' }, METAL),
  car_oxide: own(car(['0.47, 0.22, 0.18', '0.59, 0.30, 0.24']), { metres: 3.2, relief: 0.05, seed: 61, surface: 'metal' }, METAL),
  car_slate: own(car(['0.25, 0.34, 0.48', '0.34, 0.44, 0.58']), { metres: 3.2, relief: 0.05, seed: 73, surface: 'metal' }, METAL),
  car_ochre: own(car(['0.62, 0.46, 0.20', '0.73, 0.56, 0.28']), { metres: 3.2, relief: 0.05, seed: 79, surface: 'metal' }, METAL),
  iron: own(plate(['0.15, 0.145, 0.14', '0.24, 0.225, 0.21', '0.30, 0.19, 0.13']), { metres: 1.6, relief: 0.02, seed: 83, surface: 'metal' }, { ...METAL, wearColor: 0x4c4845, dustColor: 0x45423f }),
  steel: own(plate(['0.40, 0.385, 0.37', '0.54, 0.52, 0.50', '0.40, 0.26, 0.18']), { metres: 1.6, relief: 0.02, seed: 89, surface: 'metal' }, { ...METAL, wearColor: 0x8c8884 }),
  deck: own(DECK, { metres: 2.4, relief: 0.02, seed: 13, surface: 'wood' }, METAL),
  dentil: own(DENTIL, { metres: 2.4, relief: 0.05, seed: 7 }, WALL),
  ivy: own(IVY, { metres: 1.8, relief: 0.06, seed: 37, surface: 'foliage' }, { weather: [0, 0, 0, 0.1], macro: [0.085, 0.4, 0.2, 0.2] }),
  earth: own(EARTH, { metres: 2.6, relief: 0.03, seed: 53, surface: 'dirt', like: 'ground' }, FLOOR),
  ballast: own(BALLAST, { metres: 2.4, relief: 0.05, seed: 67, surface: 'dirt', like: 'ground' }, FLOOR),
  track: own(TRACK, { metres: 2.4, relief: 0.06, seed: 71, surface: 'dirt', like: 'ground' }, FLOOR),
};

/** The palette keys, which of them the kit uses for what, the sky and the cook's light. */
export const LOOK = {
  materials: {
    // walls: painted brick and the same whitewashed, sooted brick, kiln brick, cast concrete
    sd_brick: { of: 'bond', tint: 0xdedcd6 },
    sd_brick_white: { of: 'bond', tint: 0xffffff },
    sd_brick_soot: { of: 'soot', tint: 0xffffff },
    sd_brick_red: { of: 'kiln', tint: 0xffffff },
    sd_cast: { of: 'cast', tint: 0xffffff },
    sd_band: { of: 'dentil', tint: 0xffffff },
    sd_ivy: { of: 'ivy', tint: 0xffffff },
    // floors: crushed stone, the tracks' beds, earth, tar, slabs, boards
    sd_ballast: { of: 'ballast', tint: 0xffffff },
    sd_track: { of: 'track', tint: 0xffffff },
    sd_dirt: { of: 'earth', tint: 0xffffff },
    sd_tar: { of: 'asphalt', tint: 0xd2cedb, weather: [0.03, 0, 0, 0.15] },
    sd_slab: { of: 'concrete_floor', tint: 0xb4b6ae, surface: 'concrete', weather: [0.03, 0, 0, 0.2] },
    sd_plank: { of: 'boards', tint: 0x8d857a, weather: [0.03, 0.1, 0.1, 0.3] },
    // metal: the hall's roof, the rails, the wagons in their four paints, a flat wagon's deck
    sd_tin: { of: 'corrugated', tint: 0x6d7a78, weather: [0.03, 0.3, 0.2, 0.3] },
    sd_roofing: { of: 'ballast', tint: 0x8a8886 },
    sd_rail: { of: 'steel', tint: 0xffffff },
    sd_iron: { of: 'iron', tint: 0xffffff },
    sd_car_sage: { of: 'car_sage', tint: 0xffffff },
    sd_car_oxide: { of: 'car_oxide', tint: 0xffffff },
    sd_car_slate: { of: 'car_slate', tint: 0xffffff },
    sd_car_ochre: { of: 'car_ochre', tint: 0xffffff },
    sd_car_deck: { of: 'deck', tint: 0xffffff },
    // wood, and the paint of a site's mark
    sd_wood: { of: 'boards', tint: 0xa89a86, weather: [0.03, 0.1, 0.1, 0.3] },
    sd_wood_dark: { of: 'boards', tint: 0x6f665c, weather: [0.03, 0.1, 0.1, 0.3] },
    sd_mark: { of: 'cast', tint: 0xf2d27a },
  },
  keys: { ground: 'sd_ballast', side: 'sd_cast', wall: 'sd_brick', band: 'sd_band', cap: 'sd_cast', lid: 'sd_roofing', wood: 'sd_wood', woodDark: 'sd_wood_dark', iron: 'sd_iron', mark: 'sd_mark', apron: 'sd_ballast' },
  sky: { hour: 15.4, weather: { coverage: 0.42, turbidity: 1.7 } },
  // The cook's light for this place: the sky's fill taken toward grey, what the stone sends back, its lamps.
  light: { sky: [1.9, 1.72, 1.5], bounce: [0.46, 0.46, 0.47], lamp: [9, 6.6, 3.6], reach: 13 },
};
