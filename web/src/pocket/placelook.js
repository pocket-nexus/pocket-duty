// A place's own look: surfaces of ours beside the reference's, the map's palette, its sky.
//
// The reference makes a surface from a GLSL function that its material system bakes into a tile
// (web/src/materials/library.js), and the world names a tile with a tint and a scale by a palette key
// (web/src/world/palette.js). Both tables are objects the reference exports. A place adds to them while
// the page loads, before the first surface is asked for: the surfaces below, written here, and the keys
// its `LOOK.materials` names. Nothing of the reference's files changes, and the export, the cook and the
// devices take the new tiles as they take the reference's: a tile and a colour a vertex.
//
// The surfaces are drawn by these functions and by nothing else: no picture is read.
import { LIBRARY } from '../materials/library.js';
import { PALETTE } from '../world/palette.js';

const HEAD = 'void owSurface(vec2 uv, out vec3 alb, out float h, out float rough, out float metal, out float ao){\n  const vec2 P = vec2(8.0);\n';

/** Pale stone in five courses of unequal height, two or three long blocks to a course, bedded and pitted. */
const COURSES = HEAD + `
  vec2 p = uv * P + uSeed * 5.3;
  float y0 = 0.0, y1 = 0.26, row = 0.0;
  if (uv.y >= 0.26) { y0 = 0.26; y1 = 0.43; row = 1.0; }
  if (uv.y >= 0.43) { y0 = 0.43; y1 = 0.65; row = 2.0; }
  if (uv.y >= 0.65) { y0 = 0.65; y1 = 0.79; row = 3.0; }
  if (uv.y >= 0.79) { y0 = 0.79; y1 = 1.0; row = 4.0; }
  float fy = (uv.y - y0) / (y1 - y0);
  float n = 2.0 + mod(row, 2.0);
  float xf = uv.x * n + owHash11(row * 7.3 + uSeed);
  float fx = fract(xf);
  vec2 id = vec2(mod(floor(xf), n), row);
  vec4 rnd = owHash42(id * 3.1 + 11.0 + uSeed * 2.0);
  float dx = min(fx, 1.0 - fx) / n;
  float dy = min(fy, 1.0 - fy) * (y1 - y0);
  float joint = min(smoothstep(0.003, 0.010, dx), smoothstep(0.003, 0.010, dy));
  float edge = min(smoothstep(0.0, 0.03, dx), smoothstep(0.0, 0.03, dy));
  float grain = owFbm01(p * 14.0, P * 14.0, 4, 0.55);
  float bed = owFbm01(vec2(p.x * 2.0, p.y * 18.0) + rnd.xy * 9.0, vec2(P.x * 2.0, P.y * 18.0), 3, 0.5);
  float blotch = owFbm01(p * 2.0 + rnd.zw * 5.0, P * 2.0, 3, 0.55);
  vec4 pit = owWorley(p * 9.0, P * 9.0, 1.0);
  float pits = smoothstep(0.10, 0.02, pit.x) * step(0.74, pit.z);
  h = joint * (0.62 + 0.20 * edge + (rnd.x - 0.5) * 0.10 + (bed - 0.5) * 0.05 + (grain - 0.5) * 0.03) - pits * 0.08;
  vec3 c = mix(owSRGB(vec3(0.74, 0.62, 0.49)), owSRGB(vec3(0.84, 0.74, 0.61)), smoothstep(0.25, 0.8, blotch));
  c *= 0.90 + 0.20 * rnd.y;
  c = mix(c, c * 0.88, smoothstep(0.55, 0.8, bed) * 0.5);
  c *= 0.94 + 0.12 * grain;
  c = mix(c, owSRGB(vec3(0.50, 0.40, 0.31)), pits * 0.7);
  alb = mix(owSRGB(vec3(0.60, 0.52, 0.43)) * (0.85 + 0.3 * grain), c, joint);
  rough = 0.94 - 0.04 * grain;
  metal = 0.0;
  ao = 0.72 + 0.28 * joint * (0.7 + 0.3 * edge);
}`;

/** Worn cobbles of many sizes, half under the sand that lies between them. */
const COBBLES = HEAD + `
  vec2 p = uv * P + uSeed * 3.7;
  vec4 cell = owWorley(p * 1.75, P * 1.75, 0.85);
  float edge = owVoronoiEdge(p * 1.75, P * 1.75, 0.85);
  float deep = owFbm01(p * 0.75, P * 0.75, 3, 0.55);
  float stone = smoothstep(0.03 + 0.12 * deep, 0.13 + 0.16 * deep, edge);
  float dome = sqrt(clamp(edge * 2.2, 0.0, 1.0));
  float grain = owFbm01(p * 16.0, P * 16.0, 4, 0.5);
  h = mix(0.30 + 0.10 * deep + (grain - 0.5) * 0.04, 0.45 + 0.35 * dome, stone);
  vec3 sand = mix(owSRGB(vec3(0.72, 0.56, 0.38)), owSRGB(vec3(0.82, 0.67, 0.46)), grain);
  vec3 top = mix(owSRGB(vec3(0.58, 0.49, 0.40)), owSRGB(vec3(0.71, 0.61, 0.50)), cell.z);
  top *= 0.88 + 0.2 * owFbm01(p * 7.0, P * 7.0, 3, 0.5);
  alb = mix(sand, top, stone);
  rough = mix(0.97, 0.86, stone);
  metal = 0.0;
  ao = mix(0.85, 1.0, stone * dome);
}`;

/** The band along a wall's top: two flutes and a bead, twelve times to a tile, the same at every height. */
const FLUTES = HEAD + `
  vec2 p = uv * P + uSeed * 2.9;
  float x = fract(uv.x * 12.0);
  float flute = max(smoothstep(0.10, 0.0, abs(x - 0.18)), smoothstep(0.10, 0.0, abs(x - 0.42)));
  float bead = smoothstep(0.10, 0.03, abs(x - 0.76));
  float grain = owFbm01(p * 14.0, P * 14.0, 4, 0.55);
  h = 0.6 - 0.35 * flute + 0.25 * bead + (grain - 0.5) * 0.04;
  vec3 c = mix(owSRGB(vec3(0.66, 0.54, 0.42)), owSRGB(vec3(0.76, 0.64, 0.51)), owFbm01(p * 2.0, P * 2.0, 3, 0.5));
  c *= (1.0 - 0.38 * flute) * (1.0 + 0.10 * bead) * (0.94 + 0.12 * grain);
  alb = c;
  rough = 0.93;
  metal = 0.0;
  ao = 1.0 - 0.3 * flute;
}`;

/** Upright boards, seven to a tile, each weathered its own way, with open joints between. */
const BOARDS = HEAD + `
  vec2 p = uv * P + uSeed * 6.1;
  float xf = uv.x * 7.0;
  float fx = fract(xf);
  vec4 rnd = owHash42(vec2(mod(floor(xf), 7.0), 3.0) + uSeed);
  float gap = smoothstep(0.0, 0.05, min(fx, 1.0 - fx));
  float streak = owFbm01(vec2(p.x * 10.0, p.y) + rnd.xy * 13.0, vec2(P.x * 10.0, P.y), 4, 0.55);
  float grey = smoothstep(0.55, 0.9, owFbm01(p * 3.0, P * 3.0, 3, 0.5));
  h = gap * (0.55 + (streak - 0.5) * 0.25 + (rnd.z - 0.5) * 0.08);
  vec3 c = mix(owSRGB(vec3(0.38, 0.29, 0.20)), owSRGB(vec3(0.60, 0.47, 0.33)), streak);
  c *= 0.85 + 0.3 * rnd.w;
  c = mix(c, owSRGB(vec3(0.60, 0.56, 0.50)), grey * 0.35);
  alb = c * (0.35 + 0.65 * gap);
  rough = 0.9;
  metal = 0.0;
  ao = 0.6 + 0.4 * gap;
}`;

/** Level sand of fine grain, a pebble here and there: what lies in a yard that is walked. */
const GRIT = HEAD + `
  vec2 p = uv * P + uSeed * 4.4;
  float low = owFbm01(p * 0.5, P * 0.5, 3, 0.55);
  float mid = owFbm01(p * 4.0, P * 4.0, 4, 0.5);
  float grain = owFbm01(p * 22.0, P * 22.0, 4, 0.6);
  vec4 cell = owWorley(p * 6.0, P * 6.0, 1.0);
  float pebble = smoothstep(0.09, 0.03, cell.x) * step(0.8, cell.z);
  h = 0.5 + (low - 0.5) * 0.05 + (mid - 0.5) * 0.04 + (grain - 0.5) * 0.03 + pebble * 0.12;
  vec3 c = mix(owSRGB(vec3(0.76, 0.60, 0.40)), owSRGB(vec3(0.85, 0.70, 0.49)), low * 0.6 + mid * 0.4);
  c *= 0.92 + 0.16 * grain;
  alb = mix(c, owSRGB(vec3(0.60, 0.50, 0.40)), pebble * 0.8);
  rough = 0.96;
  metal = 0.0;
  ao = 1.0 - pebble * 0.1;
}`;

const stone = { parallax: 0.016, detile: 0, detail: [8, 0.9, 0.5, 22], macro: [0.085, 0.6, 0.22, 0.5], macroBig: [2.0, 0.12, 0.03, 0], weather: [0.3, 0.4, 0.5, 0.5], wearColor: 0xc4b094, dustColor: 0xb89c72, grimeColor: 0x3a3024, roughness: [0.96, -0.02, 0.26] };

/** The surfaces of ours, as the reference's library lists its own: the name a palette key gives as `of`. */
export const SURFACES = {
  courses: { glsl: COURSES, surface: 'concrete', bake: { size: 1024, worldSize: 3.0, relief: 0.05, seed: 41 }, mat: { scale: 3.0, ...stone } },
  cobbles: { glsl: COBBLES, surface: 'dirt', bake: { size: 1024, worldSize: 2.4, relief: 0.07, seed: 17 }, mat: { uvMode: 'triplanar', scale: 2.4, detile: 0, detail: [8, 0.7, 0.3, 18], macro: [0.05, 0.44, 0.14, 0.35], weather: [0.15, 0, 0, 0.18], dustColor: 0xb89c72, grimeColor: 0x4c4132, roughness: [1.0, 0.0, 0.3] } },
  grit: { glsl: GRIT, surface: 'sand', bake: { size: 1024, worldSize: 2.6, relief: 0.03, seed: 53 }, mat: { uvMode: 'triplanar', scale: 2.6, detile: 0, detail: [8, 0.7, 0.3, 18], macro: [0.05, 0.44, 0.14, 0.35], weather: [0.15, 0, 0, 0.18], dustColor: 0xb89c72, grimeColor: 0x4c4132, roughness: [1.0, 0.0, 0.3] } },
  flutes: { glsl: FLUTES, surface: 'concrete', bake: { size: 1024, worldSize: 2.4, relief: 0.05, seed: 7 }, mat: { scale: 2.4, ...stone } },
  boards: { glsl: BOARDS, surface: 'wood', bake: { size: 1024, worldSize: 1.6, relief: 0.03, seed: 29 }, mat: { scale: 1.6, parallax: 0.008, detail: [10, 0.8, 0.42, 18], macro: [0.085, 0.34, 0.14, 0.5], weather: [0.3, 0.35, 0.5, 0.45], wearColor: 0xa88b62 } },
};

/** The name our surface `of` has in the reference's library. */
export const libraryName = (of) => (of in SURFACES ? `place_${of}` : of);

/**
 * Puts a place's look into the reference's tables. `look.materials`: palette keys of the map, each
 * `{ of, tint, scale?, weather?, surface? }` where `of` is a surface of ours or of the reference's library.
 * Returns the library names of the keys, for the floors' families.
 */
export function applyLook(look = {}) {
  for (const [name, def] of Object.entries(SURFACES)) LIBRARY[`place_${name}`] ??= def;
  for (const [key, m] of Object.entries(look.materials ?? {})) {
    const name = libraryName(m.of);
    if (!LIBRARY[name]) throw new Error(`LOOK.materials.${key}: no surface "${m.of}"`);
    const opts = { vertexMasks: true, tint: m.tint ?? 0xffffff };
    if (m.scale) opts.scale = m.scale;
    if (m.weather) opts.weather = m.weather;
    PALETTE[key] = { name, surface: m.surface ?? LIBRARY[name].surface, opts };
  }
}

/** The hour and the weather a place states, on the reference's sky. */
export function applySky(sky, look = {}) {
  if (!sky || !look.sky) return;
  if (look.sky.weather) sky.setWeather(look.sky.weather);
  if (look.sky.hour !== undefined) sky.setTimeOfDay(look.sky.hour);
}
