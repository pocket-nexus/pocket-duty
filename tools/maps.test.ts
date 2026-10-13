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
  // (no file lists the maps: a directory with a `map.json` is a map)
  const directories = readdirSync(join(ROOT, "maps"), { withFileTypes: true }).filter((entry) => entry.isDirectory()).map((entry) => entry.name);
  expect(directories.sort()).toEqual(ours.map((map) => map.id).sort());
  expect(readdirSync(join(ROOT, "maps")).filter((name) => name.endsWith(".json"))).toEqual(["street.json"]);
});

test("a map states the table's id and title, four starts or more, and views for `look`", async () => {
  for (const map of ours) {
    // A place of ours (place.js) or a map in the reference's terms (layout.js).
    const free = existsSync(join(ROOT, "maps", map.id, "place.js"));
    const module = await import(join(ROOT, "maps", map.id, free ? "place.js" : "layout.js"));
    const stated = free ? module.PLACE : module.MAP;
    expect(stated.id).toBe(map.id);
    expect(stated.title).toBe(map.title);
    const starts = free ? stated.starts : stated.spawns;
    expect(starts.length).toBeGreaterThanOrEqual(4);
    for (const start of starts) expect(start.slice(0, 3).every((n: number) => Number.isFinite(n))).toBe(true);
    if (free) expect(module.GROUND.length).toBeGreaterThan(0);
    else for (const name of ["STREET", "ALLEYS", "BUILDINGS", "GATE", "SET_PIECES"]) expect(module[name]).toBeDefined();
    const views = JSON.parse(readFileSync(join(ROOT, "maps", map.id, "views.json"), "utf8")) as { name: string; pos: number[]; look: number[] }[];
    expect(views.length).toBeGreaterThan(0);
    for (const view of views) expect([view.name.length > 0, view.pos.length, view.look.length]).toEqual([true, 3, 3]);
  }
});

test("a map's files are text: its name, its plan, its look, its views, and nothing built", () => {
  for (const map of ours) {
    const files = readdirSync(join(ROOT, "maps", map.id)).sort();
    for (const file of files) expect(["layout.js", "look.js", "map.json", "place.js", "views.json"]).toContain(file);
    expect(files).toContain("map.json");
    expect(files).toContain("views.json");
  }
});
