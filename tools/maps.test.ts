// The maps' table and the layouts under maps/ say the same things.
import { expect, test } from "bun:test";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { cookedDir, FIRST, irDir, MAPS, packPath, ROOT } from "./maps.ts";

const ours = MAPS.filter((map) => !map.reference);

test("the first map is the reference's street, where its files always were", () => {
  expect(MAPS[0]).toMatchObject({ id: "street", reference: true });
  expect(FIRST).toBe("street");
  expect(irDir(FIRST)).toBe(join(ROOT, ".pocket-build/duty/ir"));
  expect(packPath(FIRST, "psp30")).toBe(join(ROOT, ".pocket-build/duty/psp30/duty.pack"));
  expect(cookedDir("yards", "n3ds30")).toBe(join(ROOT, ".pocket-build/duty/maps/yards/n3ds30"));
});

test("every directory under maps/ is a map of the table, and every map of ours has its directory", () => {
  const directories = readdirSync(join(ROOT, "maps"), { withFileTypes: true }).filter((entry) => entry.isDirectory()).map((entry) => entry.name);
  expect(directories.sort()).toEqual(ours.map((map) => map.id).sort());
});

test("a layout states the table's id and title, four starts or more inside the perimeter, and views for `look`", async () => {
  for (const map of ours) {
    const layout = await import(join(ROOT, "maps", map.id, "layout.js"));
    expect(layout.MAP.id).toBe(map.id);
    expect(layout.MAP.title).toBe(map.title);
    expect(layout.MAP.spawns.length).toBeGreaterThanOrEqual(4);
    for (const [x, z, yaw] of layout.MAP.spawns) {
      expect(Math.abs(x)).toBeLessThan(58);
      expect(Math.abs(z)).toBeLessThan(58);
      expect(Number.isFinite(yaw)).toBe(true);
    }
    // The reference's passes read these five, whatever the map.
    for (const name of ["STREET", "ALLEYS", "BUILDINGS", "GATE", "SET_PIECES"]) expect(layout[name]).toBeDefined();
    const views = JSON.parse(readFileSync(join(ROOT, "maps", map.id, "views.json"), "utf8")) as { name: string; pos: number[]; look: number[] }[];
    expect(views.length).toBeGreaterThan(0);
    for (const view of views) expect([view.name.length > 0, view.pos.length, view.look.length]).toEqual([true, 3, 3]);
  }
});

test("a map's files are text: a plan, its views, and nothing built", () => {
  for (const map of ours) {
    expect(readdirSync(join(ROOT, "maps", map.id)).sort()).toEqual(["layout.js", "views.json"]);
    expect(existsSync(join(ROOT, "maps", map.id, "layout.js"))).toBe(true);
  }
});
