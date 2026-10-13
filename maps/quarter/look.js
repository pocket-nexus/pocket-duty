/**
 * OLD QUARTER: its look. A village on a hill: limewash that has fallen from its walls in patches, plaster
 * in rose, ochre and sage, rubble walls, grey cobbles underfoot, brick laid herringbone and square tiles
 * under the roofs, pan tiles on the houses, under a clear sky early in the afternoon.
 *
 * The surfaces below are this map's own: each is a function, baked into a tile by the reference's
 * material system (web/src/pocket/placelook.js says what a function writes). No picture is read.
 */
import { surface } from '../../web/src/pocket/placelook.js';

/** Limewash over render: where the coat has fallen the render shows, darker and rougher. */
const LIMEWASH = `
  vec2 p = uv * P + uSeed * 3.1;
  float grain = owFbm01(p * 16.0, P * 16.0, 4, 0.55);
  float trowel = owFbm01(vec2(p.x * 1.5, p.y * 5.0), vec2(P.x * 1.5, P.y * 5.0), 3, 0.5);
  float patchy = owFbm01(owWarp(p * 0.75, P * 0.75, 0.4, 2), P * 0.75, 4, 0.55);
  float gone = smoothstep(0.64, 0.68, patchy + (grain - 0.5) * 0.10);
  float rim = smoothstep(0.60, 0.64, patchy + (grain - 0.5) * 0.10) - gone;
  float stain = owFbm01(vec2(p.x * 3.0, p.y * 0.5), vec2(P.x * 3.0, P.y * 0.5), 3, 0.5);
  vec3 coat = mix(owSRGB(vec3(0.80, 0.78, 0.73)), owSRGB(vec3(0.91, 0.89, 0.84)), trowel);
  coat *= 0.93 + 0.14 * grain;
  coat = mix(coat, coat * 0.80, smoothstep(0.5, 0.85, stain) * 0.5);
  vec3 under = mix(owSRGB(vec3(0.58, 0.54, 0.48)), owSRGB(vec3(0.70, 0.65, 0.57)), grain);
  alb = mix(coat, under, gone);
  alb = mix(alb, coat * 1.06, rim * 0.6);
  h = mix(0.62 + (trowel - 0.5) * 0.06 + (grain - 0.5) * 0.03, 0.42 + (grain - 0.5) * 0.10, gone);
  rough = mix(0.90, 0.97, gone);
  metal = 0.0;
  ao = 1.0 - 0.12 * gone;
`;

/** Rubble: stones of many sizes, as they came, bedded in wide mortar. */
const RUBBLE = `
  vec2 p = uv * P + uSeed * 4.9;
  vec2 q = vec2(p.x * 2.0, p.y * 2.5);
  vec2 per = vec2(P.x * 2.0, P.y * 2.5);
  vec4 cell = owWorley(q, per, 0.8);
  float edge = owVoronoiEdge(q, per, 0.8);
  float grain = owFbm01(p * 14.0, P * 14.0, 4, 0.55);
  float stone = smoothstep(0.04, 0.11, edge + (grain - 0.5) * 0.03);
  float dome = sqrt(clamp(edge * 2.4, 0.0, 1.0));
  vec3 top = mix(owSRGB(vec3(0.55, 0.51, 0.44)), owSRGB(vec3(0.73, 0.67, 0.57)), cell.z);
  top = mix(top, owSRGB(vec3(0.62, 0.47, 0.38)), step(0.78, fract(cell.z * 7.31)) * 0.55);
  top *= 0.86 + 0.28 * grain;
  vec3 mortar = owSRGB(vec3(0.74, 0.70, 0.62)) * (0.88 + 0.24 * grain);
  alb = mix(mortar, top, stone);
  h = mix(0.42 + (grain - 0.5) * 0.06, 0.50 + 0.22 * dome + (grain - 0.5) * 0.05, stone);
  rough = mix(0.97, 0.90, stone);
  metal = 0.0;
  ao = mix(0.88, 1.0, stone * dome);
`;

/** Grey cobbles, close set, a hand and a half across, dark earth in the joints. */
const SETTS = `
  vec2 p = uv * P + uSeed * 2.3;
  vec4 cell = owWorley(p * 1.5, P * 1.5, 0.72);
  float edge = owVoronoiEdge(p * 1.5, P * 1.5, 0.72);
  float grain = owFbm01(p * 18.0, P * 18.0, 4, 0.55);
  float worn = owFbm01(p, P, 3, 0.55);
  float stone = smoothstep(0.035, 0.085, edge + (grain - 0.5) * 0.02);
  float dome = sqrt(clamp(edge * 2.6, 0.0, 1.0));
  vec3 top = mix(owSRGB(vec3(0.44, 0.43, 0.42)), owSRGB(vec3(0.64, 0.62, 0.58)), cell.z);
  top = mix(top, owSRGB(vec3(0.58, 0.49, 0.43)), step(0.8, fract(cell.z * 5.77)) * 0.5);
  top *= 0.86 + 0.28 * grain;
  top = mix(top, top * 1.14, smoothstep(0.5, 0.9, worn) * dome);
  vec3 dirt = mix(owSRGB(vec3(0.27, 0.24, 0.21)), owSRGB(vec3(0.40, 0.36, 0.29)), grain);
  alb = mix(dirt, top, stone);
  h = mix(0.28 + (grain - 0.5) * 0.05, 0.48 + 0.32 * dome + (grain - 0.5) * 0.03, stone);
  rough = mix(0.97, 0.84, stone);
  metal = 0.0;
  ao = mix(0.72, 1.0, stone * dome);
`;

/** Pan tiles: eight round backs to a tile, five courses, each course lapping the one under it. */
const PANTILES = `
  vec2 p = uv * P + uSeed * 6.7;
  float fx = fract(uv.x * 8.0);
  float ry = uv.y * 5.0;
  float row = floor(ry);
  float fy = fract(ry);
  vec4 rnd = owHash42(vec2(mod(floor(uv.x * 8.0), 8.0), row) * 1.7 + 3.0 + uSeed);
  float back = sqrt(clamp(1.0 - pow(abs(fx - 0.5) * 2.0, 2.0), 0.0, 1.0));
  float lap = smoothstep(0.0, 0.09, fy);
  float grain = owFbm01(p * 14.0, P * 14.0, 4, 0.55);
  float lichen = smoothstep(0.62, 0.8, owFbm01(p * 2.0, P * 2.0, 3, 0.55));
  h = 0.22 + 0.5 * back + 0.14 * (1.0 - fy) - (1.0 - lap) * 0.2 + (grain - 0.5) * 0.04;
  vec3 c = mix(owSRGB(vec3(0.63, 0.34, 0.22)), owSRGB(vec3(0.78, 0.48, 0.30)), rnd.x);
  c = mix(c, owSRGB(vec3(0.50, 0.27, 0.19)), step(0.82, rnd.y) * 0.6);
  c *= (0.55 + 0.45 * back) * (0.75 + 0.25 * lap) * (0.9 + 0.2 * grain);
  alb = mix(c, owSRGB(vec3(0.55, 0.53, 0.38)), lichen * 0.3);
  rough = 0.9;
  metal = 0.0;
  ao = 0.55 + 0.45 * back * lap;
`;

/** Brick laid herringbone, sixteen widths to a tile, moss in the joints. */
const HERRING = `
  vec2 p = uv * P + uSeed * 5.7;
  vec2 g = uv * 16.0;
  vec2 id = floor(g), f = fract(g);
  float m = mod(id.x - id.y, 4.0);
  float l = f.x, r = 1.0 - f.x, b = f.y, t = 1.0 - f.y;
  vec2 own = id;
  if (m < 0.5) { r = 9.0; }
  else if (m < 1.5) { l = 9.0; own.x -= 1.0; }
  else if (m < 2.5) { b = 9.0; own.y -= 1.0; }
  else { t = 9.0; }
  float d = min(min(l, r), min(b, t));
  vec4 rnd = owHash42(mod(own, 16.0) * 1.3 + 7.0 + uSeed);
  float joint = smoothstep(0.05, 0.15, d);
  float grain = owFbm01(p * 16.0, P * 16.0, 4, 0.55);
  float damp = owFbm01(p * 1.5, P * 1.5, 3, 0.55);
  vec3 brick = mix(owSRGB(vec3(0.58, 0.37, 0.30)), owSRGB(vec3(0.74, 0.53, 0.42)), rnd.x);
  brick = mix(brick, owSRGB(vec3(0.66, 0.60, 0.52)), step(0.84, rnd.y) * 0.5);
  brick *= 0.88 + 0.24 * grain;
  vec3 moss = mix(owSRGB(vec3(0.30, 0.30, 0.20)), owSRGB(vec3(0.38, 0.42, 0.24)), grain);
  vec3 bed = mix(owSRGB(vec3(0.45, 0.40, 0.33)), moss, smoothstep(0.4, 0.7, damp));
  alb = mix(bed, brick, joint);
  alb = mix(alb, moss, (1.0 - smoothstep(0.15, 0.30, d)) * smoothstep(0.55, 0.8, damp) * 0.5);
  h = mix(0.34 + (grain - 0.5) * 0.06, 0.58 + (rnd.z - 0.5) * 0.08 + (grain - 0.5) * 0.04, joint);
  rough = mix(0.97, 0.88, joint);
  metal = 0.0;
  ao = mix(0.78, 1.0, joint);
`;

/** Square tiles, six to a tile's side, a small square of fired clay set cornerwise in each. */
const TILEWORK = `
  vec2 p = uv * P + uSeed * 8.3;
  vec2 g = uv * 6.0;
  vec2 f = fract(g) - 0.5;
  vec4 rnd = owHash42(mod(floor(g), 6.0) * 1.9 + 2.0 + uSeed);
  float square = max(abs(f.x), abs(f.y));
  float corner = abs(f.x) + abs(f.y);
  float inset = 1.0 - smoothstep(0.15, 0.17, corner);
  float ring = smoothstep(0.17, 0.19, corner) * (1.0 - smoothstep(0.30, 0.32, corner));
  float joint = 1.0 - smoothstep(0.455, 0.485, square);
  float grain = owFbm01(p * 16.0, P * 16.0, 4, 0.55);
  float worn = owFbm01(p * 1.5, P * 1.5, 3, 0.55);
  vec3 slab = mix(owSRGB(vec3(0.70, 0.69, 0.62)), owSRGB(vec3(0.80, 0.79, 0.72)), rnd.x);
  slab = mix(slab, slab * 0.90, ring * 0.6);
  vec3 clay = mix(owSRGB(vec3(0.62, 0.36, 0.27)), owSRGB(vec3(0.72, 0.45, 0.33)), rnd.y);
  vec3 c = mix(slab, clay, inset);
  c *= 0.90 + 0.20 * grain;
  c = mix(c, c * 0.84, smoothstep(0.55, 0.85, worn) * 0.6);
  alb = mix(c, owSRGB(vec3(0.38, 0.35, 0.30)), joint);
  h = (1.0 - joint) * (0.60 + (rnd.z - 0.5) * 0.05 + (grain - 0.5) * 0.03) + joint * 0.40;
  rough = mix(0.72, 0.95, joint);
  metal = 0.0;
  ao = 1.0 - 0.2 * joint;
`;

/** The band along a wall's top: square teeth under a fillet, sixteen to a tile, the same at every height. */
const TEETH = `
  vec2 p = uv * P + uSeed * 2.1;
  float x = fract(uv.x * 16.0);
  float tooth = smoothstep(0.06, 0.12, x) * (1.0 - smoothstep(0.54, 0.60, x));
  float grain = owFbm01(p * 14.0, P * 14.0, 4, 0.55);
  h = 0.38 + 0.36 * tooth + (grain - 0.5) * 0.04;
  vec3 c = mix(owSRGB(vec3(0.60, 0.55, 0.48)), owSRGB(vec3(0.72, 0.67, 0.59)), owFbm01(p * 2.0, P * 2.0, 3, 0.5));
  alb = c * (0.72 + 0.28 * tooth) * (0.93 + 0.14 * grain);
  rough = 0.93;
  metal = 0.0;
  ao = 0.7 + 0.3 * tooth;
`;

/** Trodden earth with dry grass lying on it: what a garden nobody waters has. */
const EARTH = `
  vec2 p = uv * P + uSeed * 9.1;
  float low = owFbm01(p * 0.5, P * 0.5, 3, 0.55);
  float mid = owFbm01(p * 4.0, P * 4.0, 4, 0.5);
  float grain = owFbm01(p * 22.0, P * 22.0, 4, 0.6);
  float blades = owFbm01(owRot(p, 0.6) * vec2(3.0, 40.0), P * 40.0, 3, 0.55);
  float tuft = smoothstep(0.45, 0.7, low * 0.5 + mid * 0.5);
  float straw = smoothstep(0.5, 0.75, blades) * tuft;
  vec3 soil = mix(owSRGB(vec3(0.44, 0.36, 0.26)), owSRGB(vec3(0.58, 0.49, 0.36)), mid);
  vec3 grass = mix(owSRGB(vec3(0.62, 0.56, 0.34)), owSRGB(vec3(0.78, 0.70, 0.46)), blades);
  grass = mix(grass, owSRGB(vec3(0.46, 0.50, 0.28)), smoothstep(0.6, 0.8, low) * 0.5);
  alb = mix(soil * (0.9 + 0.2 * grain), grass, straw);
  h = 0.45 + (low - 0.5) * 0.06 + (grain - 0.5) * 0.04 + straw * 0.14;
  rough = 0.96;
  metal = 0.0;
  ao = 1.0 - 0.15 * (1.0 - straw) * tuft;
`;

/** The map's own surfaces by name: what a palette key gives as `of`. */
export const SURFACES = {
  limewash: surface(LIMEWASH, { metres: 3.4, relief: 0.03, seed: 23, like: 'wall' }),
  rubble: surface(RUBBLE, { metres: 3.0, relief: 0.035, seed: 37, like: 'wall' }),
  setts: surface(SETTS, { metres: 2.4, relief: 0.06, seed: 13, surface: 'concrete', like: 'ground' }),
  pantiles: surface(PANTILES, { metres: 1.8, relief: 0.07, seed: 5, like: 'ground' }),
  herring: surface(HERRING, { metres: 1.9, relief: 0.03, seed: 19, like: 'ground' }),
  tilework: surface(TILEWORK, { metres: 2.4, relief: 0.015, seed: 31, like: 'ground' }),
  teeth: surface(TEETH, { metres: 2.4, relief: 0.05, seed: 3, like: 'wall' }),
  earth: surface(EARTH, { metres: 2.8, relief: 0.04, seed: 47, surface: 'dirt', like: 'ground' }),
};

/** The palette keys, which of them the kit uses for what, the windows' spacing, the sky and the cook's light. */
export const LOOK = {
  materials: {
    qt_lime: { of: 'limewash', tint: 0xf4efe4 },
    qt_rose: { of: 'limewash', tint: 0xe2b6a8 },
    qt_ochre: { of: 'limewash', tint: 0xe8c990 },
    qt_sage: { of: 'limewash', tint: 0xc3cba8 },
    qt_rubble: { of: 'rubble', tint: 0xf0e8d8 },
    qt_stone: { of: 'courses', tint: 0xd8d2c6 },
    qt_teeth: { of: 'teeth', tint: 0xe6dfd2 },
    qt_setts: { of: 'setts', tint: 0xe4e1dc },
    qt_brick: { of: 'herring', tint: 0xf2e6da },
    qt_tile: { of: 'tilework', tint: 0xf0ece0 },
    qt_earth: { of: 'earth', tint: 0xf0e6cc },
    qt_roof: { of: 'pantiles', tint: 0xf2dccb },
    qt_plank: { of: 'boards', tint: 0xcbb48e },
    qt_wood: { of: 'boards', tint: 0xd9c08c },
    qt_wood_dark: { of: 'boards', tint: 0x8c7353 },
  },
  keys: { ground: 'qt_setts', side: 'qt_rubble', wall: 'qt_lime', band: 'qt_teeth', cap: 'qt_stone', lid: 'qt_roof', wood: 'qt_wood', woodDark: 'qt_wood_dark', iron: 'metal_dark', apron: 'qt_earth' },
  // A window to so many metres of a house's wall.
  windows: { every: 6.5 },
  sky: { hour: 13.4, weather: { coverage: 0.16, turbidity: 2.6 } },
  // The cook's light for this place: the sky's fill, what its sunlit walls send back, its lamps.
  light: { sky: [2.9, 2.6, 2.3], bounce: [1.15, 1.0, 0.8], lamp: [9, 5.5, 2.4], reach: 10 },
};
