/**
 * CAUSEWAY: its look (first pass: the kit's surfaces).
 */
export const SURFACES = {};

export const LOOK = {
  materials: {
    cw_stone: { of: 'courses', tint: 0xb9b8a8 },
    cw_flag: { of: 'cobbles', tint: 0x9c9a8c },
    cw_band: { of: 'flutes', tint: 0xb4b09c },
    cw_wood: { of: 'boards', tint: 0xb39a78 },
    cw_wood_dark: { of: 'boards', tint: 0x6f5c46 },
    cw_water: { of: 'water', tint: 0xa0a878 },
    cw_ashlar: { of: 'courses', tint: 0xc2bca6 },
  },
  keys: { ground: 'cw_flag', side: 'cw_stone', wall: 'cw_stone', band: 'cw_band', cap: 'cw_stone', lid: 'cw_stone', wood: 'cw_wood', woodDark: 'cw_wood_dark', iron: 'metal_dark', water: 'cw_water', apron: 'cw_flag' },
  windows: { every: 0 },
  sky: { hour: 11, weather: { coverage: 0.85, turbidity: 6 } },
  light: { sky: [1.1, 1.1, 1.1], bounce: [0.2, 0.2, 0.18], lamp: [10, 8, 5], reach: 12 },
};
