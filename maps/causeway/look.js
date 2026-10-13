/**
 * CAUSEWAY: its look. The place's family of surfaces and colours, made by our code: rubble walls in thin
 * courses gone grey and green, a band of bosses and bars along their tops, worn flagstones, turf in the
 * open courts, dark boards, still water of an olive cast, under an overcast sky late in the morning.
 *
 * Five surfaces are this map's own: `rubble`, `flags`, `turf`, `murk` and `bars`. The others are the kit's
 * (web/src/pocket/placelook.js) and the reference's library.
 */
import { surface } from '../../web/src/pocket/placelook.js';

/** A surface that weathers green: moss where the kit's stone gathers sand. */
const mossy = (made) => ({ ...made, mat: { ...made.mat, wearColor: 0xb2b3a1, dustColor: 0x77834f, grimeColor: 0x2a3122 } });

/** The map's own surfaces: each the body of a function that writes a colour, a height and a roughness. */
export const SURFACES = {
  // Rubble in fourteen thin courses to a tile, three to six stones a course, no two courses alike:
  // grey stone with a buff one here and there, lichen in drifts, dark open joints.
  rubble: mossy(surface(`
    vec2 p = uv * P + uSeed * 4.1;
    float ry = uv.y * 14.0;
    float row = floor(ry);
    float fy = fract(ry);
    vec4 rr = owHash42(vec2(row, 7.0) + uSeed);
    float n = 3.0 + floor(rr.x * 4.0);
    float xf = uv.x * n + rr.y;
    float fx = fract(xf);
    vec2 id = vec2(mod(floor(xf), n), row);
    vec4 rnd = owHash42(id * 2.3 + 17.0 + uSeed);
    float dx = min(fx, 1.0 - fx) / n;
    float dy = min(fy, 1.0 - fy) / 14.0;
    float joint = min(smoothstep(0.002, 0.007, dx), smoothstep(0.002, 0.006, dy));
    float worn = min(smoothstep(0.0, 0.02, dx), smoothstep(0.0, 0.016, dy));
    float grain = owFbm01(p * 16.0, P * 16.0, 4, 0.55);
    float blotch = owFbm01(p * 1.5 + rnd.zw * 3.0, P * 1.5, 3, 0.55);
    float lichen = smoothstep(0.50, 0.76, owFbm01(p * 0.75 + 3.0, P * 0.75, 3, 0.5));
    h = joint * (0.55 + 0.25 * worn + (rnd.x - 0.5) * 0.18 + (grain - 0.5) * 0.06);
    vec3 c = mix(owSRGB(vec3(0.47, 0.48, 0.43)), owSRGB(vec3(0.61, 0.59, 0.51)), rnd.y);
    c = mix(c, owSRGB(vec3(0.56, 0.50, 0.39)), step(0.82, rnd.z) * 0.6);
    c *= 0.88 + 0.24 * blotch;
    c *= 0.92 + 0.16 * grain;
    c = mix(c, owSRGB(vec3(0.33, 0.39, 0.25)), lichen * 0.55 * (0.5 + 0.5 * grain));
    alb = mix(owSRGB(vec3(0.29, 0.30, 0.25)) * (0.8 + 0.4 * grain), c, joint);
    rough = 0.95;
    metal = 0.0;
    ao = 0.68 + 0.32 * joint * (0.6 + 0.4 * worn);
  `, { metres: 2.8, relief: 0.035, seed: 19, like: 'wall' })),
  // Flagstones of no two shapes, six across a tile, sunk a little at their joints, moss in the joints.
  flags: mossy(surface(`
    vec2 p = uv * P + uSeed * 2.7;
    vec4 cell = owWorley(p * 0.75, P * 0.75, 0.7);
    float edge = owVoronoiEdge(p * 0.75, P * 0.75, 0.7);
    float slab = smoothstep(0.02, 0.05, edge);
    float grain = owFbm01(p * 14.0, P * 14.0, 4, 0.55);
    float wear = owFbm01(p * 2.0, P * 2.0, 3, 0.5);
    float moss = (1.0 - smoothstep(0.02, 0.10, edge)) * smoothstep(0.35, 0.7, owFbm01(p * 1.0 + 5.0, P * 1.0, 3, 0.5));
    h = mix(0.25, 0.55 + (cell.z - 0.5) * 0.12 + (grain - 0.5) * 0.06, slab);
    vec3 c = mix(owSRGB(vec3(0.33, 0.33, 0.29)), owSRGB(vec3(0.45, 0.43, 0.37)), cell.z);
    c *= 0.86 + 0.28 * wear;
    c *= 0.92 + 0.16 * grain;
    vec3 gap = mix(owSRGB(vec3(0.17, 0.17, 0.14)), owSRGB(vec3(0.21, 0.27, 0.14)), moss);
    alb = mix(gap, mix(c, owSRGB(vec3(0.27, 0.32, 0.18)), moss * 0.5), slab);
    rough = 0.93;
    metal = 0.0;
    ao = 0.7 + 0.3 * slab;
  `, { metres: 3.6, relief: 0.03, seed: 37, like: 'ground' })),
  // Turf: short grass in clumps, darker in its hollows, bare earth where it is trodden.
  turf: surface(`
    vec2 p = uv * P + uSeed * 6.3;
    float low = owFbm01(p * 0.5, P * 0.5, 3, 0.55);
    float mid = owFbm01(p * 4.0, P * 4.0, 4, 0.55);
    float blade = owFbm01(p * 28.0, P * 28.0, 3, 0.6);
    vec4 tuft = owWorley(p * 5.0, P * 5.0, 1.0);
    float clump = smoothstep(0.35, 0.05, tuft.x);
    float bare = smoothstep(0.68, 0.86, owFbm01(p * 1.0 + 9.0, P * 1.0, 3, 0.5));
    h = 0.45 + (mid - 0.5) * 0.12 + clump * 0.14 + (blade - 0.5) * 0.08 - bare * 0.1;
    vec3 g = mix(owSRGB(vec3(0.19, 0.25, 0.12)), owSRGB(vec3(0.29, 0.35, 0.16)), low * 0.5 + clump * 0.5);
    g *= 0.82 + 0.36 * blade;
    vec3 earth = owSRGB(vec3(0.24, 0.21, 0.14)) * (0.85 + 0.3 * mid);
    alb = mix(g, earth, bare);
    rough = 0.97;
    metal = 0.0;
    ao = 0.85 + 0.15 * clump;
  `, { metres: 3.2, relief: 0.04, seed: 59, surface: 'dirt', like: 'ground' }),
  // Standing water of an olive cast, a slow swell and a scum of weed on it.
  murk: surface(`
    vec2 p = uv * P + uSeed * 7.1;
    float swell = owFbm01(p * 0.75, P * 0.75, 3, 0.55);
    float ripple = owFbm01(owWarp(p * 3.0, P * 3.0, 0.35, 2), P * 3.0, 3, 0.5);
    float weed = smoothstep(0.6, 0.82, owFbm01(p * 1.5 + 4.0, P * 1.5, 4, 0.55));
    h = 0.5 + (swell - 0.5) * 0.06 + (ripple - 0.5) * 0.05 + weed * 0.03;
    vec3 c = mix(owSRGB(vec3(0.14, 0.17, 0.10)), owSRGB(vec3(0.25, 0.28, 0.15)), swell * 0.6 + ripple * 0.4);
    alb = mix(c, owSRGB(vec3(0.30, 0.37, 0.16)), weed * 0.6);
    rough = mix(0.14, 0.6, weed);
    metal = 0.0;
    ao = 1.0;
  `, { metres: 4, relief: 0.02, seed: 67, surface: 'water', like: 'ground' }),
  // The band along a wall's top: a boss and two bars, eight times to a tile, the same at every height.
  bars: mossy(surface(`
    vec2 p = uv * P + uSeed * 3.3;
    float x = fract(uv.x * 8.0);
    float boss = smoothstep(0.02, 0.06, x) * smoothstep(0.46, 0.42, x);
    float bar = max(smoothstep(0.06, 0.0, abs(x - 0.62)), smoothstep(0.06, 0.0, abs(x - 0.84)));
    float grain = owFbm01(p * 14.0, P * 14.0, 4, 0.55);
    h = 0.35 + 0.4 * boss + 0.3 * bar + (grain - 0.5) * 0.05;
    vec3 c = mix(owSRGB(vec3(0.52, 0.53, 0.46)), owSRGB(vec3(0.65, 0.62, 0.53)), owFbm01(p * 2.0, P * 2.0, 3, 0.5));
    c *= (0.70 + 0.30 * boss + 0.22 * bar) * (0.94 + 0.12 * grain);
    alb = c;
    rough = 0.94;
    metal = 0.0;
    ao = 0.7 + 0.3 * max(boss, bar);
  `, { metres: 2.4, relief: 0.05, seed: 13, like: 'wall' })),
};

/** The palette keys, which of them the kit uses for what, the sky and the cook's light. */
export const LOOK = {
  materials: {
    cw_stone: { of: 'rubble', tint: 0xf4f4ec },
    cw_ashlar: { of: 'rubble', tint: 0xfbfaf0, scale: 5.6 },
    cw_band: { of: 'bars', tint: 0xf0f0e6 },
    cw_flag: { of: 'flags', tint: 0xf2f0e6 },
    cw_bed: { of: 'flags', tint: 0xd8dcc8 },
    cw_turf: { of: 'turf', tint: 0xf0f2e2 },
    cw_lid: { of: 'turf', tint: 0x9aa58a },
    cw_wood: { of: 'boards', tint: 0xa68c6c },
    cw_wood_dark: { of: 'boards', tint: 0x6a5744 },
    cw_water: { of: 'murk', tint: 0xffffff },
  },
  keys: { ground: 'cw_flag', side: 'cw_stone', wall: 'cw_stone', band: 'cw_band', cap: 'cw_ashlar', lid: 'cw_lid', wood: 'cw_wood', woodDark: 'cw_wood_dark', iron: 'metal_dark', water: 'cw_water', mark: 'cw_ashlar', apron: 'cw_flag' },
  // (ruins: no shuttered window in a wall)
  windows: { every: 0 },
  // (the reference's sky reads `cloudCoverage`: a deck that covers nearly all of it, a hazy air)
  sky: { hour: 10.6, weather: { cloudCoverage: 0.9, cloudDensity: 2.6, cirrusCoverage: 0.7, cirrusOpacity: 0.55, turbidity: 3.2 } },
  // The cook's light for this place: an overcast sky's fill, strong and a little cool; little back from
  // wet ground; lamps in the roofed ways.
  light: { sky: [1.05, 1.1, 1.12], bounce: [0.12, 0.13, 0.1], lamp: [19, 14, 8], reach: 15 },
};
