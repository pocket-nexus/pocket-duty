#!/usr/bin/env bun
// Pocket Duty's command line.
//
//   bun tools/duty.ts export [--map yards] [--only world,sky,…]   a map as the reference builds it → DutyIR
//   bun tools/duty.ts cook [--map yards] [--profile psp30] [flags]  DutyIR → a device pack and its receipt
//   bun tools/duty.ts shot [--map yards] [--out f.png] [cook flags] a frame of the cooked pack, drawn on this computer
//   bun tools/duty.ts look --map yards [--views FILE] [--out DIR]   pictures of a map as the reference draws it
//   bun tools/duty.ts walk [--map yards] [--out f.png]              where the soldiers can walk, from the export
//   bun tools/duty.ts maps                                          the maps, and which of them are cooked
//
// Without `--map` a command means the first map of maps/maps.json, the reference's own street. Where a
// map's files lie is tools/maps.ts.

import { $ } from "bun";
import { existsSync, statSync } from "node:fs";
import { relative } from "node:path";
import { cookedDir, irDir, mapArgument, MAPS, packPath, ROOT } from "./maps.ts";

export { ROOT };
export const BUILD = `${ROOT}/.pocket-build`;

function value(argv: string[], flag: string, fallback: string): string {
  const i = argv.indexOf(flag);
  return i >= 0 && argv[i + 1] ? argv[i + 1]! : fallback;
}

const [cmd, ...all] = process.argv.slice(2);
switch (cmd) {
  case "export": {
    const { map, rest } = mapArgument(all);
    const { exportIr } = await import("./export.ts");
    await exportIr(map, rest);
    break;
  }
  case "look": {
    const { map, rest } = mapArgument(all);
    const { lookAt } = await import("./export.ts");
    await lookAt(map, rest);
    break;
  }
  case "walk": {
    const { map, rest } = mapArgument(all);
    const { walk } = await import("./walk.ts");
    walk(map, rest);
    break;
  }
  case "cook":
  case "shot": {
    const { map, rest } = mapArgument(all);
    const profile = value(rest, "--profile", "psp30");
    const extra = rest.filter((a, i) => a !== "--profile" && rest[i - 1] !== "--profile");
    const into = relative(ROOT, cookedDir(map, profile));
    await $`cargo run --release -q -p duty-cook -- ${cmd} --in ${relative(ROOT, irDir(map))} --out ${into} --profile profiles/${profile}.json ${extra}`.cwd(ROOT);
    break;
  }
  case "maps": {
    for (const map of MAPS) {
      const packs = ["psp30", "n3ds30"].map((profile) => (existsSync(packPath(map.id, profile)) ? `${profile} ${(statSync(packPath(map.id, profile)).size / 1048576).toFixed(1)} MiB` : `${profile} not cooked`));
      console.log(`${map.id.padEnd(10)} ${map.title.padEnd(16)} ${existsSync(irDir(map.id)) ? "exported" : "not exported"}, ${packs.join(", ")}`);
    }
    break;
  }
  default:
    console.log("usage: bun tools/duty.ts <export|cook|shot|look|walk|maps> [--map <id>]");
    process.exit(cmd ? 1 : 0);
}
