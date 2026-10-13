#!/usr/bin/env bun
/**
 * Compiles the interface (`ui/`, one PocketJS app) for a device. PocketJS
 * resolves `ui/pocket.json` against the device's profile, picks the
 * presentation its modality asks for and writes the bundle and its pak.
 *
 *   bun tools/ui.ts <psp|vita|3ds>      → .pocket-build/ui/<device>/duty.{js,pak}, plan.json
 *   bun tools/ui.ts prepare             → what the interface compiles from the maps and their cooked levels (ui/app/generated/)
 *   bun tools/ui.ts preview [device…]   → pictures of every screen, .pocket-build/ui/preview/
 *   bun tools/ui.ts test                → the interface driven through a mock renderer
 */
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { extractHostBuildInputs, type HostBuildInputs } from "../vendor/pocketjs/framework/src/manifest/index.ts";
import { createCanvas, loadImage } from "../vendor/pocketjs/node_modules/@napi-rs/canvas";
import { resolve3dsBuildPlan } from "../vendor/pocketjs/tools/3ds-profile.ts";
import { cookedDir, MAPS, type MapEntry } from "./maps.ts";

export const DEVICES = ["psp", "vita", "3ds"] as const;
export type Device = (typeof DEVICES)[number];

const root = resolve(import.meta.dir, "..");
const pocket = join(root, "vendor/pocketjs");
const project = join(root, "ui");
const manifest = join(project, "pocket.json");
const generated = join(project, "app/generated");

/** Devices outside PocketJS's public registry resolve through a profile kept with their tool. */
const PRIVATE: Partial<Record<Device, (manifest: unknown) => unknown>> = { "3ds": resolve3dsBuildPlan };

export interface Interface {
  /** Directory holding `duty.js`, `duty.pak` and `plan.json`. */
  directory: string;
  inputs: HostBuildInputs;
  plan: { features: Record<string, boolean> };
}

function run(args: string[], cwd = pocket) {
  const done = Bun.spawnSync(args, { cwd, stdout: "inherit", stderr: "inherit" });
  if (done.exitCode !== 0) throw new Error(`ui: ${args.slice(0, 3).join(" ")} failed`);
}

/** Writes `bytes` to `path` unless the file already holds them: a build that changed nothing leaves its inputs' dates alone. */
function put(path: string, bytes: Uint8Array | string) {
  const next = typeof bytes === "string" ? new TextEncoder().encode(bytes) : bytes;
  if (existsSync(path) && Buffer.compare(readFileSync(path), next) === 0) return;
  writeFileSync(path, next);
}

/** The side of a map's picture when the cook has not written one. */
const PLACEHOLDER = 256;
/** A map is read under marks: the picture steps back by this much dark glass. It is laid on here,
 *  once, and not as a layer the device would blend over the map in every frame. */
const WASH = "#080c1047";

/**
 * Writes what the interface compiles from the cooked levels (`ui/app/generated/`, ignored): for each
 * map of maps/maps.json its name, its line, the level from above and how many metres a pixel of that
 * picture covers. A picture is the cook's own under a thin dark wash, so the map and the scene cannot
 * disagree. For a map the cook has not written one for, a plain dark picture stands in, at half a
 * metre a pixel. `listed`: the maps the build is for, when a device's package holds fewer than all.
 */
export async function prepareInterface(device?: Device, listed: MapEntry[] = MAPS): Promise<void> {
  mkdirSync(generated, { recursive: true });
  const maps = [];
  const images: Record<string, object> = {};
  for (const entry of listed) {
    const cooked = cookedDir(entry.id, "psp30");
    const picture = join(cooked, "map.png"), factsPath = join(cooked, "map.json");
    const image = `generated/map-${entry.id}.png`;
    let map = { texture: PLACEHOLDER, side: PLACEHOLDER, metres: 0.5 };
    const canvas = createCanvas(PLACEHOLDER, PLACEHOLDER);
    let drawn = canvas;
    if (existsSync(picture) && existsSync(factsPath)) {
      const facts = (await Bun.file(factsPath).json()) as { side: number; x0: number; z0: number; metres: number };
      const bytes = readFileSync(picture);
      // A PNG's size stands in its first chunk.
      const width = bytes.readUInt32BE(16), height = bytes.readUInt32BE(20);
      // A pak image is a square of a power of two, 512 at most.
      if (width !== height || width > 512 || (width & (width - 1)) !== 0) throw new Error(`ui: ${picture} is ${width} × ${height}: a pak image is a square of a power of two, 512 at most`);
      if (!(facts.side > 0 && facts.side <= width && facts.metres > 0)) throw new Error(`ui: ${factsPath} does not describe the picture: side ${facts.side}, metres ${facts.metres}`);
      map = { texture: width, side: facts.side, metres: facts.metres };
      drawn = createCanvas(width, height);
      const ctx = drawn.getContext("2d");
      ctx.fillStyle = "#1b2228";
      ctx.fillRect(0, 0, width, height);
      ctx.drawImage(await loadImage(bytes), 0, 0);
      ctx.fillStyle = WASH;
      ctx.fillRect(0, 0, width, height);
    } else {
      console.log(`ui: no cooked map of ${entry.id} in ${cooked} (map.png, map.json): a plain placeholder picture is in use, at 0.5 m a pixel`);
      const ctx = canvas.getContext("2d");
      ctx.fillStyle = "#1b2228";
      ctx.fillRect(0, 0, PLACEHOLDER, PLACEHOLDER);
    }
    put(join(project, "app", image), drawn.toBuffer("image/png"));
    maps.push({ id: entry.id, title: entry.title, line: entry.line, image, ...map });
    // The PSP keeps a map as 16-bit texels; a map drawn smaller than it is stored is sampled between texels.
    images[image] = device === "psp" ? { psm: 0, linear: true } : { linear: true };
  }
  put(join(generated, "level.ts"), `// Generated by tools/ui.ts from maps/maps.json and the cooked levels (map.json beside each map's psp30 pack).

/** A map: its id, what the interface calls it, its line in the map list, and the level from above,
 *  north (-z) up: the picture (a square of \`texture\` pixels with the map at its top left), the
 *  map's side in it, and the metres a pixel of it covers. */
export interface LevelMap {
  id: string;
  title: string;
  line: string;
  image: string;
  texture: number;
  side: number;
  metres: number;
}

/** The maps, in the order of maps/maps.json. */
export const MAPS: LevelMap[] = ${JSON.stringify(maps, null, 2)};
`);
  put(join(project, "app/images.json"), JSON.stringify(images, null, 2) + "\n");
}

export async function compileInterface(device: Device, listed: MapEntry[] = MAPS): Promise<Interface> {
  await prepareInterface(device, listed);
  const directory = join(root, ".pocket-build/ui", device);
  mkdirSync(directory, { recursive: true });
  const planPath = join(directory, "plan.json");
  const resolver = PRIVATE[device];
  if (resolver) {
    writeFileSync(planPath, JSON.stringify(resolver(await Bun.file(manifest).json()), null, 2) + "\n");
    run(["bun", "tools/build.ts", `--plan=${planPath}`, `--project-root=${project}`, `--outdir=${directory}`]);
  } else {
    run(["bun", "tools/pocket.ts", "compile", "--target", device, "--manifest", manifest, "--project-root", project, "--outdir", directory]);
    await Bun.write(planPath, Bun.file(join(project, ".pocket", device, "plan.json")));
  }
  const plan = await Bun.file(planPath).json();
  return { directory, plan, inputs: extractHostBuildInputs(plan) };
}

if (import.meta.main) {
  const [command, ...rest] = process.argv.slice(2);
  const usage = `usage: bun tools/ui.ts <${DEVICES.join("|")}|prepare|preview [device…]|test [device…]>`;
  if (command === "prepare") await prepareInterface();
  else if (command === "preview" || command === "test") {
    const devices = rest.length ? rest : [...DEVICES];
    for (const device of devices) if (!DEVICES.includes(device as Device)) throw new Error(usage);
    // Both run the bundle on PocketJS's UI core built for wasm.
    if (!existsSync(join(pocket, "hosts/web/pocketjs.wasm"))) run(["bun", "tools/wasm.ts"]);
    // One process per device: a bundle owns `globalThis.frame`.
    for (const device of devices as Device[]) {
      await compileInterface(device);
      run(["bun", `ui/test/${command === "test" ? "flow" : "preview"}.ts`, device], root);
    }
  } else if (DEVICES.includes(command as Device)) {
    const built = await compileInterface(command as Device);
    console.log(`${command}: ${built.inputs.target}, ${built.inputs.viewport.logical.join("×")} @${built.inputs.viewport.rasterDensity}x → ${built.directory}`);
  } else throw new Error(usage);
}
