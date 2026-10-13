/**
 * ARCADE ROW: its look. An older quarter of the desert town's family, in its own turn: rose and ochre
 * stone laid in tall courses with a thin redder course between, rubble under the road where the way
 * is sunk, a toothed band along the tops of walls, setts in rows half under sand, square flags in the
 * halls and on the walks, orange sand in the yards, under a hazy sky late in the afternoon.
 *
 * Every surface here is a function written here (`surface()` of web/src/pocket/placelook.js, baked by
 * the reference's material system) or one of the kit's; no picture is read.
 */
import { surface } from '../../web/src/pocket/placelook.js';

export const SURFACES = {
  /** Dressed stone in six courses: tall, thin, tall, thin, taller, thin. The thin courses are a redder stone. */
  ashlar: surface(`
  vec2 p = uv * P + uSeed * 4.1;
  float y0 = 0.0, y1 = 0.21, row = 0.0;
  if (uv.y >= 0.21) { y0 = 0.21; y1 = 0.30; row = 1.0; }
  if (uv.y >= 0.30) { y0 = 0.30; y1 = 0.52; row = 2.0; }
  if (uv.y >= 0.52) { y0 = 0.52; y1 = 0.61; row = 3.0; }
  if (uv.y >= 0.61) { y0 = 0.61; y1 = 0.90; row = 4.0; }
  if (uv.y >= 0.90) { y0 = 0.90; y1 = 1.0; row = 5.0; }
  float thin = mod(row, 2.0);
  float fy = (uv.y - y0) / (y1 - y0);
  float n = mix(3.0, 5.0, thin);
  float xf = uv.x * n + owHash11(row * 5.7 + uSeed);
  float fx = fract(xf);
  vec2 id = vec2(mod(floor(xf), n), row);
  vec4 rnd = owHash42(id * 2.7 + 5.0 + uSeed * 3.0);
  float dx = min(fx, 1.0 - fx) / n;
  float dy = min(fy, 1.0 - fy) * (y1 - y0);
  float joint = min(smoothstep(0.002, 0.008, dx), smoothstep(0.002, 0.008, dy));
  float edge = min(smoothstep(0.0, 0.022, dx), smoothstep(0.0, 0.022, dy));
  float grain = owFbm01(p * 16.0, P * 16.0, 4, 0.55);
  float cloud = owFbm01(p * 1.5 + rnd.xy * 7.0, P * 1.5, 3, 0.55);
  float tool = owFbm01(vec2(p.x * 24.0, p.y * 3.0) + rnd.zw * 11.0, vec2(P.x * 24.0, P.y * 3.0), 3, 0.5);
  vec4 pit = owWorley(p * 7.0, P * 7.0, 1.0);
  float pits = smoothstep(0.12, 0.03, pit.x) * step(0.8, pit.z);
  h = joint * (0.6 + 0.22 * edge + (rnd.x - 0.5) * 0.12 + (tool - 0.5) * 0.05 + (grain - 0.5) * 0.03 - thin * 0.05) - pits * 0.07;
  vec3 c = mix(owSRGB(vec3(0.69, 0.53, 0.38)), owSRGB(vec3(0.82, 0.67, 0.50)), smoothstep(0.2, 0.8, cloud));
  c = mix(c, owSRGB(vec3(0.62, 0.44, 0.32)), thin * 0.45);
  c *= 0.88 + 0.24 * rnd.y;
  c *= 0.95 + 0.10 * tool;
  c = mix(c, c * 0.84, (1.0 - smoothstep(0.0, 0.25, fy)) * 0.25 * (1.0 - thin));
  c = mix(c, owSRGB(vec3(0.45, 0.35, 0.27)), pits * 0.7);
  alb = mix(owSRGB(vec3(0.52, 0.42, 0.33)) * (0.85 + 0.3 * grain), c, joint);
  rough = 0.93 - 0.04 * grain;
  metal = 0.0;
  ao = 0.7 + 0.3 * joint * (0.7 + 0.3 * edge);
`, { metres: 3.2, relief: 0.05, seed: 23, like: 'wall' }),

  /** Rubble: stones of many shapes, wider than tall, in pale mortar, damp in patches. Under the road. */
  rubble: surface(`
  vec2 p = uv * P + uSeed * 6.7;
  vec2 q = vec2(p.x * 0.75, p.y);
  vec2 per = vec2(P.x * 0.75, P.y);
  vec4 cell = owWorley(q, per, 0.8);
  float edge = owVoronoiEdge(q, per, 0.8);
  float stone = smoothstep(0.03, 0.10, edge);
  float dome = sqrt(clamp(edge * 2.4, 0.0, 1.0));
  float grain = owFbm01(p * 14.0, P * 14.0, 4, 0.55);
  float damp = owFbm01(p, P, 3, 0.55);
  h = mix(0.25 + (grain - 0.5) * 0.05, 0.5 + 0.3 * dome + (cell.z - 0.5) * 0.12, stone);
  vec3 c = mix(owSRGB(vec3(0.52, 0.41, 0.31)), owSRGB(vec3(0.70, 0.57, 0.43)), cell.z);
  c *= 0.9 + 0.2 * grain;
  c = mix(c, c * 0.78, smoothstep(0.45, 0.8, damp) * 0.6);
  alb = mix(owSRGB(vec3(0.60, 0.49, 0.37)) * (0.8 + 0.3 * grain), c, stone);
  rough = mix(0.97, 0.9, stone);
  metal = 0.0;
  ao = mix(0.7, 1.0, stone * dome);
`, { metres: 2.8, relief: 0.07, seed: 13, like: 'wall' }),

  /** The band along a wall's top: square teeth and the dark slots between them, eight to a tile, the same at every height. */
  teeth: surface(`
  vec2 p = uv * P + uSeed * 2.3;
  float x = fract(uv.x * 8.0);
  float tooth = smoothstep(0.0, 0.05, x) * (1.0 - smoothstep(0.58, 0.63, x));
  float grain = owFbm01(p * 14.0, P * 14.0, 4, 0.55);
  float cloud = owFbm01(p * 2.0, P * 2.0, 3, 0.5);
  h = 0.25 + 0.5 * tooth + (grain - 0.5) * 0.04;
  vec3 c = mix(owSRGB(vec3(0.63, 0.49, 0.37)), owSRGB(vec3(0.76, 0.62, 0.47)), cloud);
  c *= mix(0.42, 1.0, tooth) * (0.94 + 0.12 * grain);
  alb = c;
  rough = 0.93;
  metal = 0.0;
  ao = mix(0.55, 1.0, tooth);
`, { metres: 2.0, relief: 0.06, seed: 9, like: 'wall' }),

  /** Setts in rows, six rows and four setts to a tile, their joints full of sand, sand drifted over them. */
  setts: surface(`
  vec2 p = uv * P + uSeed * 3.3;
  float row = floor(uv.y * 6.0);
  float fy = fract(uv.y * 6.0);
  float xf = uv.x * 4.0 + owHash11(row * 3.1 + uSeed);
  float fx = fract(xf);
  vec2 id = vec2(mod(floor(xf), 4.0), row);
  vec4 rnd = owHash42(id * 1.9 + 7.0 + uSeed);
  float deep = owFbm01(p * 0.75, P * 0.75, 3, 0.55);
  float gap = 0.05 + 0.07 * deep;
  float dx = min(fx, 1.0 - fx) * 1.5, dy = min(fy, 1.0 - fy);
  float drift = smoothstep(0.55, 0.8, owFbm01(p * 1.25 + 3.0, P * 1.25, 3, 0.5));
  float stone = smoothstep(gap, gap + 0.07, dx) * smoothstep(gap, gap + 0.07, dy) * (1.0 - 0.7 * drift);
  float dome = sqrt(clamp(min(dx, dy) * 2.6, 0.0, 1.0));
  float grain = owFbm01(p * 16.0, P * 16.0, 4, 0.5);
  h = mix(0.30 + 0.08 * deep + (grain - 0.5) * 0.04, 0.48 + 0.3 * dome + (rnd.x - 0.5) * 0.08, stone);
  vec3 sand = mix(owSRGB(vec3(0.66, 0.54, 0.40)), owSRGB(vec3(0.75, 0.63, 0.47)), grain);
  vec3 top = mix(owSRGB(vec3(0.57, 0.50, 0.42)), owSRGB(vec3(0.71, 0.63, 0.53)), rnd.y);
  top *= 0.88 + 0.2 * owFbm01(p * 7.0, P * 7.0, 3, 0.5);
  alb = mix(sand, top, stone);
  rough = mix(0.97, 0.86, stone);
  metal = 0.0;
  ao = mix(0.85, 1.0, stone * dome);
`, { metres: 2.4, relief: 0.06, seed: 31, surface: 'dirt', like: 'ground' }),

  /** Square flags in two rows, the rows half a flag apart, worn where feet go. */
  flags: surface(`
  vec2 p = uv * P + uSeed * 5.9;
  float row = floor(uv.y * 2.0);
  float fy = fract(uv.y * 2.0);
  float xf = uv.x * 2.0 + row * 0.5;
  float fx = fract(xf);
  vec2 id = vec2(mod(floor(xf), 2.0), row);
  vec4 rnd = owHash42(id * 4.3 + 2.0 + uSeed);
  float d = min(min(fx, 1.0 - fx), min(fy, 1.0 - fy));
  float joint = smoothstep(0.006, 0.018, d);
  float edge = smoothstep(0.0, 0.06, d);
  float grain = owFbm01(p * 18.0, P * 18.0, 4, 0.55);
  float cloud = owFbm01(p * 2.0 + rnd.xy * 5.0, P * 2.0, 3, 0.55);
  float worn = owFbm01(p * 0.5, P * 0.5, 2, 0.5);
  h = joint * (0.55 + 0.12 * edge + (rnd.x - 0.5) * 0.06 + (grain - 0.5) * 0.03);
  vec3 c = mix(owSRGB(vec3(0.80, 0.66, 0.47)), owSRGB(vec3(0.90, 0.78, 0.60)), cloud);
  c *= 0.92 + 0.16 * rnd.y;
  c *= 0.95 + 0.1 * grain;
  c = mix(c, c * 0.9, worn * 0.4);
  alb = mix(owSRGB(vec3(0.62, 0.48, 0.33)), c, joint);
  rough = 0.9;
  metal = 0.0;
  ao = 0.8 + 0.2 * joint;
`, { metres: 2.6, relief: 0.02, seed: 47, surface: 'concrete', like: 'ground' }),
};

/** The palette keys, which of them the kit uses for what, the windows' spacing, the sky and the cook's light. */
export const LOOK = {
  materials: {
    ar_stone: { of: 'ashlar', tint: 0xefe0cc },
    ar_stone_old: { of: 'ashlar', tint: 0xd2bea4 },
    ar_stone_warm: { of: 'ashlar', tint: 0xd8b891 },
    ar_rubble: { of: 'rubble', tint: 0xe6d4bc },
    ar_frieze: { of: 'teeth', tint: 0xf0dcc0 },
    ar_roof: { of: 'plaster', tint: 0xb8946a, scale: 3.2 },
    ar_sett: { of: 'setts', tint: 0xefe6d8 },
    ar_slab: { of: 'flags', tint: 0xf4e3c8 },
    ar_sand: { of: 'grit', tint: 0xf4e0c6 },
    ar_wood: { of: 'boards', tint: 0xe6c9a4 },
    ar_wood_dark: { of: 'boards', tint: 0x94765a },
    ar_mark: { of: 'plaster', tint: 0x7c3a28 },
  },
  keys: { ground: 'ar_sett', side: 'ar_stone', wall: 'ar_stone', band: 'ar_frieze', cap: 'ar_stone_warm', lid: 'ar_roof', wood: 'ar_wood', woodDark: 'ar_wood_dark', iron: 'metal_dark', mark: 'ar_mark', apron: 'ar_sand' },
  // A window to so many metres of a house's wall.
  windows: { every: 9 },
  sky: { hour: 16.4, weather: { coverage: 0.18, turbidity: 3.0 } },
  // The cook's light for this place: a late sky's fill, what the sunlit sand sends back, its lamps.
  light: { sky: [2.4, 2.0, 1.6], bounce: [0.8, 0.62, 0.42], lamp: [8.5, 6.2, 3.6], reach: 12 },
};
