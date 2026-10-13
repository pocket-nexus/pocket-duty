/**
 * DEPOT: its look. The place's family of surfaces and colours, made by our code: brick walls gone dark,
 * dark block, cast concrete, tarmac, paving, sheet in zinc and in three paints, under the sky of a little before sunset.
 *
 * Three surfaces are this map's own: `bond`, `blocks` and `tarmac`. The others are the kit's
 * (web/src/pocket/placelook.js) and the reference's library.
 */
import { surface } from '../../web/src/pocket/placelook.js';

/** The map's own surfaces: each the body of a function that writes a colour, a height and a roughness. */
export const SURFACES = {
  // Brick in sixteen courses to a tile: stretchers a half brick apart from course to course, and every
  // sixth course of headers. A burnt brick here and there, soot in drifts.
  bond: surface(`
    vec2 p = uv * P + uSeed * 3.1;
    float ry = uv.y * 16.0;
    float row = floor(ry);
    float fy = fract(ry);
    float header = step(4.5, mod(row, 6.0));
    float n = mix(8.0, 16.0, header);
    float xf = uv.x * n + 0.5 * mod(row, 2.0);
    float fx = fract(xf);
    vec2 id = vec2(mod(floor(xf), n), row);
    vec4 rnd = owHash42(id * 1.7 + 5.0 + uSeed);
    float dx = min(fx, 1.0 - fx) / n;
    float dy = min(fy, 1.0 - fy) / 16.0;
    float joint = min(smoothstep(0.002, 0.006, dx), smoothstep(0.002, 0.006, dy));
    float grain = owFbm01(p * 18.0, P * 18.0, 4, 0.55);
    float soot = owFbm01(p * 1.0, P * 1.0, 3, 0.55);
    h = joint * (0.6 + (rnd.x - 0.5) * 0.12 + (grain - 0.5) * 0.05);
    vec3 c = mix(owSRGB(vec3(0.34, 0.23, 0.17)), owSRGB(vec3(0.47, 0.31, 0.22)), rnd.y);
    c = mix(c, owSRGB(vec3(0.22, 0.18, 0.18)), step(0.86, rnd.z) * 0.8);
    c *= 0.86 + 0.28 * grain;
    c = mix(c, c * 0.6, smoothstep(0.45, 0.85, soot) * 0.6);
    alb = mix(owSRGB(vec3(0.40, 0.37, 0.33)) * (0.8 + 0.3 * grain), c, joint);
    rough = 0.92;
    metal = 0.0;
    ao = 0.7 + 0.3 * joint;
  `, { metres: 2.4, relief: 0.03, seed: 23, like: 'wall' }),
  // Dark blocks, four to a course, each course a third of a block along from the one under it.
  blocks: surface(`
    vec2 p = uv * P + uSeed * 2.3;
    float ry = uv.y * 6.0;
    float row = floor(ry);
    float fy = fract(ry);
    float xf = uv.x * 3.0 + mod(row, 3.0) / 3.0;
    float fx = fract(xf);
    vec2 id = vec2(mod(floor(xf), 3.0), row);
    vec4 rnd = owHash42(id * 2.9 + 3.0 + uSeed);
    float dx = min(fx, 1.0 - fx) / 3.0;
    float dy = min(fy, 1.0 - fy) / 6.0;
    float joint = min(smoothstep(0.003, 0.009, dx), smoothstep(0.003, 0.009, dy));
    float grain = owFbm01(p * 20.0, P * 20.0, 4, 0.6);
    float run = owFbm01(vec2(p.x * 3.0, p.y), vec2(P.x * 3.0, P.y), 3, 0.5);
    h = joint * (0.55 + (rnd.x - 0.5) * 0.08 + (grain - 0.5) * 0.08);
    vec3 c = mix(owSRGB(vec3(0.23, 0.23, 0.25)), owSRGB(vec3(0.33, 0.32, 0.32)), rnd.y);
    c *= 0.85 + 0.3 * grain;
    c = mix(c, c * 0.7, smoothstep(0.5, 0.9, run) * 0.5);
    alb = mix(owSRGB(vec3(0.15, 0.15, 0.16)), c, joint);
    rough = 0.95;
    metal = 0.0;
    ao = 0.72 + 0.28 * joint;
  `, { metres: 2.4, relief: 0.03, seed: 31, like: 'wall' }),
  // Tarmac: dark and fine, its stones showing where it is worn.
  tarmac: surface(`
    vec2 p = uv * P + uSeed * 4.9;
    float low = owFbm01(p * 0.5, P * 0.5, 3, 0.55);
    float mid = owFbm01(p * 5.0, P * 5.0, 4, 0.5);
    float grain = owFbm01(p * 26.0, P * 26.0, 4, 0.6);
    vec4 cell = owWorley(p * 10.0, P * 10.0, 1.0);
    float stone = smoothstep(0.10, 0.03, cell.x) * step(0.55, cell.z) * smoothstep(0.35, 0.7, low);
    h = 0.5 + (low - 0.5) * 0.04 + (grain - 0.5) * 0.05 + stone * 0.06;
    vec3 c = mix(owSRGB(vec3(0.21, 0.21, 0.24)), owSRGB(vec3(0.31, 0.31, 0.33)), low * 0.5 + mid * 0.5);
    c *= 0.88 + 0.24 * grain;
    alb = mix(c, owSRGB(vec3(0.46, 0.44, 0.41)), stone * 0.6);
    rough = 0.9;
    metal = 0.0;
    ao = 1.0;
  `, { metres: 3.2, relief: 0.02, seed: 47, like: 'ground' }),
};

/** The palette keys, which of them the kit uses for what, the sky and the cook's light. */
export const LOOK = {
  materials: {
    dp_brick: { of: 'bond', tint: 0xf2e8e0 },
    dp_block: { of: 'blocks', tint: 0xdcdcde },
    dp_band: { of: 'blocks', tint: 0x9a9796 },
    dp_concrete: { of: 'concrete', tint: 0x9a958d },
    dp_tarmac: { of: 'tarmac', tint: 0xb2b2ba, surface: 'concrete' },
    dp_pave: { of: 'concrete_floor', tint: 0x8f8880, surface: 'concrete' },
    dp_deck: { of: 'metal_painted', tint: 0x5c5f5e },
    dp_steel: { of: 'metal_painted', tint: 0x3f4447 },
    dp_sheet: { of: 'corrugated', tint: 0x8e969b },
    dp_roofing: { of: 'corrugated', tint: 0x6d6862 },
    dp_can_rust: { of: 'corrugated', tint: 0xa35432 },
    dp_can_teal: { of: 'corrugated', tint: 0x3d7078 },
    dp_can_ochre: { of: 'corrugated', tint: 0xa98a3c },
    dp_lid: { of: 'gravel', tint: 0x5a5651 },
    dp_wood: { of: 'boards', tint: 0xb7a283 },
    dp_wood_dark: { of: 'boards', tint: 0x6c5a45 },
  },
  keys: { ground: 'dp_tarmac', side: 'dp_concrete', wall: 'dp_brick', band: 'dp_band', cap: 'dp_concrete', lid: 'dp_lid', wood: 'dp_wood', woodDark: 'dp_wood_dark', iron: 'dp_steel', mark: 'dp_can_ochre', apron: 'dp_tarmac' },
  // A small window to so many metres of a block's wall.
  windows: { every: 11 },
  sky: { hour: 19, weather: { coverage: 0.4, turbidity: 3.4 } },
  // The cook's light for this place: an evening sky's fill, little back from dark ground, strong lamps
  // that hang high.
  light: { sky: [0.85, 0.95, 1.25], bounce: [0.22, 0.2, 0.2], lamp: [44, 28, 13], reach: 18 },
};
