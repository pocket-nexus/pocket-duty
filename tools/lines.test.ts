// The title's line about its map: laid in rows that stay on the screen, and written short enough.
import { expect, test } from "bun:test";
import { fitLines } from "../ui/app/lines.ts";
import { MAPS } from "./maps.ts";

/** Six pixels a character: the smallest type's widest average on the three devices. */
const wide = (row: string) => row.length * 6;

test("a line breaks between words and every row stays inside the width", () => {
  const rows = fitLines("Two walled yards, a long lane, a covered way and the street between.", 240, 2, wide);
  expect(rows).toEqual(["Two walled yards, a long lane, a covered", "way and the street between."]);
  for (const row of rows) expect(wide(row)).toBeLessThanOrEqual(240);
});

test("words past the last row leave, and the row ends in three dots", () => {
  const rows = fitLines("one two three four five six seven eight nine ten", 60, 2, wide);
  expect(rows).toEqual(["one two", "three..."]);
  for (const row of rows) expect(wide(row)).toBeLessThanOrEqual(60);
  expect(fitLines("alpha, beta gamma", 48, 1, wide)).toEqual(["alpha..."]);
});

test("a short line is one row, and an empty one none", () => {
  expect(fitLines("One long street.", 300, 2, wide)).toEqual(["One long street."]);
  expect(fitLines("", 300, 2, wide)).toEqual([]);
});

test("every map's line fits one row of the narrowest screen", () => {
  // A 3DS's upper screen is 400 wide; the title's line starts 24 from the left and keeps 16 at the right.
  for (const map of MAPS) expect(`${map.id}: ${fitLines(map.line, 400 - 24 - 16, 1, wide).join("")}`).toBe(`${map.id}: ${map.line}`);
});
