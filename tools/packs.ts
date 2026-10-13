#!/usr/bin/env bun
// The maps' packs as pieces (vendor/pocketjs/tools/pocket3d-packs.ts): every
// cooked map's pack of a profile cut into pieces named by their bytes, with
// one manifest, in .pocket-build/duty/packs/<profile>. A device's copy and a
// release's directory are brought up to date from there with the pieces they
// lack. A section two maps share is one piece.
//
//   bun tools/packs.ts cut [--profile psp30]            cut the cooked maps' packs; says what they share
//   bun tools/packs.ts bring <directory> [--profile psp30] [--prune]
//                                                       the pieces <directory> lacks, then `pocket.packs`
//
// The programs read `pocket.packs` and the pieces beside it: beside the EBOOT on a PSP, in
// `sdmc:/3ds/pocket-duty/` on a 3DS. The browser's page names the manifest by its hash (tools/wgpu.ts).

import { existsSync } from "node:fs";
import { join } from "node:path";
import { bring, piecesOf, writePacks, type Brought, type CutSizes, type Written } from "../vendor/pocketjs/tools/pocket3d-packs.ts";
import { MAPS, packPath, ROOT } from "./maps.ts";

/** Where a profile's pieces and manifests lie. */
export const packsDir = (profile: string): string => join(ROOT, ".pocket-build/duty/packs", profile);

/** The maps that are cooked for a profile, in the order of maps/maps.json. */
export function cooked(profile: string): { id: string; file: string }[] {
  return MAPS.map((map) => ({ id: map.id, file: packPath(map.id, profile) })).filter((each) => existsSync(each.file));
}

/** Cuts every cooked map's pack of a profile. The first map's pack must be there: every copy of the game has it. */
export function cutMaps(profile: string, sizes?: CutSizes, out = packsDir(profile)): Written & { directory: string } {
  const maps = cooked(profile);
  if (maps[0]?.id !== MAPS[0]!.id) throw new Error(`${packPath(MAPS[0]!.id, profile)} is missing: bun tools/duty.ts export, then bun tools/duty.ts cook --profile ${profile}`);
  return { ...writePacks(out, maps.map((each) => ({ name: each.id, file: each.file })), sizes), directory: out };
}

/** Cuts a profile's packs and brings a directory up to date with them. */
export function bringMaps(profile: string, to: string, options: { prune?: boolean } = {}): Brought & { written: Written } {
  const written = cutMaps(profile);
  return { ...bring(packsDir(profile), written.manifest, to, options), written };
}

const mib = (bytes: number) => `${(bytes / (1 << 20)).toFixed(1)} MiB`;
/** One line on what a cut holds. */
export const said = (written: Written): string =>
  `${written.packs.map((pack) => `${pack.name} ${mib(pack.bytes)}`).join(", ")}: ${piecesOf(written.packs).length} pieces of ${mib(written.stored)}, ${mib(written.bytes - written.stored)} shared`;

if (import.meta.main) {
  const [command, ...rest] = process.argv.slice(2);
  const at = rest.indexOf("--profile");
  const profile = at >= 0 && rest[at + 1] ? rest[at + 1]! : "psp30";
  const words = rest.filter((word, i) => word !== "--profile" && rest[i - 1] !== "--profile" && word !== "--prune");
  if (command === "cut" && words.length === 0) {
    const written = cutMaps(profile);
    console.log(`${join(packsDir(profile), written.manifest)}\n${said(written)}; ${mib(written.written)} written`);
  } else if (command === "bring" && words.length === 1) {
    const brought = bringMaps(profile, words[0]!, { prune: rest.includes("--prune") });
    console.log(`${said(brought.written)}\n${brought.sent} pieces copied (${mib(brought.bytes)}), ${brought.had} were there${brought.removed ? `, ${brought.removed} removed` : ""}`);
  } else {
    console.error("usage: bun tools/packs.ts cut [--profile psp30] | bring <directory> [--profile psp30] [--prune]");
    process.exit(1);
  }
}
