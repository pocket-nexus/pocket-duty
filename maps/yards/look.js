/**
 * TWIN YARDS: its look. The place's family of surfaces and colours, made by our code: pale stone in
 * courses, a fluted band along the tops of walls, cream slab, cobbles in sand, level sand, weathered
 * boards, under a clear sky a little after two.
 *
 * `LOOK.materials` names the map's palette keys over surfaces: the kit's own, which every map has
 * (`courses`, `cobbles`, `grit`, `flutes`, `boards`, `water`: web/src/pocket/placelook.js), the
 * reference's library (`plaster`, `concrete_floor`, `metal_painted`, …), or a surface of this map's own.
 * A map's own surface is a function written here and listed in `SURFACES`:
 *
 *   import { surface } from '../../web/src/pocket/placelook.js';
 *   export const SURFACES = { rendered: surface(`
 *     vec2 p = uv * P + uSeed;
 *     float grain = owFbm01(p * 12.0, P * 12.0, 4, 0.55);
 *     h = 0.5 + (grain - 0.5) * 0.1;
 *     alb = owSRGB(vec3(0.80, 0.72, 0.60)) * (0.9 + 0.2 * grain);
 *     rough = 0.95; metal = 0.0; ao = 1.0;
 *   `, { metres: 2.4, like: 'wall' }) };
 *
 * and then `{ of: 'rendered', tint: … }` in the materials. Twin Yards needs none of its own.
 */

/** The palette keys, which of them the kit uses for what, the windows' spacing, the sky and the cook's light. */
export const LOOK = {
  materials: {
    yd_stone: { of: 'courses', tint: 0xf4eadc },
    yd_stone_warm: { of: 'courses', tint: 0xe6cfae },
    yd_frieze: { of: 'flutes', tint: 0xecdfcc },
    yd_roof: { of: 'plaster', tint: 0xc9b089, scale: 3.2 },
    yd_cobble: { of: 'cobbles', tint: 0xf2e8d8 },
    yd_slab: { of: 'concrete_floor', tint: 0xe9cfa2, surface: 'concrete' },
    yd_sand: { of: 'grit', tint: 0xf6dfbc },
    yd_wood: { of: 'boards', tint: 0xeadcc4 },
    yd_wood_dark: { of: 'boards', tint: 0xa58c6c },
  },
  keys: { ground: 'yd_cobble', side: 'yd_stone', wall: 'yd_stone', band: 'yd_frieze', cap: 'yd_stone_warm', lid: 'yd_roof', wood: 'yd_wood', woodDark: 'yd_wood_dark', iron: 'metal_dark', apron: 'yd_sand' },
  // A window to so many metres of a house's wall.
  windows: { every: 7.5 },
  sky: { hour: 14.2, weather: { coverage: 0.1, turbidity: 2.2 } },
  // The cook's light for this place: the sky's fill warmed, what the sunlit sand sends back, its lamps.
  light: { sky: [1.55, 1.2, 0.85], bounce: [0.62, 0.46, 0.28], lamp: [9, 5.5, 2.4], reach: 11 },
};
