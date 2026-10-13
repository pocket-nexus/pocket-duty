#!/usr/bin/env bun
// Pocket Duty on PS Vita: compile the interface (ui/) for the Vita, build the
// SELF with PocketJS's pinned toolchain, stage it with the maps' packs on the
// wired debug host's share, replace the binary in Pocket Devkit's native
// slot over the cable, steer and measure it.
//
//   bun tools/vita.ts build [--standalone] [--debug]   # vita/ → dist/vita/pocket-duty-<title>.{self,vpk,runtime.json}
//   bun tools/vita.ts stage                    # interface, every cooked map's pieces and boot.txt → <share>/duty/
//   bun tools/vita.ts serve                    # the USB host (PocketJS's vita:dev serve): keep it running, detached
//   bun tools/vita.ts run [--no-build] [--boot "<words>"]
//                                              # stage + build + the SELF into Pocket Devkit's native slot over the cable
//   bun tools/vita.ts session                  # a session for the title on a share whose host was started for another
//   bun tools/vita.ts status
//   bun tools/vita.ts ctl "ui=start auto=1"   # <share>/duty/control.txt (the words are in vita/src/main.rs, `remote`:
//                                              # `mode=title|play|menu` sets the flow, `ui=start|menu|resume|restart|title`
//                                              # asks what the interface's lists ask, `map=<id>` asks for a match on a map,
//                                              # `press=<mask>` and `tap=<x>,<y>` reach the interface, `pad=<held>,<x>,<y>,
//                                              # <frames>` holds the player's pad, `at=x,y,z,yaw,pitch` puts the player
//                                              # somewhere, and `sky= haze= shadows= effects= figures= shades= flash=
//                                              # interface=` leave a part of the frame out)
//   bun tools/vita.ts capture [--out f.png]    # the frame on the display, 960 x 544
//   bun tools/vita.ts bench [--seconds 60] [--clock 50]
//                                              # a match played by the bench's player (ended after --clock seconds, so
//                                              # the results come up): frame timings → .pocket-build/validation/vita/
//   bun tools/vita.ts cost [--view "x,y,z,yaw,pitch"] [--quick]
//                                              # one held view with each part left out in turn, the GPU timed: what each costs
//   bun tools/vita.ts package                  # dist/vita/pocket-duty-PKDT00001.vpk: every cooked map's pieces, the
//                                              # interface and the programs the console compiled, no debug link
//   bun tools/vita.ts emu [--seconds 25] [--boot "<words>"]
//                                              # the package in Vita3K, in a VitaFS of its own under .pocket-build/:
//                                              # the status the game writes there
//
// `run`, `bench` and `emu` take `--map <id>` (maps/maps.json): the map the run starts on. Every map that is
// cooked for `vita60` is staged, so the interface's map list offers them all. Staging brings the share up to
// date (tools/packs.ts): only the pieces it lacks are copied, and the console fetches from the share the
// pieces its memory card lacks.
//
// The default title is Pocket Devkit (PocketJS apps/devkit), the development container installed on the
// console: its native slot accepts a replacement SELF, and reopening its LiveArea bubble returns to it. A
// standalone package carries no debug link.
//
// One USB host owns the Vita's cable. `--share DIR` (or DUTY_VITA_SHARE) names the directory of a host that
// already runs; the default is this repository's `.pocket-build/vita-usb/share`. After a crash, or after
// Pocket Devkit is opened again, start the host again once Devkit is open; when Devkit goes silent, a fresh
// host and then a replug.

import { $ } from "bun";
import { createHash, randomBytes } from "node:crypto";
import { cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { PACKS_FILE } from "../vendor/pocketjs/tools/pocket3d-packs.ts";
import { packageVitaVpk } from "../vendor/pocketjs/tools/vita-package.ts";
import { VitaUsbClient } from "../vendor/pocketjs/tools/vita-dev-client.ts";
import { prepareVitaUsb } from "../vendor/pocketjs/tools/vita-usb.ts";
import { FIRST, mapNamed } from "./maps.ts";
import { bringMaps, cooked, said } from "./packs.ts";
import { compileInterface } from "./ui.ts";

export const ROOT = resolve(import.meta.dir, "..");
const POCKETJS = resolve(ROOT, "vendor/pocketjs");
const CRATE = resolve(ROOT, "vita");
export const OUT = resolve(ROOT, "dist/vita");
const VALIDATION = resolve(ROOT, ".pocket-build/validation/vita");
const home = process.env.HOME ?? "";
const vitasdk = process.env.VITASDK || `${home}/vitasdk`;
const rustup = Bun.which("rustup") ?? `${home}/.cargo/bin/rustup`;
const BIN = "pocket-duty-vita";
/** The toolchain PocketJS's Vita host is pinned to (vendor/pocketjs/hosts/vita/rust-toolchain.toml). */
const TOOLCHAIN = "nightly-2026-05-28";
/**
 * Pocket Devkit on the development console: vitaTitleId("dev.pocket-nexus.devkit"). A console whose Devkit was
 * installed before PocketJS's rename has it as P3B1D7273: `--title P3B1D7273` (or DUTY_VITA_TITLE). The USB
 * host's log names the title the console asks for (`pocket-vita/<title>/session.json`).
 */
const DEVKIT = process.env.DUTY_VITA_TITLE || "P25BFE5E2";
/** The package's own title. */
export const TITLE = "PKDT00001";
export const PROFILE = "vita60";
/** The compiled interface (`bun tools/ui.ts vita`), which the console reads beside the packs. */
const INTERFACE = ["duty.js", "duty.pak"];

const argv = import.meta.main ? process.argv.slice(2) : [];
const cmd = argv[0] ?? "";
const has = (flag: string, from = argv) => from.includes(flag);
const opt = (key: string, fallback: string, from = argv) => {
  const at = from.indexOf(key);
  return at < 0 ? fallback : (from[at + 1] ?? fallback);
};

export function context(from: string[] = argv) {
  const standalone = has("--standalone", from);
  const title = opt("--title", standalone ? TITLE : DEVKIT, from);
  const share = resolve(opt("--share", process.env.DUTY_VITA_SHARE ?? resolve(ROOT, ".pocket-build/vita-usb/share"), from));
  return { title, share, game: resolve(share, "duty"), output: `pocket-duty-${title}`, release: !has("--debug", from), standalone };
}

/**
 * The id a build carries. A development build takes a fresh one, which the debug host tells two builds apart
 * by. `tools/release.ts` names a release build's (POCKET_RELEASE_BUILD: 32 hex digits from the commit and the
 * hashes of what the package is built from), so two builds of one commit are the same bytes.
 */
function buildId(): string {
  const named = process.env.POCKET_RELEASE_BUILD;
  if (named === undefined) return randomBytes(16).toString("hex");
  if (!/^[0-9a-f]{32}$/.test(named)) throw new Error("POCKET_RELEASE_BUILD is not 32 hex digits");
  return named;
}

/** Builds the SELF and its package. `assets`: the directory whose files go into the package beside the SELF. */
export async function build(from: string[] = argv, assets = `${CRATE}/assets`): Promise<string> {
  const c = context(from);
  if (!existsSync(`${vitasdk}/bin/vita-pack-vpk`)) throw new Error(`VitaSDK not found at ${vitasdk} (set VITASDK)`);
  // A development build carries PocketJS's USB driver: the wired debug host talks to it.
  const usb = c.standalone ? undefined : await prepareVitaUsb();
  const nativeBuild = buildId();
  const env = {
    ...process.env,
    PATH: `${vitasdk}/bin:${home}/.cargo/bin:${process.env.PATH ?? ""}`,
    VITASDK: vitasdk,
    VITA_DEFAULT_TITLE_ID: c.title,
    POCKETJS_VITA_TITLE_ID: c.title,
    POCKETJS_NATIVE_BUILD: nativeBuild,
    POCKETJS_EMBED_APP: "0",
    TARGET_AR: "arm-vita-eabi-ar",
    AR_armv7_sony_vita_newlibeabihf: "arm-vita-eabi-ar",
    TARGET_CC: "arm-vita-eabi-gcc",
    CC_armv7_sony_vita_newlibeabihf: "arm-vita-eabi-gcc",
    TARGET_CXX: "arm-vita-eabi-g++",
    CXX_armv7_sony_vita_newlibeabihf: "arm-vita-eabi-g++",
  };
  const cargoArgs = [...(c.release ? ["--release"] : []), ...(c.standalone ? ["--no-default-features"] : [])];
  console.log(`vita: cargo vita build vpk (title ${c.title}, ${c.release ? "release" : "debug"}${c.standalone ? ", standalone" : ""})`);
  await $`${rustup} run ${TOOLCHAIN} cargo vita build vpk -- ${cargoArgs}`.cwd(CRATE).env(env);

  const target = `${CRATE}/target/armv7-sony-vita-newlibeabihf/${c.release ? "release" : "debug"}`;
  const eboot = `${target}/${BIN}.self`;
  const sfo = `${target}/${BIN}.sfo`;
  const vpk = `${target}/${BIN}.vpk`;
  // An unsafe-homebrew SELF: loading the USB driver and writing the inactive native slot need its permissions.
  await $`${vitasdk}/bin/vita-make-fself ${target}/${BIN}.velf ${eboot}`;
  await $`${vitasdk}/bin/vita-mksfoex -d ATTRIBUTE2=12 -s TITLE_ID=${c.title} ${"Pocket Duty"} ${sfo}`;
  // The bubble's icon is Pocket3D's, in the development build and in the package; the tree holds the LiveArea's
  // pictures. Loaded by path at run time: the module also bakes the icons, and its rasterizer's types come with
  // PocketJS's own dependencies.
  const iconModule: string = `${POCKETJS}/tools/pocket3d-icon.ts`;
  const { POCKET3D_ICON } = (await import(iconModule)) as { POCKET3D_ICON: { vita: string } };
  await packageVitaVpk({ tool: `${vitasdk}/bin/vita-pack-vpk`, sfo, eboot, output: vpk, usbDriver: usb?.driver, applicationAssets: assets, icon: POCKET3D_ICON.vita });

  mkdirSync(OUT, { recursive: true });
  cpSync(vpk, `${OUT}/${c.output}.vpk`);
  cpSync(eboot, `${OUT}/${c.output}.self`);
  const selfSha256 = createHash("sha256").update(readFileSync(eboot)).digest("hex");
  const runtime = `${OUT}/${c.output}.runtime.json`;
  writeFileSync(
    runtime,
    JSON.stringify({ version: 1, titleId: c.title, applicationId: "dev.pocket-nexus.duty", output: c.output, nativeBuild, plan: null, self: `${c.output}.self`, usbDebug: !c.standalone, usbDriver: usb?.fingerprint ?? null, selfSha256 }, null, 2) + "\n",
  );
  console.log(`vita: ${OUT}/${c.output}.self (native build ${nativeBuild})`);
  return runtime;
}

const sha = (path: string) => createHash("sha256").update(readFileSync(path)).digest("hex");

/** Copies `from` to `to` unless the same bytes are there. */
function bringFile(from: string, to: string): boolean {
  if (existsSync(to) && sha(to) === sha(from)) return false;
  cpSync(from, to);
  return true;
}

/**
 * The share's folder for the game as a run needs it: the interface, the pieces of every map that is cooked for
 * the Vita with their manifest, and the words for the start. The console never creates a directory on
 * `host0:`; every one it writes into exists up front.
 */
export async function stage(from: string[] = argv, boot = ""): Promise<void> {
  const c = context(from);
  for (const directory of ["", "gxp", "packs"]) mkdirSync(`${c.game}/${directory}`, { recursive: true });
  if (cooked(PROFILE)[0]?.id !== FIRST) throw new Error(`no pack of ${FIRST} for ${PROFILE}: bun tools/duty.ts cook --profile ${PROFILE}`);
  const brought = bringMaps(PROFILE, `${c.game}/packs`, { prune: true });
  console.log(`vita: ${said(brought.written)}; ${brought.sent} pieces copied to the share, ${brought.had} were there`);
  const ui = await compileInterface("vita");
  for (const name of INTERFACE) if (bringFile(`${ui.directory}/${name}`, `${c.game}/${name}`)) console.log(`vita: staged ${name}`);
  const map = opt("--map", "", from);
  const words = [map ? `map=${mapNamed(map).id}` : "", boot, opt("--boot", "", from)].filter(Boolean).join(" ");
  writeFileSync(`${c.game}/boot.txt`, `${words}\n`);
}

/**
 * A session for the title on the share, when it has none: the console's Devkit answers the session it finds
 * under its own title, and a host that was started for another title wrote none there. The host itself serves
 * the whole share and is left running.
 */
function session(from: string[]): void {
  const c = context(from);
  if (!existsSync(`${c.share}/pocket-vita/${c.title}/session.json`)) {
    new VitaUsbClient(c.share, c.title, 60_000).startSession();
    console.log(`vita: started a session for ${c.title} on ${c.share}`);
  }
}

/** PocketJS's wired debug tool, pointed at the chosen share. */
async function dev(from: string[], ...args: string[]): Promise<void> {
  const c = context(from);
  mkdirSync(c.share, { recursive: true });
  await $`bun ${POCKETJS}/tools/vita-dev.ts ${args} --runtime ${OUT}/${c.output}.runtime.json --title ${c.title} --dir ${c.share}`.cwd(POCKETJS);
}

/** Says words to the running game. The last line differs each time, so the same words can be said twice. */
export function ctl(words: string, from: string[] = argv): void {
  const c = context(from);
  mkdirSync(c.game, { recursive: true });
  writeFileSync(`${c.game}/control.txt`, `${words}\n#${Date.now()}\n`);
}

/** The running process's status receipt, straight from the share: the game's own part is under `engine`. */
export function status(from: string[] = argv): any {
  const c = context(from);
  // The console replaces the file every few frames; a read can land between the old one and the new one.
  for (let i = 0; ; i++) {
    try {
      return JSON.parse(readFileSync(`${c.share}/pocket-vita/${c.title}/status.json`, "utf8"));
    } catch (e) {
      if (i >= 20) throw e;
      Bun.sleepSync(25);
    }
  }
}

/** Waits until the game says it runs a map, and returns its status. */
async function running(from: string[], seconds = 600): Promise<any> {
  const until = Date.now() + seconds * 1000;
  let last = "";
  while (Date.now() < until) {
    try {
      const engine = status(from).engine ?? {};
      if (engine.stage === "running") return engine;
      if (engine.stage === "error") throw new Error(`the game stopped: ${engine.message}`);
      if (engine.message && engine.message !== last) console.log(`vita: ${(last = engine.message)}`);
    } catch (e) {
      if (String(e).includes("the game stopped")) throw e;
    }
    await Bun.sleep(500);
  }
  throw new Error("the game did not come up: is Pocket Devkit open, the cable in, and the USB host running (bun tools/vita.ts serve)?");
}

/** A run of the bench's player on a map: what the frames cost, as a record under .pocket-build/validation/vita/. */
async function bench(from: string[]): Promise<void> {
  const seconds = Number(opt("--seconds", "60", from));
  await running(from);
  // `--clock N` ends the match after N seconds, so the run goes through to the results.
  const clock = opt("--clock", "", from);
  ctl("ui=start auto=1 god=1 stats=1", from);
  await Bun.sleep(3000);
  // (a match that starts sets its own clock: the word comes after it has)
  if (clock) ctl(`clock=${clock}`, from);
  const samples: any[] = [];
  for (let at = 0; at < seconds; at++) {
    await Bun.sleep(1000);
    const e = status(from).engine;
    samples.push({ frameMs: e.frameMs, worstMs: e.worstMs, late: e.late, lateBy: e.lateBy, tris: e.tris, figureTris: e.figureTris, draws: e.draws, cpuMs: e.cpuMs, scale: e.governor?.scale });
  }
  const end = status(from).engine;
  const mean = (key: string) => samples.reduce((sum, s) => sum + s[key], 0) / samples.length;
  const record = {
    map: end.map, seconds, mode: end.mode, match: end.match, pictures: end.pictures, frames: end.frames, late: end.late, lateBy: end.lateBy,
    frameMs: mean("frameMs"), worstMs: Math.max(...samples.map((s) => s.worstMs)), tris: { mean: mean("tris"), most: Math.max(...samples.map((s) => s.tris)) }, draws: { mean: mean("draws"), most: Math.max(...samples.map((s) => s.draws)) },
    cpuMs: end.cpuMs, governor: end.governor, memory: end.memory, pack: end.pack, msaa: end.msaa, clockMhz: end.clockMhz, samples,
  };
  mkdirSync(VALIDATION, { recursive: true });
  const out = `${VALIDATION}/bench-${end.map}.json`;
  writeFileSync(out, JSON.stringify(record, null, 2) + "\n");
  console.log(`vita: ${end.map}: ${record.frameMs.toFixed(2)} ms a frame, worst ${record.worstMs.toFixed(1)}, ${end.late} late of ${end.frames} (cpu ${end.lateBy.cpu}, gpu ${end.lateBy.gpu}), ${Math.round(record.tris.mean)} triangles (most ${record.tris.most}), ${Math.round(record.draws.mean)} draws → ${out}`);
  ctl("auto=0 stats=0", from);
}

/**
 * What each part of the frame costs: one held view (`--view "x,y,z,yaw,pitch"`, default the street from its
 * middle) with the part left out, the GPU waited for after each frame and timed (`profile=1`: the time from
 * the scene's end to the GPU's word that it is drawn). The soldiers stand in a held view; the held weapon
 * does not.
 */
async function cost(from: string[]): Promise<void> {
  const engine = await running(from);
  const view = opt("--view", "0,1.7,0,0,0", from);
  const read = async (words: string) => {
    ctl(words, from);
    await Bun.sleep(5000);
    const e = status(from).engine;
    return { gpuMs: e.gpuMs as number, drawMs: e.cpuMs.draw as number, selectMs: e.cpuMs.select as number, tris: e.tris as number, figureTris: e.figureTris as number, draws: e.draws as number };
  };
  const all = "sky=1 haze=1 shadows=1 effects=1 figures=1 shades=1 flash=1 interface=1 nearer=100 merge=150";
  ctl(`ui=start god=1 govern=0 profile=1 pace=1 view=${view},65`, from);
  await Bun.sleep(3000);
  const whole = await read(all);
  const rows: [string, Awaited<ReturnType<typeof read>>][] = [["the whole frame", whole]];
  // (`--quick`: the whole frame alone, for a build that differs in what it was started with)
  for (const [name, words] of has("--quick", from) ? [] : [
    ["without the sky", "sky=0"],
    ["without the haze", "haze=0"],
    ["without the ground's shadow picture", "shadows=0"],
    ["without the soldiers and their shadows", "figures=0"],
    ["without the soldiers' shadows", "shades=0"],
    ["without the interface", "interface=0"],
    ["runs drawn one by one (merge=0)", "merge=0"],
    ["runs joined across 200 triangles (merge=600)", "merge=600"],
    ["the levels' distances halved (nearer=50)", "nearer=50"],
    ["the levels' distances quartered (nearer=25)", "nearer=25"],
  ] as const) {
    rows.push([name, await read(`${all} ${words}`)]);
  }
  rows.push(["the whole frame again", await read(all)]);
  ctl(`${all} govern=1 profile=0 view=off`, from);
  mkdirSync(VALIDATION, { recursive: true });
  const out = `${VALIDATION}/cost-${engine.map}.json`;
  if (!has("--quick", from)) writeFileSync(out, JSON.stringify({ map: engine.map, view, pictures: engine.pictures, msaa: engine.msaa, rows: Object.fromEntries(rows) }, null, 2) + "\n");
  const signed = (ms: number) => `${ms >= 0 ? "+" : ""}${ms.toFixed(2)}`;
  for (const [name, r] of rows) console.log(`${name.padEnd(46)} GPU ${r.gpuMs.toFixed(2)} ms (${signed(r.gpuMs - whole.gpuMs)}), draws ${r.drawMs.toFixed(2)} ms, choice ${r.selectMs.toFixed(2)} ms, ${r.tris} + ${r.figureTris} triangles in ${r.draws} draws`);
  console.log(`vita: ${out}`);
}

/** The directory a package holds beside its SELF: the LiveArea, the interface, the packs and the programs. */
async function packageAssets(from: string[], programs: boolean): Promise<string> {
  const c = context(from);
  const staged = resolve(ROOT, ".pocket-build/vita-package");
  rmSync(staged, { recursive: true, force: true });
  mkdirSync(`${staged}/gxp`, { recursive: true });
  cpSync(`${CRATE}/assets`, staged, { recursive: true });
  const manifest = `${c.game}/gxp/manifest.txt`;
  if (programs) {
    // The programs are compiled on a console (SceShaccCg) and kept on the share under the hash of their source.
    if (!existsSync(manifest)) throw new Error(`${manifest} is missing: run the development build on the console first (bun tools/vita.ts run)`);
    const hashes = readFileSync(manifest, "utf8").split("\n").filter(Boolean);
    for (const hash of hashes) {
      const gxp = `${c.game}/gxp/${hash}.gxp`;
      if (!existsSync(gxp)) throw new Error(`${gxp} is missing: the console has not compiled this build's programs`);
      cpSync(gxp, `${staged}/gxp/${hash}.gxp`);
    }
    console.log(`vita: ${hashes.length} programs`);
  }
  const brought = bringMaps(PROFILE, `${staged}/packs`);
  console.log(`vita: ${said(brought.written)}`);
  if (!existsSync(`${staged}/packs/${PACKS_FILE}`)) throw new Error("the packs' manifest was not written");
  const ui = await compileInterface("vita");
  for (const name of INTERFACE) cpSync(`${ui.directory}/${name}`, `${staged}/${name}`);
  return staged;
}

/** The package: every cooked map's pieces, the interface and the programs inside, no debug link. */
export async function packageVpk(from: string[] = argv, programs = true): Promise<string> {
  const flags = [...from.filter((word) => word !== "--standalone"), "--standalone"];
  await build(flags, await packageAssets(from, programs));
  return `${OUT}/pocket-duty-${TITLE}.vpk`;
}

/**
 * The package in Vita3K, in a VitaFS of its own. The game is told to write its status to its data folder
 * (`report=1` in `ux0:data/pocket-duty/boot.txt`), which is what this prints. On macOS Vita3K shows no picture
 * of a game's own GXM scene that can be read back; a console is where the frame is looked at.
 */
async function emu(from: string[]): Promise<void> {
  const app = [process.env.VITA3K, `${home}/Applications/Vita3K.app/Contents/MacOS/Vita3K`, "/Applications/Vita3K.app/Contents/MacOS/Vita3K"].find((path) => path && existsSync(path));
  if (!app) throw new Error("Vita3K not found (set VITA3K)");
  const base = [process.env.VITA3K_CONFIG, `${home}/Library/Application Support/Vita3K/Vita3K/config.yml`, `${home}/Library/Application Support/Vita3K/config.yml`].find((path) => path && existsSync(path));
  if (!base) throw new Error("Vita3K has no config yet (set VITA3K_CONFIG): open it once");
  const share = context(from).game;
  // The programs a console compiled, when there are any: Vita3K has the shader compiler only with a console's
  // firmware installed.
  const vpk = await packageVpk(from, existsSync(`${share}/gxp/manifest.txt`));
  const profile = resolve(ROOT, ".pocket-build/vita3k");
  const fs = `${profile}/fs`;
  const data = `${fs}/ux0/data/pocket-duty`;
  rmSync(`${fs}/ux0/app/${TITLE}`, { recursive: true, force: true });
  mkdirSync(`${fs}/ux0/app/${TITLE}`, { recursive: true });
  await $`unzip -oq ${vpk} -d ${fs}/ux0/app/${TITLE}`.quiet();
  mkdirSync(`${fs}/ux0/user/00`, { recursive: true });
  mkdirSync(data, { recursive: true });
  writeFileSync(`${fs}/ux0/user/time.xml`, '<?xml version="1.0" encoding="utf-8"?>\n<time><user id="00" /></time>\n');
  writeFileSync(`${fs}/ux0/user/00/user.xml`, '<?xml version="1.0" encoding="utf-8"?>\n<user id="00" name="Vita3K"><theme use-background="true"><content-id>default</content-id></theme><start-screen type="default"><path></path></start-screen></user>\n');
  rmSync(`${data}/status.json`, { force: true });
  const map = opt("--map", "", from);
  writeFileSync(`${data}/boot.txt`, [`report=1 title=0`, map ? `map=${mapNamed(map).id}` : "", opt("--boot", "", from)].filter(Boolean).join(" ") + "\n");
  let config = readFileSync(base, "utf8");
  const set = (key: string, value: string) => {
    const line = new RegExp(`^${key}:.*$`, "m");
    config = line.test(config) ? config.replace(line, `${key}: ${value}`) : `${config}${key}: ${value}\n`;
  };
  for (const [key, value] of [["initial-setup", "false"], ["pref-path", JSON.stringify(`${fs}/`)], ["show-live-area-screen", "false"], ["boot-apps-full-screen", "false"], ["show-welcome", "false"], ["warn-missing-firmware", "false"], ["check-for-updates-mode", "0"], ["discord-rich-presence", "false"], ["v-sync", "false"]] as const) set(key, value);
  mkdirSync(`${profile}/config`, { recursive: true });
  writeFileSync(`${profile}/config/config.yml`, config);
  const seconds = Number(opt("--seconds", "25", from));
  console.log(`vita: Vita3K for ${seconds} s (${fs})`);
  const emulator = Bun.spawn([app, "--keep-config", "--load-config", "--config-location", `${profile}/config/config.yml`, "-r", TITLE], { cwd: ROOT, stdout: "ignore", stderr: "ignore" });
  const until = Date.now() + seconds * 1000;
  let last = "";
  while (Date.now() < until && emulator.exitCode === null) {
    await Bun.sleep(500);
    if (!existsSync(`${data}/status.json`)) continue;
    const text = readFileSync(`${data}/status.json`, "utf8");
    try {
      const said = JSON.parse(text);
      const line = `${said.stage}${said.message ? `: ${said.message}` : ""}${said.stage === "running" ? `: ${said.map}, ${said.frames} frames, ${said.tris} triangles in ${said.draws} draws` : ""}`;
      if (line !== last) console.log(`vita: ${(last = line)}`);
    } catch {}
  }
  if (emulator.exitCode === null) emulator.kill("SIGTERM");
  await emulator.exited;
  if (!existsSync(`${data}/status.json`)) throw new Error("the game wrote no status in Vita3K: see its log (~/Library/Application Support/Vita3K/Vita3K/vita3k.log)");
  console.log(readFileSync(`${data}/status.json`, "utf8"));
}

if (import.meta.main) {
  if (cmd === "build") await build();
  else if (cmd === "stage") await stage();
  else if (cmd === "serve") await dev(argv, "serve");
  else if (cmd === "run") {
    await stage();
    if (!has("--no-build")) await build();
    session(argv);
    await dev(argv, "native");
    const engine = await running(argv);
    console.log(`vita: ${engine.map} runs: ${engine.frameMs.toFixed(2)} ms a frame, ${engine.tris} triangles in ${engine.draws} draws, programs ${engine.programs.compiled} compiled and ${engine.programs.cached} from the cache`);
  } else if (cmd === "session") session(argv);
  else if (cmd === "status") console.log(JSON.stringify(status(), null, 2));
  else if (cmd === "ctl" && argv[1]) ctl(argv[1]);
  else if (cmd === "capture") await dev(argv, "capture", ...(has("--out") ? ["--out", resolve(opt("--out", ""))] : []));
  else if (cmd === "bench") await bench(argv);
  else if (cmd === "cost") await cost(argv);
  else if (cmd === "package") console.log(`vita: ${await packageVpk()}`);
  else if (cmd === "emu") await emu(argv);
  else {
    console.error('usage: bun tools/vita.ts <build|stage|serve|run|status|ctl "<words>"|capture|bench|cost|package|emu> [--map <id>] [--share DIR]');
    process.exit(1);
  }
}
