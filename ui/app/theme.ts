// The look every presentation shares: cool white type over the street, dark
// glass behind anything that must stay legible over a sunlit wall, amber for
// what is selected or asks for care, red for a threat.
export const INK = "#eef4f7f2";
export const DIM = "#eef4f799";
export const FAINT = "#eef4f74d";
export const HAIRLINE = "#ffffff26";
export const OUTLINE = "#080c10";
export const AMBER = "#ffb02a";
export const RED = "#ff3f31";
export const BLOOD = "#8d0f0a";
export const CYAN = "#79d2ff";
export const FRIEND = "#8fc8ff";
export const ENEMY = "#ff7a63";
export const OK = "#a8e86a";
/** Glass behind a readout, behind a strip, and a list's panel. */
export const PLATE = "#080c10a0";
export const GLASS = "#080c10d9";
export const PANEL = "#0a0f14f0";
/** An empty part of a gauge. */
export const WELL = "#080c10b8";
export const WASH = "#ffffff1c";
export const NIGHT = "#06090c";

/** "#rrggbb" or "#rrggbbaa" as the number a colour prop carries, for a
 *  colour written straight to its node (`hot.prop`). */
export function abgr(color: string): number {
  const n = parseInt(color.slice(1, 7), 16);
  const alpha = color.length > 7 ? parseInt(color.slice(7, 9), 16) : 255;
  return ((alpha << 24) | ((n & 0xff) << 16) | (n & 0xff00) | (n >>> 16)) >>> 0;
}
