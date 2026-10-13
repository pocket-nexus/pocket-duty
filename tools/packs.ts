#!/usr/bin/env bun
// The maps' packs as pieces (vendor/pocketjs/tools/pocket3d-packs.ts): every
// cooked map's pack of a profile cut into pieces named by their bytes, with
// one manifest, in .pocket-build/duty/packs/<profile>. A device's copy and a
// release's directory are brought up to date from there with the pieces they
// lack. A section two maps share is one piece.
//
// Beside each map's pack goes the pack `<id>-map`: the map from above, which the interface lays its
// dots on (`duty_interface::Chart`). It is written here from the cook's own `map.png` and `map.json`
// of the same profile, so a map's picture arrives where its pack does, by the same pieces.
//
//   bun tools/packs.ts cut [--profile psp30]            cut the cooked maps' packs; says what they share
//   bun tools/packs.ts bring <directory> [--profile psp30] [--prune]
//                                                       the pieces <directory> lacks, then `pocket.packs`
//
// The programs read `pocket.packs` and the pieces beside it: beside the EBOOT on a PSP, in
// `sdmc:/3ds/pocket-duty/` on a 3DS. The browser's page names the manifest by its hash (tools/wgpu.ts).

import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { IMG_FLAG_LINEAR, PSM } from "../vendor/pocketjs/contracts/spec/spec.ts";
import { decodePng, encodeImageEntry } from "../vendor/pocketjs/framework/compiler/pak.ts";
import { bring, piecesOf, writePacks, type Brought, type CutSizes, type Written } from "../vendor/pocketjs/tools/pocket3d-packs.ts";
import { cookedDir, MAPS, packPath, ROOT } from "./maps.ts";

/** Where a profile's pieces and manifests lie. */
export const packsDir = (profile: string): string => join(ROOT, ".pocket-build/duty/packs", profile);

/** The maps that are cooked for a profile, in the list's order (tools/maps.ts). */
export function cooked(profile: string): { id: string; file: string }[] {
  return MAPS.map((map) => ({ id: map.id, file: packPath(map.id, profile) })).filter((each) => existsSync(each.file));
}

/** The pack that carries a map's picture from above. */
export const chartName = (id: string): string => `${id}-map`;
/** Whether a pack of the manifest is a map's picture and not a map. */
export const isChart = (name: string): boolean => name.endsWith("-map");

/** The ground under a picture's clear pixels, and the dark glass over all of it: a map is read under marks. */
const GROUND = [0x1b, 0x22, 0x28], GLASS = [0x08, 0x0c, 0x10], GLASS_ALPHA = 0x47 / 255;

/**
 * A map's chart pack for a profile, or null when the cook wrote no picture: `DMAP`, the picture's
 * side, the map's side in it, the metres a pixel covers and the length of what follows, then the
 * picture as a PocketJS image entry sampled between texels. A PSP keeps it as 16-bit texels.
 */
export function chartBytes(id: string, profile: string): Uint8Array | null {
  const picture = join(cookedDir(id, profile), "map.png"), factsPath = join(cookedDir(id, profile), "map.json");
  if (!existsSync(picture) || !existsSync(factsPath)) return null;
  const facts = JSON.parse(readFileSync(factsPath, "utf8")) as { side: number; metres: number };
  const image = decodePng(new Uint8Array(readFileSync(picture)));
  const { width, height } = image;
  // (a texture of the interface's core is a square of a power of two, 512 at most)
  if (width !== height || width > 512 || (width & (width - 1)) !== 0) throw new Error(`${picture} is ${width} × ${height}: a map's picture is a square of a power of two, 512 at most`);
  if (!(facts.side > 0 && facts.side <= width && facts.metres > 0)) throw new Error(`${factsPath} does not describe the picture: side ${facts.side}, metres ${facts.metres}`);
  const rgba = new Uint8Array(image.rgba.length);
  for (let at = 0; at < rgba.length; at += 4) {
    const alpha = image.rgba[at + 3]! / 255;
    for (let c = 0; c < 3; c++) {
      const under = image.rgba[at + c]! * alpha + GROUND[c]! * (1 - alpha);
      rgba[at + c] = Math.round(under * (1 - GLASS_ALPHA) + GLASS[c]! * GLASS_ALPHA);
    }
    rgba[at + 3] = 255;
  }
  const entry = encodeImageEntry({ width, height, rgba }, profile.startsWith("psp") ? PSM.PSM_5650 : PSM.PSM_8888, IMG_FLAG_LINEAR);
  const out = new Uint8Array(16 + entry.length);
  const head = new DataView(out.buffer);
  out.set([0x44, 0x4d, 0x41, 0x50]); // DMAP
  head.setUint16(4, width, true);
  head.setUint16(6, facts.side, true);
  head.setFloat32(8, facts.metres, true);
  head.setUint32(12, entry.length, true);
  out.set(entry, 16);
  return out;
}

/** The chart packs of `ids` for a profile, as `writePacks` takes them: each one piece, after the maps' own. */
export function chartPacks(profile: string, ids: readonly string[]): { name: string; file: string; cuts: number[] }[] {
  const directory = join(ROOT, ".pocket-build/duty/charts", profile);
  mkdirSync(directory, { recursive: true });
  return ids.flatMap((id) => {
    const bytes = chartBytes(id, profile);
    if (!bytes) {
      console.log(`packs: the cook wrote no picture of ${id} for ${profile} (${join(cookedDir(id, profile), "map.png")}): the interface shows its dots on a plain ground`);
      return [];
    }
    const file = join(directory, `${id}.chart`);
    writeFileSync(file, bytes);
    return [{ name: chartName(id), file, cuts: [] }];
  });
}

/** Cuts every cooked map's pack of a profile, and its picture. The first map's pack must be there: every copy of the game has it. */
export function cutMaps(profile: string, sizes?: CutSizes, out = packsDir(profile)): Written & { directory: string } {
  const maps = cooked(profile);
  if (maps[0]?.id !== MAPS[0]!.id) throw new Error(`${packPath(MAPS[0]!.id, profile)} is missing: bun tools/duty.ts export, then bun tools/duty.ts cook --profile ${profile}`);
  const charts = chartPacks(profile, maps.map((each) => each.id));
  return { ...writePacks(out, [...maps.map((each) => ({ name: each.id, file: each.file })), ...charts], sizes), directory: out };
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
