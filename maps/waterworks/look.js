/**
 * WATERWORKS: its look. The place's family of surfaces and colours, made by our code: red rock in beds
 * about a works of grey-green blocks and slate-blue sheet, dry cracked clay outside, petrol tiles and
 * painted steel inside, rust-brown ribbed roofs, under a clear afternoon sky; cold lamps below ground.
 *
 * Eight surfaces are this map's own: `strata`, `blockwork`, `tiles`, `clay`, `panels`, `ribs` and the
 * two freight paints `ribred` and `ribgreen`. The others are the kit's (web/src/pocket/placelook.js)
 * and the reference's library.
 */
import { surface } from '../../web/src/pocket/placelook.js';

/** A ribbed sheet in one paint: `base` and `worn` are colours as they are seen. */
const ribbed = (base, worn, seed) => surface(`
    vec2 p = uv * P + uSeed * 2.9;
    float rib = abs(fract(uv.y * 12.0) - 0.5) * 2.0;
    float crest = smoothstep(0.15, 0.85, rib);
    float grain = owFbm01(p * 20.0, P * 20.0, 4, 0.55);
    float run = owFbm01(vec2(p.x * 9.0, p.y * 0.6), vec2(P.x * 9.0, P.y * 0.6), 3, 0.5);
    float blotch = owFbm01(p * 1.3, P * 1.3, 3, 0.55);
    float bare = smoothstep(0.62, 0.86, blotch * 0.6 + run * 0.4);
    h = 0.35 + 0.45 * crest + (grain - 0.5) * 0.03;
    vec3 c = owSRGB(vec3(${base}));
    c *= 0.86 + 0.22 * crest;
    c = mix(c, owSRGB(vec3(${worn})), bare * 0.75);
    c *= 0.92 + 0.16 * grain;
    alb = c;
    rough = 0.62 + 0.25 * bare;
    metal = 0.35 * (1.0 - bare);
    ao = 0.78 + 0.22 * crest;
  `, { metres: 2.4, relief: 0.03, seed, surface: 'metal', like: 'wall' });

/** The map's own surfaces: each the body of a function that writes a colour, a height and a roughness. */
export const SURFACES = {
  // Rock in beds: bands of unequal thickness that wander, darker in their joints, split by a few upright cracks.
  strata: surface(`
    vec2 p = uv * P + uSeed * 4.1;
    float wander = owFbm01(vec2(p.x * 1.2, p.y * 0.4), vec2(P.x * 1.2, P.y * 0.4), 3, 0.55);
    float y = uv.y * 7.0 + (wander - 0.5) * 1.4;
    float bed = floor(y);
    float fy = fract(y);
    vec4 rnd = owHash42(vec2(mod(bed, 7.0), 3.0) * 2.3 + uSeed);
    float joint = smoothstep(0.0, 0.10 + 0.08 * rnd.x, min(fy, 1.0 - fy));
    float grain = owFbm01(p * 16.0, P * 16.0, 4, 0.55);
    float lump = owFbm01(p * 3.0, P * 3.0, 4, 0.5);
    vec4 crack = owWorley(vec2(p.x * 2.0, p.y * 0.7), vec2(P.x * 2.0, P.y * 0.7), 1.0);
    float split = smoothstep(0.05, 0.0, abs(crack.x - 0.28)) * step(0.6, crack.z);
    h = joint * (0.45 + 0.30 * lump + (rnd.y - 0.5) * 0.2) + (grain - 0.5) * 0.06 - split * 0.2;
    vec3 c = mix(owSRGB(vec3(0.50, 0.27, 0.20)), owSRGB(vec3(0.64, 0.39, 0.27)), rnd.z);
    c = mix(c, owSRGB(vec3(0.58, 0.44, 0.36)), smoothstep(0.55, 0.85, lump) * 0.5);
    c *= 0.84 + 0.30 * grain;
    c = mix(owSRGB(vec3(0.26, 0.15, 0.13)), c, joint * (1.0 - split * 0.7));
    alb = c;
    rough = 0.95;
    metal = 0.0;
    ao = 0.62 + 0.38 * joint;
  `, { metres: 4.8, relief: 0.09, seed: 17, like: 'wall' }),
  // Cast blocks, three courses to a metre and a half, each two long; a green cast, stains that run down.
  blockwork: surface(`
    vec2 p = uv * P + uSeed * 3.3;
    float ry = uv.y * 6.0;
    float row = floor(ry);
    float fy = fract(ry);
    float xf = uv.x * 3.0 + 0.5 * mod(row, 2.0);
    float fx = fract(xf);
    vec2 id = vec2(mod(floor(xf), 3.0), row);
    vec4 rnd = owHash42(id * 2.7 + 9.0 + uSeed);
    float dx = min(fx, 1.0 - fx) / 3.0;
    float dy = min(fy, 1.0 - fy) / 6.0;
    float joint = min(smoothstep(0.003, 0.009, dx), smoothstep(0.003, 0.009, dy));
    float grain = owFbm01(p * 22.0, P * 22.0, 4, 0.6);
    float run = owFbm01(vec2(p.x * 5.0, p.y * 0.8), vec2(P.x * 5.0, P.y * 0.8), 3, 0.5);
    h = joint * (0.55 + (rnd.x - 0.5) * 0.08 + (grain - 0.5) * 0.08);
    vec3 c = mix(owSRGB(vec3(0.40, 0.45, 0.43)), owSRGB(vec3(0.52, 0.56, 0.53)), rnd.y);
    c *= 0.86 + 0.28 * grain;
    c = mix(c, c * vec3(0.72, 0.74, 0.70), smoothstep(0.5, 0.9, run) * 0.6);
    alb = mix(owSRGB(vec3(0.30, 0.33, 0.32)), c, joint);
    rough = 0.93;
    metal = 0.0;
    ao = 0.74 + 0.26 * joint;
  `, { metres: 2.4, relief: 0.025, seed: 31, like: 'wall' }),
  // Small square tiles, twelve to a tile's side, in petrol with a paler one here and there; dark joints.
  tiles: surface(`
    vec2 p = uv * P + uSeed * 1.9;
    vec2 g = uv * 12.0;
    vec2 id = mod(floor(g), 12.0);
    vec2 f = fract(g);
    vec4 rnd = owHash42(id * 1.3 + 4.0 + uSeed);
    float joint = min(smoothstep(0.03, 0.09, min(f.x, 1.0 - f.x)), smoothstep(0.03, 0.09, min(f.y, 1.0 - f.y)));
    float grain = owFbm01(p * 18.0, P * 18.0, 4, 0.55);
    float wear = owFbm01(p * 1.1, P * 1.1, 3, 0.55);
    h = joint * (0.6 + (rnd.x - 0.5) * 0.05);
    vec3 c = mix(owSRGB(vec3(0.19, 0.31, 0.35)), owSRGB(vec3(0.26, 0.40, 0.43)), rnd.y);
    c = mix(c, owSRGB(vec3(0.36, 0.47, 0.47)), step(0.88, rnd.z));
    c *= 0.88 + 0.22 * grain;
    c = mix(c, owSRGB(vec3(0.40, 0.42, 0.40)), smoothstep(0.6, 0.9, wear) * 0.45);
    alb = mix(owSRGB(vec3(0.14, 0.17, 0.18)), c, joint);
    rough = 0.55 + 0.3 * smoothstep(0.6, 0.9, wear);
    metal = 0.0;
    ao = 0.8 + 0.2 * joint;
  `, { metres: 2.4, relief: 0.01, seed: 43, like: 'ground' }),
  // Dry clay: plates between cracks, their edges curled a little, dust in the cracks.
  clay: surface(`
    vec2 p = uv * P + uSeed * 6.1;
    vec4 cell = owWorley(p * 1.5, P * 1.5, 0.9);
    float edge = owVoronoiEdge(p * 1.5, P * 1.5, 0.9);
    float fine = owVoronoiEdge(p * 4.5 + 3.0, P * 4.5, 0.9);
    float plate = smoothstep(0.015, 0.06, edge) * (0.75 + 0.25 * smoothstep(0.01, 0.04, fine));
    float grain = owFbm01(p * 20.0, P * 20.0, 4, 0.55);
    float low = owFbm01(p * 0.8, P * 0.8, 3, 0.55);
    h = plate * (0.5 + 0.12 * smoothstep(0.0, 0.2, edge)) + (grain - 0.5) * 0.04;
    vec3 c = mix(owSRGB(vec3(0.60, 0.44, 0.37)), owSRGB(vec3(0.71, 0.56, 0.47)), cell.z * 0.6 + low * 0.4);
    c *= 0.90 + 0.18 * grain;
    alb = mix(owSRGB(vec3(0.36, 0.25, 0.21)), c, plate);
    rough = 0.96;
    metal = 0.0;
    ao = 0.7 + 0.3 * plate;
  `, { metres: 3.2, relief: 0.03, seed: 59, like: 'ground' }),
  // Sheet in upright panels, four to a tile, slate-blue paint gone chalky, rust running down from the laps.
  panels: surface(`
    vec2 p = uv * P + uSeed * 2.3;
    float xf = uv.x * 4.0;
    float fx = fract(xf);
    float lap = smoothstep(0.0, 0.035, min(fx, 1.0 - fx));
    float fold = smoothstep(0.42, 0.5, abs(fx - 0.5));
    float grain = owFbm01(p * 24.0, P * 24.0, 4, 0.55);
    float chalk = owFbm01(p * 1.4, P * 1.4, 3, 0.55);
    float run = owFbm01(vec2(p.x * 10.0, p.y * 0.5), vec2(P.x * 10.0, P.y * 0.5), 4, 0.55);
    float rust = smoothstep(0.58, 0.82, run * 0.7 + (1.0 - lap) * 0.5 + chalk * 0.15);
    h = 0.5 * lap + 0.12 * fold + (grain - 0.5) * 0.02;
    vec3 c = mix(owSRGB(vec3(0.36, 0.42, 0.52)), owSRGB(vec3(0.48, 0.53, 0.60)), chalk);
    c *= 0.90 + 0.16 * grain;
    c = mix(c, owSRGB(vec3(0.50, 0.27, 0.15)), rust * 0.8);
    alb = mix(owSRGB(vec3(0.20, 0.22, 0.26)), c, lap);
    rough = 0.6 + 0.3 * rust;
    metal = 0.4 * (1.0 - rust);
    ao = 0.75 + 0.25 * lap;
  `, { metres: 3.2, relief: 0.02, seed: 67, surface: 'metal', like: 'wall' }),
  // The roofs' ribbed sheet, rusted brown; the two paints of the freight boxes.
  ribs: ribbed('0.48, 0.30, 0.19', '0.33, 0.20, 0.14', 71),
  ribred: ribbed('0.58, 0.20, 0.15', '0.36, 0.21, 0.16', 73),
  ribgreen: ribbed('0.27, 0.39, 0.30', '0.34, 0.27, 0.19', 79),
};

/** The palette keys, which of them the kit uses for what, the sky and the cook's light. */
export const LOOK = {
  materials: {
    ww_rock: { of: 'strata' },
    ww_block: { of: 'blockwork' },
    ww_band: { of: 'blockwork', tint: 0xb9bcb8 },
    ww_tile: { of: 'tiles' },
    ww_clay: { of: 'clay', surface: 'sand' },
    ww_sheet: { of: 'panels' },
    ww_roof: { of: 'ribs' },
    ww_can_red: { of: 'ribred' },
    ww_can_green: { of: 'ribgreen' },
    ww_tar: { of: 'concrete_floor', tint: 0x6f7076, surface: 'concrete' },
    ww_slab: { of: 'concrete_floor', tint: 0xa6a8a4, surface: 'concrete' },
    ww_grate: { of: 'metal_painted', tint: 0x8a9694 },
    ww_steel: { of: 'metal_painted', tint: 0x40474a },
    ww_paint: { of: 'metal_painted', tint: 0x5d8c82 },
    ww_tank: { of: 'metal_painted', tint: 0xb4bab8 },
    ww_mark: { of: 'metal_painted', tint: 0xc9a03a },
    ww_lid: { of: 'gravel', tint: 0x6a5a52 },
    ww_wood: { of: 'boards', tint: 0xb39a78 },
    ww_wood_dark: { of: 'boards', tint: 0x66553f },
  },
  keys: { ground: 'ww_clay', side: 'ww_rock', wall: 'ww_rock', band: 'ww_band', cap: 'ww_slab', lid: 'ww_lid', wood: 'ww_wood', woodDark: 'ww_wood_dark', iron: 'ww_steel', mark: 'ww_mark', apron: 'ww_clay' },
  sky: { hour: 15.6, weather: { coverage: 0.22, turbidity: 2.4 } },
  // The cook's light for this place: a clear sky's fill a little warmed by the rock, what the pale clay
  // sends back, and cold lamps with a long reach for the rooms under the ground.
  light: { sky: [1.5, 1.45, 1.5], bounce: [0.6, 0.48, 0.4], lamp: [16, 20, 24], reach: 16 },
};
