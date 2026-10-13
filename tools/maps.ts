// The maps (maps/street.json and a directory a map under maps/) and where each one's export and packs lie under the ignored .pocket-build/.
//
// The first map is the reference's own street: its export is the reference as it is, and its files keep
// the places they had when it was the only map (.pocket-build/duty/ir, .pocket-build/duty/<profile>).
// Every other map is a layout of ours (maps/<id>/layout.js) with its files under
// .pocket-build/duty/maps/<id>/.
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";

export const ROOT = resolve(import.meta.dir, "..");

export interface MapEntry {
  /** Lowercase letters, digits and hyphens: the directory under maps/, the pack's name on a device. */
  id: string;
  /** What the interface calls it. */
  title: string;
  /** One line under the title in the map list. */
  line: string;
  /** The reference's own street: no layout of ours. */
  reference?: boolean;
}

/**
 * The maps. The first is the reference's street (`maps/street.json`). Every other is a directory under
 * `maps/` with its own `map.json` (`{ "title", "line", "order" }`): its id is the directory's name, and
 * the list's order is `order`, then the id. No file lists the maps: a map is added by adding its directory.
 */
function listed(): MapEntry[] {
  const street = { ...JSON.parse(readFileSync(join(ROOT, "maps/street.json"), "utf8")), reference: true } as MapEntry;
  const ours = readdirSync(join(ROOT, "maps"), { withFileTypes: true })
    .filter((entry) => entry.isDirectory() && existsSync(join(ROOT, "maps", entry.name, "map.json")))
    .map((entry) => ({ id: entry.name, ...(JSON.parse(readFileSync(join(ROOT, "maps", entry.name, "map.json"), "utf8")) as { title: string; line: string; order?: number }) }))
    .sort((a, b) => (a.order ?? 1000) - (b.order ?? 1000) || (a.id < b.id ? -1 : 1))
    .map(({ id, title, line }) => ({ id, title, line }));
  return [street, ...ours];
}

export const MAPS: MapEntry[] = listed();
/** The map a command means when it names none. */
export const FIRST = MAPS[0]!.id;

for (const [i, map] of MAPS.entries()) {
  if (!/^[a-z][a-z0-9-]{0,23}$/.test(map.id)) throw new Error(`maps/: "${map.id}" is not a map's id`);
  if (MAPS.findIndex((m) => m.id === map.id) !== i) throw new Error(`maps/: "${map.id}" stands twice`);
  if (!map.title || !map.line) throw new Error(`maps/${map.id}/map.json has no title or no line`);
  if (!map.reference && !existsSync(join(ROOT, "maps", map.id, "place.js")) && !existsSync(join(ROOT, "maps", map.id, "layout.js"))) throw new Error(`maps/${map.id}/place.js is missing`);
}

export function mapNamed(id: string): MapEntry {
  const map = MAPS.find((m) => m.id === id);
  if (!map) throw new Error(`no map "${id}": the maps are ${MAPS.map((m) => m.id).join(", ")}`);
  return map;
}

/** Where a map's files lie: `.pocket-build/duty` for the street, `.pocket-build/duty/maps/<id>` for another. */
export function mapDir(id: string): string {
  return mapNamed(id).reference ? join(ROOT, ".pocket-build/duty") : join(ROOT, ".pocket-build/duty/maps", id);
}

/** A map's export (DutyIR). */
export const irDir = (id: string): string => join(mapDir(id), "ir");
/** A map's cooked files for a profile: `duty.pack`, `receipt.json`, `map.png`, `map.json`. */
export const cookedDir = (id: string, profile: string): string => join(mapDir(id), profile);
/** A map's pack for a profile. */
export const packPath = (id: string, profile: string): string => join(cookedDir(id, profile), "duty.pack");

/** `--map <id>` among a command's arguments, or the first map; and the arguments without it. */
export function mapArgument(argv: string[]): { map: string; rest: string[] } {
  const i = argv.indexOf("--map");
  if (i < 0) return { map: FIRST, rest: argv };
  const id = argv[i + 1];
  if (!id) throw new Error("--map takes a map's id");
  return { map: mapNamed(id).id, rest: [...argv.slice(0, i), ...argv.slice(i + 2)] };
}
