/**
 * COUNTING HOUSE: its look. A grey house in a hard winter, made by our code: blockwork in cold greys
 * with a stained foot, oxblood brick at the front, poured concrete, pale plaster indoors over slate
 * carpet under a tiled ceiling, walnut frames, trodden snow on every yard and on every roof, a road of
 * tar showing through packed snow. The sky is high cloud a little before noon; the snow sends the
 * light back blue.
 */
import { surface } from '../../web/src/pocket/placelook.js';

/** A surface of a cold place: what the reference rubs into it is frost and soot, not sand (indoors: its own tones). */
const cold = (def, more = {}) => ({ ...def, mat: { ...def.mat, wearColor: 0xc9ced6, dustColor: 0xdfe6ee, grimeColor: 0x2d3138, ...more } });

/** The map's own surfaces. */
export const SURFACES = {
  // Trodden snow: drifts, a hollow here and there where a boot pressed it, blue in its shade.
  snow: cold(surface(`
    vec2 p = uv * P + uSeed * 3.1;
    float drift = owFbm01(p * 0.5, P * 0.5, 3, 0.55);
    float mid = owFbm01(p * 3.0, P * 3.0, 4, 0.5);
    float grain = owFbm01(p * 20.0, P * 20.0, 3, 0.6);
    vec4 cell = owWorley(p * 2.5, P * 2.5, 1.0);
    float tread = smoothstep(0.16, 0.05, cell.x) * step(0.72, cell.z);
    h = 0.55 + (drift - 0.5) * 0.25 + (mid - 0.5) * 0.08 + (grain - 0.5) * 0.02 - tread * 0.15;
    vec3 c = mix(owSRGB(vec3(0.72, 0.79, 0.90)), owSRGB(vec3(0.93, 0.95, 0.98)), smoothstep(0.3, 0.75, drift * 0.6 + mid * 0.4));
    c *= 0.97 + 0.06 * grain;
    alb = mix(c, owSRGB(vec3(0.63, 0.70, 0.82)), tread * 0.6);
    rough = 0.86; metal = 0.0; ao = 1.0 - tread * 0.15;
  `, { metres: 3.2, relief: 0.05, seed: 23, surface: 'sand', like: 'ground' }), { weather: [0.05, 0, 0, 0.08] }),
  // The road: tar and grit where wheels went, packed snow and ice between.
  packed: cold(surface(`
    vec2 p = uv * P + uSeed * 5.7;
    float cover = owFbm01(owWarp(p * 2.5, P * 2.5, 0.4, 2), P * 2.5, 4, 0.55);
    float grain = owFbm01(p * 18.0, P * 18.0, 4, 0.55);
    vec4 stone = owWorley(p * 10.0, P * 10.0, 1.0);
    float snow = smoothstep(0.30, 0.46, cover);
    float ice = smoothstep(0.24, 0.30, cover) * (1.0 - snow);
    vec3 tar = owSRGB(vec3(0.30, 0.31, 0.34)) * (0.8 + 0.4 * grain);
    tar = mix(tar, owSRGB(vec3(0.48, 0.49, 0.51)), smoothstep(0.07, 0.02, stone.x) * step(0.6, stone.z) * 0.7);
    tar = mix(tar, owSRGB(vec3(0.52, 0.58, 0.66)), ice * 0.7);
    vec3 white = mix(owSRGB(vec3(0.74, 0.78, 0.86)), owSRGB(vec3(0.88, 0.90, 0.95)), smoothstep(0.46, 0.8, cover)) * (0.95 + 0.08 * grain);
    alb = mix(tar, white, snow);
    h = mix(0.34 + (grain - 0.5) * 0.06, 0.58 + (cover - 0.5) * 0.25, snow);
    rough = mix(0.9, 0.86, snow) - ice * 0.4; metal = 0.0; ao = 1.0;
  `, { metres: 4.8, relief: 0.04, seed: 37, surface: 'concrete', like: 'ground' }), { weather: [0.05, 0, 0, 0.08] }),
  // Carpet in squares of 0.6 m: slate pile with a paler fleck, each square laid its own way, worn in patches.
  carpet: cold(surface(`
    vec2 p = uv * P + uSeed * 2.3;
    vec2 t = fract(uv * 4.0);
    float seam = min(smoothstep(0.0, 0.012, min(t.x, 1.0 - t.x)), smoothstep(0.0, 0.012, min(t.y, 1.0 - t.y)));
    vec4 rnd = owHash42(floor(uv * 4.0) + uSeed + 2.0);
    float pile = owFbm01(p * 40.0, P * 40.0, 3, 0.6);
    float fleck = smoothstep(0.62, 0.7, owFbm01(p * 28.0 + 3.0, P * 28.0, 2, 0.5));
    float worn = owFbm01(p * 1.5, P * 1.5, 3, 0.5);
    vec3 c = mix(owSRGB(vec3(0.27, 0.32, 0.41)), owSRGB(vec3(0.37, 0.42, 0.51)), pile);
    c *= 0.9 + 0.2 * rnd.x;
    c = mix(c, owSRGB(vec3(0.52, 0.56, 0.62)), fleck * 0.45);
    c = mix(c, c * 1.2 + 0.01, smoothstep(0.55, 0.85, worn) * 0.5);
    alb = c * (0.78 + 0.22 * seam);
    h = 0.5 + (pile - 0.5) * 0.1 - (1.0 - seam) * 0.12;
    rough = 0.98; metal = 0.0; ao = 0.85 + 0.15 * seam;
  `, { metres: 2.4, relief: 0.01, seed: 5, surface: 'fabric', like: 'ground' }), { weather: [0, 0, 0, 0.05], wearColor: 0x5a6474, dustColor: 0x4a5464, grimeColor: 0x1c222b }),
  // Concrete blocks, 0.4 by 0.2 m, half a block over on every other course, pitted, streaked down the wall.
  blockwork: cold(surface(`
    vec2 p = uv * P + uSeed * 4.1;
    float row = floor(uv.y * 12.0);
    float xf = uv.x * 6.0 + 0.5 * mod(row, 2.0);
    vec2 f = vec2(fract(xf), fract(uv.y * 12.0));
    vec4 rnd = owHash42(vec2(mod(floor(xf), 6.0), row) * 2.7 + 5.0 + uSeed);
    float dx = min(f.x, 1.0 - f.x) / 6.0, dy = min(f.y, 1.0 - f.y) / 12.0;
    float joint = min(smoothstep(0.002, 0.007, dx), smoothstep(0.002, 0.007, dy));
    float grain = owFbm01(p * 16.0, P * 16.0, 4, 0.55);
    float streak = owFbm01(vec2(p.x * 2.0, p.y * 0.5), vec2(P.x * 2.0, P.y * 0.5), 3, 0.55);
    vec4 pit = owWorley(p * 12.0, P * 12.0, 1.0);
    float pits = smoothstep(0.08, 0.02, pit.x) * step(0.7, pit.z);
    h = joint * (0.6 + (rnd.x - 0.5) * 0.08 + (grain - 0.5) * 0.05) - pits * 0.05;
    vec3 c = mix(owSRGB(vec3(0.40, 0.44, 0.49)), owSRGB(vec3(0.54, 0.57, 0.60)), rnd.y);
    c *= 0.9 + 0.2 * grain;
    c = mix(c, c * 0.78, smoothstep(0.5, 0.8, streak) * 0.6);
    c = mix(c, owSRGB(vec3(0.30, 0.32, 0.35)), pits * 0.6);
    alb = mix(owSRGB(vec3(0.34, 0.36, 0.39)), c, joint);
    rough = 0.95; metal = 0.0; ao = 0.75 + 0.25 * joint;
  `, { metres: 2.4, relief: 0.03, seed: 19 })),
  // Painted plaster: pale, a little warm against the snow's light, with the float's marks in it.
  plaster: cold(surface(`
    vec2 p = uv * P + uSeed * 6.3;
    float trowel = owFbm01(owWarp(p * 1.5, P * 1.5, 0.5, 2), P * 1.5, 4, 0.55);
    float grain = owFbm01(p * 24.0, P * 24.0, 3, 0.55);
    h = 0.5 + (trowel - 0.5) * 0.06 + (grain - 0.5) * 0.02;
    alb = mix(owSRGB(vec3(0.72, 0.73, 0.71)), owSRGB(vec3(0.83, 0.83, 0.80)), trowel) * (0.97 + 0.06 * grain);
    rough = 0.9; metal = 0.0; ao = 1.0;
  `, { metres: 3.0, relief: 0.01, seed: 31, surface: 'plaster' }), { weather: [0.08, 0.1, 0.1, 0.12], wearColor: 0xd2d1cb, dustColor: 0xc6c5be, grimeColor: 0x5a574f }),
  // A ceiling of tiles 0.6 m square in a grid of dark tees, each tile pricked.
  tiles: cold(surface(`
    vec2 p = uv * P + uSeed * 1.7;
    vec2 t = fract(uv * 4.0);
    float grid = min(smoothstep(0.0, 0.03, min(t.x, 1.0 - t.x)), smoothstep(0.0, 0.03, min(t.y, 1.0 - t.y)));
    vec4 rnd = owHash42(floor(uv * 4.0) + uSeed + 9.0);
    float prick = smoothstep(0.35, 0.6, owFbm01(p * 30.0, P * 30.0, 3, 0.6));
    vec3 c = owSRGB(vec3(0.78, 0.79, 0.78)) * (0.93 + 0.1 * rnd.x) * (0.88 + 0.12 * prick);
    alb = mix(owSRGB(vec3(0.36, 0.38, 0.41)), c, grid);
    h = 0.15 + 0.45 * grid - 0.05 * (1.0 - prick);
    rough = 0.93; metal = 0.0; ao = 0.8 + 0.2 * grid;
  `, { metres: 2.4, relief: 0.01, seed: 43, surface: 'plaster', like: 'ground' }), { weather: [0, 0, 0, 0.04], wearColor: 0xcfcfca, dustColor: 0xc4c4be, grimeColor: 0x55534d }),
};

/** The palette keys, which of them the kit uses for what, the sky and the cook's light. */
export const LOOK = {
  materials: {
    ch_snow: { of: 'snow', tint: 0xffffff },
    ch_road: { of: 'packed', tint: 0xffffff },
    ch_carpet: { of: 'carpet', tint: 0xffffff },
    ch_slab: { of: 'concrete_floor', tint: 0xa4a9b0, surface: 'concrete' },
    ch_asphalt: { of: 'asphalt', tint: 0x8f959e },
    ch_block: { of: 'blockwork', tint: 0xffffff },
    ch_brick: { of: 'brick', tint: 0xd6c2bc },
    ch_concrete: { of: 'concrete', tint: 0x8aa4c6 },
    ch_plaster: { of: 'plaster', tint: 0xffffff },
    ch_ceiling: { of: 'tiles', tint: 0xffffff },
    ch_band: { of: 'concrete', tint: 0x586272 },
    ch_cap: { of: 'snow', tint: 0xf6f8fb },
    ch_roof: { of: 'packed', tint: 0x9da2ab },
    ch_wood: { of: 'boards', tint: 0xdcd6ca },
    ch_wood_dark: { of: 'boards', tint: 0x8e8478 },
    ch_desk: { of: 'wood', tint: 0xd8b48a },
    ch_van: { of: 'metal_painted', tint: 0x5f7fa8 },
    ch_van_box: { of: 'corrugated', tint: 0xb4c2d0 },
    ch_tin: { of: 'corrugated', tint: 0x8d5b4a },
  },
  keys: { ground: 'ch_snow', side: 'ch_concrete', wall: 'ch_block', band: 'ch_band', cap: 'ch_cap', lid: 'ch_snow', wood: 'ch_wood', woodDark: 'ch_wood_dark', iron: 'metal_dark', apron: 'ch_snow' },
  sky: { hour: 11, weather: { coverage: 0.55, turbidity: 3 } },
  // The cook's light for this place: the sky's fill brought from blue toward grey (the rooms take a
  // share of it), what the sunlit snow sends back, the rooms' lamps. The exposure is the reference's
  // at the player's first start, in the open yard on sunlit snow (0.83): the fill, the bounce and the
  // lamps are large so that snow in shade and the rooms still show at it.
  light: { sky: [2.94, 2.15, 1.56], bounce: [1.47, 1.51, 1.6], lamp: [29.4, 27.8, 24.8], reach: 14 },
};
