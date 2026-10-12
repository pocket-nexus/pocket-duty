#!/usr/bin/env bun
// Pocket Duty's command line.
//
//   bun tools/duty.ts export [--only world,sky,…]       the game as the reference builds it → .pocket-build/duty/ir
//   bun tools/duty.ts cook [--profile psp30] [flags]     DutyIR → a device pack and its receipt
//   bun tools/duty.ts shot [--out f.png] [cook flags]    a frame of the cooked pack, drawn on this computer

import { $ } from "bun";
import { join, resolve } from "node:path";

export const ROOT = resolve(import.meta.dir, "..");
export const BUILD = join(ROOT, ".pocket-build");

function value(argv: string[], flag: string, fallback: string): string {
  const i = argv.indexOf(flag);
  return i >= 0 && argv[i + 1] ? argv[i + 1]! : fallback;
}

const [cmd, ...rest] = process.argv.slice(2);
switch (cmd) {
  case "export": {
    const { exportIr } = await import("./export.ts");
    await exportIr(rest);
    break;
  }
  case "cook":
  case "shot": {
    const profile = value(rest, "--profile", "psp30");
    const extra = rest.filter((a, i) => a !== "--profile" && rest[i - 1] !== "--profile");
    await $`cargo run --release -q -p duty-cook -- ${cmd} --in .pocket-build/duty/ir --out .pocket-build/duty/${profile} --profile profiles/${profile}.json ${extra}`.cwd(ROOT);
    break;
  }
  default:
    console.log("usage: bun tools/duty.ts <export|cook|shot>");
    process.exit(cmd ? 1 : 0);
}
