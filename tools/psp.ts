#!/usr/bin/env bun
// Pocket Duty on PSP: compile the interface (ui/) for the PSP, build the
// PRX with PocketJS's pinned rust-psp toolchain, stage both with the pack on a
// PSPLINK share, start it, steer and measure it.
//
//   bun tools/psp.ts build                    # ui/ + psp/ → dist/psp/{pocket-duty.prx,EBOOT.PBP,duty.js,duty.pak}
//   bun tools/psp.ts serve                    # start usbhostfs_pc (detached, logged) when none is running
//   bun tools/psp.ts run [--no-build]         # build, stage, reset PSPLINK, wait for it to reconnect, start the PRX
//   bun tools/psp.ts status
//   bun tools/psp.ts ctl "ui=start auto=1"   # host0:/duty/control.txt (the words are in psp/src/main.rs, `remote`:
//                                             # `mode=title|play|menu` sets the flow, `ui=start|menu|resume|restart|title`
//                                             # asks what the interface's lists ask, `press=<mask>` and `rest=<turns>`
//                                             # press PocketJS buttons on it, `pad=<held>,<x>,<y>,<frames>` holds the
//                                             # player's pad, `at=x,y,z,yaw,pitch` puts the player somewhere)
//   bun tools/psp.ts capture [--out f.png]    # PSPLINK screenshot
//   bun tools/psp.ts bench [--seconds 60]     # a match played by the bench's player: frame timings → .pocket-build/validation/psp/
//   bun tools/psp.ts tour [--seconds 3]       # every start point, four headings, turning and walking: late frames by place
//   bun tools/psp.ts film --boot "<words>" --frames 300 --out DIR [--from 2] [--bare] [--emu] [--build]
//                                             # the pictures of a run, one thirtieth of a second of the match apart,
//                                             # drawn by the console (or by PPSSPPHeadless with --emu): what
//                                             # tools/listing.ts makes the listing's clips from. --bare: no interface
//   bun tools/psp.ts package                  # dist/psp/PSP/GAME/PocketDuty for a Memory Stick: every cooked map's pack
//   bun tools/psp.ts emu [--frames 240] [--ctl "at=..."] [--out f.png] [--standalone] [--keep] [--no-interface]
//                                             # the same PRX in PPSSPPHeadless (software GE): a frame and its status;
//                                             # --keep leaves interface.json from the last run
//
// `run`, `bench`, `tour`, `film` and `emu` take `--map <id>` (maps/maps.json): the map the run starts on. Every
// cooked map's pack is staged, so the interface's map list offers them all.
//
// One usbhostfs_pc owns the PSP's cable. If one is running (in any checkout),
// these commands use its directory; `--share DIR` names another. Device
// commands take PocketJS's `psp:usb` lease; `--take` ends another holder first.

import { $ } from "bun";
import { closeSync, cpSync, existsSync, mkdirSync, openSync, readdirSync, readFileSync, readSync, rmSync, statSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { deflateSync } from "node:zlib";
import { extractHostBuildInputs, hostBuildEnvironment } from "../vendor/pocketjs/framework/src/manifest/index.ts";
import { withDeviceLease } from "../vendor/pocketjs/tools/device-lease.ts";
import { POCKET3D_ICON } from "../vendor/pocketjs/tools/pocket3d-icon.ts";
import { FIRST, mapNamed, MAPS, packPath } from "./maps.ts";
import { compileInterface, type Interface } from "./ui.ts";

const ROOT = resolve(import.meta.dir, "..");
const OUT = resolve(ROOT, "dist/psp");
const argv = process.argv.slice(2);
const cmd = argv[0] ?? "";
const opt = (key: string, fallback: string) => {
  const at = argv.indexOf(key);
  return at < 0 ? fallback : (argv[at + 1] ?? fallback);
};
const PACK = packPath(FIRST, "psp30");
// The map a run starts on (`map.txt` beside the packs): the first, or `--map <id>`.
const MAP = mapNamed(opt("--map", FIRST)).id;
const port = opt("--port", "10000");

/** The directory the running usbhostfs_pc serves, if there is one. */
async function runningShare(): Promise<string | undefined> {
  const pid = (await $`pgrep -x usbhostfs_pc`.nothrow().quiet().text()).trim().split("\n")[0];
  if (!pid) return undefined;
  const args = (await $`ps -o args= -p ${pid}`.nothrow().quiet().text()).trim().split(/\s+/);
  return args.at(-1);
}
const share = resolve(opt("--share", process.env.DUTY_PSP_SHARE ?? (await runningShare()) ?? `${ROOT}/.pocket-build/psp/host0`));
const app = `${share}/duty`;

const HOST_LOG = `${ROOT}/.pocket-build/psp/usbhostfs.log`;

/** Where the usbhostfs_pc log ends now; undefined without the log (a host started elsewhere). */
function logEnd(): number | undefined {
  return existsSync(HOST_LOG) ? statSync(HOST_LOG).size : undefined;
}

/** Whether the host has logged a new connection to the PSP since `from`. */
function reconnected(from: number): boolean {
  const size = statSync(HOST_LOG).size;
  if (size <= from) return false;
  const fd = openSync(HOST_LOG, "r");
  const buf = Buffer.alloc(Math.min(size - from, 1 << 20));
  readSync(fd, buf, 0, buf.length, size - buf.length);
  closeSync(fd);
  return buf.includes("Connected to device");
}

function encodePng(rgba: Uint8Array, w: number, h: number): Uint8Array {
  const crcTable = new Uint32Array(256).map((_, n) => {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    return c >>> 0;
  });
  const crc = (b: Uint8Array) => {
    let c = 0xffffffff;
    for (const x of b) c = crcTable[(c ^ x) & 255]! ^ (c >>> 8);
    return (c ^ 0xffffffff) >>> 0;
  };
  const chunk = (type: string, data: Uint8Array) => {
    const out = new Uint8Array(12 + data.length);
    const view = new DataView(out.buffer);
    view.setUint32(0, data.length);
    out.set(new TextEncoder().encode(type), 4);
    out.set(data, 8);
    view.setUint32(8 + data.length, crc(out.subarray(4, 8 + data.length)));
    return out;
  };
  const head = new Uint8Array(13);
  const hv = new DataView(head.buffer);
  hv.setUint32(0, w);
  hv.setUint32(4, h);
  head.set([8, 6, 0, 0, 0], 8);
  const raw = new Uint8Array((w * 4 + 1) * h);
  for (let y = 0; y < h; y++) raw.set(rgba.subarray(y * w * 4, (y + 1) * w * 4), y * (w * 4 + 1) + 1);
  return Buffer.concat([new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10]), chunk("IHDR", head), chunk("IDAT", deflateSync(raw)), chunk("IEND", new Uint8Array(0))]);
}

/** The interface's bundle and its pak: the program reads them beside the pack. */
const UI_FILES = ["duty.js", "duty.pak"];

/** The interface compiled for the PSP; the last compiled one when `ui/` does not compile just now. */
async function interfaceBundle(): Promise<Interface> {
  try {
    return await compileInterface("psp");
  } catch (e) {
    const directory = `${ROOT}/.pocket-build/ui/psp`;
    if (!UI_FILES.every((f) => existsSync(`${directory}/${f}`)) || !existsSync(`${directory}/plan.json`)) throw e;
    console.warn(`psp: ui/ did not compile (${String(e).split("\n")[0]}); using the bundle already in ${directory}`);
    const plan = JSON.parse(readFileSync(`${directory}/plan.json`, "utf8"));
    return { directory, plan, inputs: extractHostBuildInputs(plan) };
  }
}

/** A PARAM.SFO: keys in order, 32-bit integers and NUL-terminated strings padded to four bytes. */
function paramSfo(values: Record<string, number | string>): Buffer {
  const keys = Object.keys(values).sort();
  const data = keys.map((key) => {
    const value = values[key]!;
    if (typeof value === "number") {
      const bytes = Buffer.alloc(4);
      bytes.writeUInt32LE(value);
      return { format: 0x0404, used: 4, bytes };
    }
    const text = Buffer.from(value + "\0");
    return { format: 0x0204, used: text.length, bytes: Buffer.concat([text, Buffer.alloc((4 - (text.length % 4)) % 4)]) };
  });
  const names = Buffer.from(keys.map((key) => key + "\0").join(""));
  const keyTable = 20 + keys.length * 16;
  const dataTable = keyTable + Math.ceil(names.length / 4) * 4;
  const out = Buffer.alloc(dataTable + data.reduce((sum, d) => sum + d.bytes.length, 0));
  out.write("\0PSF");
  out.writeUInt32LE(0x101, 4);
  out.writeUInt32LE(keyTable, 8);
  out.writeUInt32LE(dataTable, 12);
  out.writeUInt32LE(keys.length, 16);
  let nameAt = 0;
  let dataAt = 0;
  keys.forEach((key, i) => {
    const at = 20 + i * 16;
    const d = data[i]!;
    out.writeUInt16LE(nameAt, at);
    out.writeUInt16LE(d.format, at + 2);
    out.writeUInt32LE(d.used, at + 4);
    out.writeUInt32LE(d.bytes.length, at + 8);
    out.writeUInt32LE(dataAt, at + 12);
    d.bytes.copy(out, dataTable + dataAt);
    nameAt += key.length + 1;
    dataAt += d.bytes.length;
  });
  names.copy(out, keyTable);
  return out;
}

/**
 * Packs the PRX as an EBOOT. `large` asks for the 52 MB of a PSP-2000 or later (cargo-psp has no
 * setting for it): the street's pack alone is larger than a PSP-1000's 24 MB. ICON0.PNG is the
 * Pocket3D icon from PocketJS; PIC1.PNG is a capture of this game, once one is in psp/assets.
 */
async function pbp(out: string, prx: string, large: boolean) {
  const sfo = `${out}.SFO`;
  writeFileSync(sfo, paramSfo({ BOOTABLE: 1, CATEGORY: "MG", DISC_VERSION: "1.00", ...(large ? { MEMSIZE: 1 } : {}), PARENTAL_LEVEL: 1, PSP_SYSTEM_VER: "1.00", REGION: 0x8000, TITLE: "Pocket Duty" }));
  const pic1 = `${ROOT}/psp/assets/pic1.png`;
  await $`pack-pbp ${out} ${sfo} ${POCKET3D_ICON.psp} NULL NULL ${existsSync(pic1) ? pic1 : "NULL"} NULL ${prx} NULL`.quiet();
  rmSync(sfo, { force: true });
}

async function build() {
  const ui = await interfaceBundle();
  // Loaded by path at run time: PocketJS's toolchain module resolves its manifest through its own tsconfig.
  const toolchain: string = `${ROOT}/vendor/pocketjs/tools/psp-toolchain.ts`;
  const tc = (await import(toolchain)).resolvePspBuildToolchain();
  // The interface's runtime (PocketJS's PSP host library) builds QuickJS from C for the same target, with
  // PocketJS's own flags for it, and checks the target it was compiled for against the interface's plan.
  await $`${tc.rustup} run ${tc.manifest.rust.toolchain} cargo psp --release`.cwd(`${ROOT}/psp`).env({
    ...tc.environment,
    RUSTFLAGS: "-A linker-messages -A unexpected-cfgs -A unstable-name-collisions",
    CRATE_CC_NO_DEFAULTS: "1",
    TARGET_CC: "clang",
    TARGET_AR: `${tc.llvmBin}/llvm-ar`,
    TARGET_CFLAGS:
      `-target mipsel-sony-psp -mcpu=mips2 -msingle-float -mlittle-endian -mno-abicalls -fno-pic -G0 -mno-check-zero-division ` +
      `-fno-stack-protector -O2 -I${tc.sdk.path}/psp/include -I${tc.sdk.path}/psp/sdk/include`,
    AR_mipsel_sony_psp: `${tc.llvmBin}/llvm-ar`,
    RANLIB_mipsel_sony_psp: `${tc.llvmBin}/llvm-ranlib`,
    ...hostBuildEnvironment(ui.inputs, { outputDirectory: ui.directory, embedApp: false }),
    POCKETJS_OFFLOAD_SLOT: "",
    RUST_PSP_ABORT_ONLY: "1",
    RUST_PSP_TARGET: `${ROOT}/vendor/pocketjs/hosts/psp/targets/mipsel-sony-psp.json`,
  });
  const from = `${ROOT}/psp/target/mipsel-sony-psp/release`;
  mkdirSync(OUT, { recursive: true });
  cpSync(`${from}/pocket-duty-psp.prx`, `${OUT}/pocket-duty.prx`);
  await pbp(`${OUT}/EBOOT.PBP`, `${OUT}/pocket-duty.prx`, true);
  for (const f of UI_FILES) cpSync(`${ui.directory}/${f}`, `${OUT}/${f}`);
  console.log(`psp: ${OUT}/pocket-duty.prx ${(readFileSync(`${OUT}/pocket-duty.prx`).length / 1024).toFixed(0)} KiB, interface ${UI_FILES.map((f) => `${f} ${(readFileSync(`${OUT}/${f}`).length / 1024).toFixed(0)} KiB`).join(", ")}`);
}

/** Puts the interface's files in `dir` when they differ from the built ones. */
function stageInterface(dir: string) {
  for (const f of UI_FILES) {
    if (!existsSync(`${OUT}/${f}`)) throw new Error(`no ${OUT}/${f}: run \`bun tools/psp.ts build\` first`);
    if (!existsSync(`${dir}/${f}`) || sha(`${dir}/${f}`) !== sha(`${OUT}/${f}`)) cpSync(`${OUT}/${f}`, `${dir}/${f}`);
  }
}

function sha(path: string): string {
  const h = new Bun.CryptoHasher("sha256");
  h.update(readFileSync(path));
  return h.digest("hex");
}

/** A pack's identity as the cook wrote it, so an unchanged pack is not copied again. */
function packId(pack = PACK): string {
  const receipt = resolve(pack, "../receipt.json");
  return existsSync(receipt) ? JSON.parse(readFileSync(receipt, "utf8")).pack.sha256 : sha(pack);
}

/** The packs of the maps that are cooked, under the names the program opens them by (`Map::pack_name`). */
function packs(): { id: string; file: string; name: string }[] {
  return MAPS.flatMap((map) => (existsSync(packPath(map.id, "psp30")) ? [{ id: map.id, file: packPath(map.id, "psp30"), name: map.id === FIRST ? "duty" : map.id }] : []));
}

/**
 * Puts every cooked map's pack in `dir`, and the name of the map the run starts on (`map.txt`: `MAP`, or
 * none for a copy a person starts). Returns the first map's pack's identity.
 */
function stage(dir: string, map: string | null = MAP) {
  if (!existsSync(PACK)) throw new Error(`no pack at ${PACK}: run \`bun tools/duty.ts cook\` first`);
  if (map && !existsSync(packPath(map, "psp30"))) throw new Error(`no pack at ${packPath(map, "psp30")}: run \`bun tools/duty.ts cook --map ${map}\` first`);
  mkdirSync(dir, { recursive: true });
  for (const pack of packs()) {
    const id = packId(pack.file);
    const mark = `${dir}/${pack.name}.json`;
    if (!existsSync(`${dir}/${pack.name}.pack`) || !existsSync(mark) || JSON.parse(readFileSync(mark, "utf8")).sha256 !== id) {
      cpSync(pack.file, `${dir}/${pack.name}.pack`);
      writeFileSync(mark, JSON.stringify({ sha256: id, bytes: statSync(pack.file).size }));
    }
  }
  if (map) writeFileSync(`${dir}/map.txt`, map);
  else rmSync(`${dir}/map.txt`, { force: true });
  return packId();
}

/** Takes the packs and their marks out of a directory a run does not read them from. */
function unstage(dir: string) {
  for (const map of MAPS) for (const end of ["pack", "json"]) rmSync(`${dir}/${map.id === FIRST ? "duty" : map.id}.${end}`, { force: true });
  rmSync(`${dir}/map.txt`, { force: true });
}

async function pspsh(text: string): Promise<string> {
  const p = Bun.spawn(["pspsh", "-p", port, "-e", text], { stdout: "pipe", stderr: "pipe" });
  const timer = setTimeout(() => p.kill(), 15000);
  const [out, err] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text(), p.exited]);
  clearTimeout(timer);
  if (/Error|Could not|failed|connect:/i.test(out + err)) throw new Error(`pspsh ${text}: ${(out + err).trim()}`);
  return out + err;
}

function readStatus(): any {
  // The device rewrites the file in place; a read can land in the middle. Its storage thread runs
  // below the frame's: while frames come late the file can stand empty for seconds.
  for (let i = 0; ; i++) {
    try {
      return JSON.parse(readFileSync(`${app}/status.json`, "utf8"));
    } catch (e) {
      if (i >= 400) throw new Error(`no status at ${app}/status.json: is the program running?`);
      Bun.sleepSync(25);
    }
  }
}

async function waitFor(test: (s: any) => boolean, seconds: number): Promise<any> {
  const end = Date.now() + seconds * 1000;
  let last: any;
  while (Date.now() < end) {
    try {
      last = readStatus();
      if (last.stage === "failed") throw new Error(`the device reports: ${last.error} (${last.code})`);
      if (test(last)) return last;
    } catch (e) {
      if (String(e).includes("the device reports")) throw e;
    }
    await Bun.sleep(300);
  }
  throw new Error(`the PSP did not get there in ${seconds} s (last status: ${JSON.stringify(last)})`);
}

function ctl(text: string) {
  mkdirSync(app, { recursive: true });
  // The nonce makes two equal commands differ.
  writeFileSync(`${app}/control.txt`, `${text} nonce=${Date.now()}\n`);
}

async function capture(out: string) {
  const name = `capture-${Date.now()}.bmp`;
  await pspsh(`scrshot host0:/${name}`);
  const path = `${share}/${name}`;
  for (let i = 0; i < 40 && !existsSync(path); i++) await Bun.sleep(100);
  const bmp = readFileSync(path);
  rmSync(path, { force: true });
  // BITMAPINFOHEADER, 24 or 32 bits, rows bottom-up unless the height is negative.
  const view = new DataView(bmp.buffer, bmp.byteOffset, bmp.byteLength);
  const at = view.getUint32(10, true);
  const w = view.getInt32(18, true);
  const hRaw = view.getInt32(22, true);
  const h = Math.abs(hRaw);
  const bpp = view.getUint16(28, true) / 8;
  const stride = (w * bpp + 3) & ~3;
  const rgba = new Uint8Array(w * h * 4);
  for (let y = 0; y < h; y++) {
    const row = at + (hRaw > 0 ? h - 1 - y : y) * stride;
    for (let x = 0; x < w; x++) {
      const s = row + x * bpp;
      rgba.set([bmp[s + 2]!, bmp[s + 1]!, bmp[s]!, 255], (y * w + x) * 4);
    }
  }
  mkdirSync(resolve(out, ".."), { recursive: true });
  writeFileSync(out, encodePng(rgba, w, h));
  console.log(out);
}

async function bench(seconds: number, extra: string) {
  // A match as a person sees it, the interface over it, played by the bench's player: he turns to the
  // nearest soldier in sight and fires, so soldiers, effects and sounds are all in the frames measured.
  ctl(`ui=title`);
  await Bun.sleep(1500);
  ctl(`ui=start auto=1 god=0 ${extra}`);
  await Bun.sleep(4000);
  const first = readStatus();
  const build = JSON.parse(readFileSync(`${app}/build.json`, "utf8"));
  const samples: any[] = [];
  const start = Date.now();
  let prev = first;
  while (Date.now() - start < seconds * 1000) {
    await Bun.sleep(1000);
    const s = readStatus();
    if (s.frames === prev.frames) continue;
    samples.push({ t: (Date.now() - start) / 1000, frameMs: s.frameMs, worstMs: s.worstMs, late: s.late, frames: s.frames, cpuMs: s.cpuMs, simUs: s.simUs, selectUs: s.selectUs, listUs: s.listUs, gpuMs: s.gpuMs, geMs: s.geMs, busyMs: s.busyMs, draws: s.draws, tris: s.tris, figureTris: s.figureTris, clusters: s.clusters, cut: s.cut, nearer: s.nearer, eye: s.eye, match: s.match, interface: s.interface });
    prev = s;
  }
  ctl("auto=0");
  if (samples.length < 2) throw new Error("no samples: is the program on screen?");
  const last = samples.at(-1)!;
  const mean = (f: (s: any) => number) => samples.reduce((n, s) => n + f(s), 0) / samples.length;
  const frames = last.frames - first.frames;
  const late = last.late - first.late;
  const summary = {
    seconds,
    frames,
    lateFrames: late,
    lateShare: late / Math.max(frames, 1),
    averageFrameMs: mean((s) => s.frameMs),
    fps: 1000 / mean((s) => s.frameMs),
    worstFrameMs: Math.max(...samples.map((s) => s.worstMs)),
    triangles: { least: Math.min(...samples.map((s) => s.tris)), mean: Math.round(mean((s) => s.tris)), most: Math.max(...samples.map((s) => s.tris)) },
    draws: { mean: Math.round(mean((s) => s.draws)), most: Math.max(...samples.map((s) => s.draws)) },
    cpuMs: { sim: mean((s) => s.cpuMs.sim), select: mean((s) => s.cpuMs.select), list: mean((s) => s.cpuMs.list) },
    // Microseconds a frame. The match: the player and his rounds, the ways, the soldiers, the rest.
    simUs: [0, 1, 2, 3].map((k) => Math.round(mean((s) => s.simUs?.[k] ?? 0))),
    // The list: sky, near pass, its shadows, far pass, its shadows, glass, soldiers, effects, held weapon, interface.
    listUs: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9].map((k) => Math.round(mean((s) => s.listUs?.[k] ?? 0))),
    selectUs: Math.round(mean((s) => s.selectUs ?? 0)),
    gpuWaitMs: mean((s) => s.gpuMs),
    // From the list's first word to the GE's last, in the frames the GE was waited for; and what a
    // frame took of the CPU and the GE without its interface turn.
    geMs: mean((s) => s.geMs ?? 0),
    busyMs: mean((s) => s.busyMs ?? 0),
    nearer: { least: Math.min(...samples.map((s) => s.nearer)), mean: mean((s) => s.nearer) },
    match: last.match,
    interface: last.interface && {
      up: last.interface.up,
      turnsPerSecond: (last.interface.turns - first.interface.turns) / seconds,
      turnMs: { script: mean((s) => s.interface.turnMs.script), layout: mean((s) => s.interface.turnMs.layout), worst: last.interface.turnMs.worst },
      cpuMsPerFrame: mean((s) => s.interface.cpuMs),
      listWords: Math.max(...samples.map((s) => s.interface.words)),
      bytes: last.interface.bytes,
    },
    control: extra,
  };
  const dir = resolve(ROOT, `.pocket-build/validation/psp/bench-${new Date().toISOString().replace(/[:.]/g, "-")}`);
  mkdirSync(dir, { recursive: true });
  writeFileSync(`${dir}/device.json`, JSON.stringify({ summary, identity: { device: "psp:usb", ...build }, memory: readStatus().memory, samples }, null, 1));
  console.log(`${dir}/device.json`);
  console.log(JSON.stringify(summary, null, 1));
}

/** The level's start points (`cargo run --release -p duty-sim --example grid -- <pack>` prints them). */
const STARTS = [[13.71, 0.01, 19.84], [15.54, 0.01, 27.64], [6.67, 0.01, 3.51], [-8.58, 0.01, -6.76], [-14.68, 0.01, -26.73], [-20.8, 0.01, -30.4], [12.19, 0.05, -0.65], [-12.25, 0.05, -2.16]];

async function tour(seconds: number) {
  // A player who cannot fall stands at each start point looking four ways in turn, and from each
  // walks ahead while the view turns: what the bench's player, who stands still, does not ask of
  // the frame. The east face button (32) turns the view.
  ctl("ui=title");
  await Bun.sleep(1500);
  ctl("ui=start god=1 auto=0");
  await Bun.sleep(4000);
  const rows: any[] = [];
  for (const [place, p] of STARTS.entries()) {
    for (const yaw of [0, 90, 180, 270]) {
      ctl(`at=${p[0]},${p[1]},${p[2]},${yaw},-4 pad=32,0,-100,${seconds * 30}`);
      await Bun.sleep(600);
      const first = readStatus();
      const samples: any[] = [];
      const end = Date.now() + seconds * 1000 - 600;
      while (Date.now() < end) {
        await Bun.sleep(400);
        samples.push(readStatus());
      }
      const last = samples.at(-1) ?? first;
      const mean = (f: (s: any) => number) => samples.reduce((n, s) => n + f(s), 0) / Math.max(samples.length, 1);
      const row = { place, yaw, frames: last.frames - first.frames, late: last.late - first.late, busyMs: mean((s) => s.busyMs), geMs: mean((s) => s.geMs), cpuMs: { sim: mean((s) => s.cpuMs.sim), select: mean((s) => s.cpuMs.select), list: mean((s) => s.cpuMs.list), interface: mean((s) => s.interface.cpuMs) }, tris: Math.round(mean((s) => s.tris)), draws: Math.round(mean((s) => s.draws)), nearer: mean((s) => s.nearer) };
      rows.push(row);
      console.log(`place ${place} yaw ${String(yaw).padStart(3)}: ${String(row.late).padStart(3)} late of ${row.frames} | busy ${row.busyMs.toFixed(1)} ge ${row.geMs.toFixed(1)} | sim ${row.cpuMs.sim.toFixed(1)} select ${row.cpuMs.select.toFixed(1)} list ${row.cpuMs.list.toFixed(1)} interface ${row.cpuMs.interface.toFixed(1)} | ${row.tris} triangles in ${row.draws} draws, levels at ${row.nearer.toFixed(2)}`);
    }
  }
  ctl("god=0");
  const frames = rows.reduce((n, r) => n + r.frames, 0);
  const late = rows.reduce((n, r) => n + r.late, 0);
  const build = JSON.parse(readFileSync(`${app}/build.json`, "utf8"));
  const dir = resolve(ROOT, `.pocket-build/validation/psp/tour-${new Date().toISOString().replace(/[:.]/g, "-")}`);
  mkdirSync(dir, { recursive: true });
  writeFileSync(`${dir}/device.json`, JSON.stringify({ summary: { frames, lateFrames: late, lateShare: late / Math.max(frames, 1) }, identity: { device: "psp:usb", ...build }, rows }, null, 1));
  console.log(`${dir}/device.json`);
  console.log(`tour: ${late} late of ${frames} frames (${((late / Math.max(frames, 1)) * 100).toFixed(2)}%)`);
}

/** Runs `f` holding the PSP's lease; `--take` ends another holder first. */
async function device<T>(f: () => Promise<T>): Promise<T> {
  for (let attempt = 0; ; attempt++) {
    try {
      return await withDeviceLease("psp:usb", f);
    } catch (e) {
      const m = /owner pid=(\d+)/.exec(String(e));
      if (!m || !argv.includes("--take") || attempt > 0) throw e;
      console.log(`psp: ending the holder of psp:usb (pid ${m[1]})`);
      process.kill(Number(m[1]), "SIGTERM");
      await Bun.sleep(800);
    }
  }
}

/**
 * Stages the built program, the pack and the interface on the share with `boot` as `boot.txt`, starts it under
 * PSPLINK (after a reset when a program is loaded) and returns its first status. `bare` leaves the interface's
 * files off the share: the street with nothing drawn over it.
 */
async function start(boot: string, bare = false): Promise<any> {
  const id = stage(app);
  cpSync(`${OUT}/pocket-duty.prx`, `${share}/pocket-duty.prx`);
  if (bare) for (const f of UI_FILES) rmSync(`${app}/${f}`, { force: true });
  else stageInterface(app);
  writeFileSync(`${app}/build.json`, JSON.stringify({ prxSha256: sha(`${OUT}/pocket-duty.prx`), packSha256: id, interfaceSha256: Object.fromEntries(UI_FILES.map((f) => [f, sha(`${OUT}/${f}`)])) }, null, 1));
  writeFileSync(`${app}/boot.txt`, `${boot}\n`);
  rmSync(`${app}/status.json`, { force: true });
  // A reset restarts PSPLINK from the Memory Stick and drops the cable for several seconds. A command sent
  // before the cable is back leaves PSPLINK half reset (the shell answers, storage does not) until someone
  // restarts it on the console. So: reset, then nothing until usbhostfs_pc logs a new connection.
  // PSPLINK lists its own modules last: with nothing after USBHostFS, no program is loaded and no reset is needed.
  const names = (await pspsh("modlist").catch(() => "")).split("\n").filter((l) => l.includes("Name:")).map((l) => l.split("Name:")[1]!.trim());
  const idle = names.length > 0 && (names.at(-1) === "USBHostFS" || names.at(-1) === "PSPLINK");
  const before = logEnd();
  if (idle) {
    console.log("psp: PSPLINK is idle; loading without a reset");
  } else {
    console.log(`psp: resetting PSPLINK (${names.at(-1) ?? "module list unreadable"})`);
    await pspsh("reset").catch(() => "");
    if (before === undefined) {
      console.log("psp: no usbhostfs_pc log here (the host was started elsewhere); waiting 15 s for PSPLINK");
      await Bun.sleep(15000);
    } else {
      const end = Date.now() + 40000;
      while (!reconnected(before)) {
        if (Date.now() > end) throw new Error("PSPLINK did not reconnect after the reset: restart PSPLINK on the console");
        await Bun.sleep(250);
      }
      await Bun.sleep(1000);
    }
  }
  if (!/duty\.pack/.test(await pspsh("ls host0:/duty"))) throw new Error("PSPLINK does not serve host0: restart PSPLINK on the console");
  await pspsh("ldstart host0:/pocket-duty.prx");
  const s = await waitFor((s) => s.stage === "running", 180);
  return s;
}

switch (cmd) {
  case "build":
    await build();
    break;
  case "run":
    if (!argv.includes("--no-build")) await build();
    await device(async () => console.log(JSON.stringify(await start(opt("--boot", "title=0")), null, 1)));
    break;
  case "film": {
    // A film: `--frames` pictures from frame `--from` of a run that `--boot` words steer, one thirtieth of a
    // second of the match apart. The console draws each one and writes it to the share; `--emu` has
    // PPSSPPHeadless draw them. They go to `--out` as 00000.raw onwards: 480 × 272 texels of 16 bits.
    if (argv.includes("--build")) await build();
    const frames = Number(opt("--frames", "300"));
    const from = Number(opt("--from", "2"));
    const bare = argv.includes("--bare");
    const out = resolve(opt("--out", `${ROOT}/.pocket-build/film/${Date.now()}`));
    const boot = `title=0 @${from}:film=${frames} ${opt("--boot", "")}`.trim();
    // The program reads this many bytes of boot.txt (`boot_buf` in psp/src/main.rs); `exit=` is added for the emulator.
    const BOOT_MAX = 2048;
    if (boot.length + 16 > BOOT_MAX) throw new Error(`the film's words are ${boot.length} bytes; boot.txt holds ${BOOT_MAX - 16}`);
    const bytes = 480 * 272 * 2;
    const take = (dir: string) => {
      rmSync(out, { recursive: true, force: true });
      mkdirSync(out, { recursive: true });
      for (let i = 0; i < frames; i++) {
        const name = `${String(i).padStart(5, "0")}.raw`;
        if (!existsSync(`${dir}/${name}`) || statSync(`${dir}/${name}`).size !== bytes) throw new Error(`the film stops at frame ${i} of ${frames}`);
        cpSync(`${dir}/${name}`, `${out}/${name}`);
      }
      rmSync(dir, { recursive: true, force: true });
    };
    if (argv.includes("--emu")) {
      const headless = process.env.PPSSPP_HEADLESS ?? `${process.env.HOME}/ppsspp-src/build/PPSSPPHeadless`;
      if (!existsSync(headless)) throw new Error(`no PPSSPPHeadless at ${headless} (set PPSSPP_HEADLESS)`);
      const root = `${ROOT}/.pocket-build/psp/emu`;
      mkdirSync(`${root}/duty`, { recursive: true });
      stage(`${root}/duty`);
      for (const f of [...UI_FILES, "interface.json", "status.json"]) for (const dir of [root, `${root}/duty`]) rmSync(`${dir}/${f}`, { force: true });
      unstage(root);
      rmSync(`${root}/duty/film`, { recursive: true, force: true });
      mkdirSync(`${root}/duty/film`, { recursive: true });
      if (!bare) stageInterface(`${root}/duty`);
      cpSync(`${OUT}/EBOOT.PBP`, `${root}/EBOOT.PBP`);
      writeFileSync(`${root}/duty/boot.txt`, `exit=${from + frames + 3} ${boot}\n`);
      await $`${headless} --root ${root} --graphics=${opt("--graphics", "software")} --timeout=${opt("--timeout", "300")} ${root}/EBOOT.PBP`.nothrow().quiet();
      take(`${root}/duty/film`);
      console.log(JSON.stringify({ out, frames, drawnBy: "PPSSPPHeadless" }));
      break;
    }
    await device(async () => {
      rmSync(`${app}/film`, { recursive: true, force: true });
      mkdirSync(`${app}/film`, { recursive: true });
      const began = Date.now();
      await start(boot, bare);
      // The last frame is whole when its file has every byte. A frame takes the cable a fraction of a second.
      const last = `${app}/film/${String(frames - 1).padStart(5, "0")}.raw`;
      const end = Date.now() + 60000 + frames * 2500;
      while (!existsSync(last) || statSync(last).size !== bytes) {
        if (Date.now() > end) throw new Error(`the PSP wrote ${existsSync(`${app}/film`) ? readdirSync(`${app}/film`).length : 0} of ${frames} frames`);
        await Bun.sleep(500);
      }
      await Bun.sleep(500);
      const status = readStatus();
      take(`${app}/film`);
      console.log(JSON.stringify({ out, frames, drawnBy: "psp:usb", seconds: Math.round((Date.now() - began) / 1000), mode: status.mode, match: status.match }));
    });
    break;
  }
  case "serve": {
    // One usbhostfs_pc owns the cable. This one outlives the command and logs where `run` can count its connections.
    if ((await $`pgrep -x usbhostfs_pc`.nothrow().quiet()).exitCode === 0) throw new Error("a usbhostfs_pc is already running; stop it first, or pass its directory with --share");
    mkdirSync(share, { recursive: true });
    mkdirSync(resolve(HOST_LOG, ".."), { recursive: true });
    const { spawn } = await import("node:child_process");
    const log = openSync(HOST_LOG, "w");
    spawn("usbhostfs_pc", ["-b", port, share], { cwd: share, detached: true, stdio: ["ignore", log, log] }).unref();
    console.log(`psp: usbhostfs_pc serves ${share} (log ${HOST_LOG})`);
    break;
  }
  case "status":
    console.log(JSON.stringify(readStatus(), null, 1));
    break;
  case "ctl":
    ctl(argv[1] ?? "");
    break;
  case "capture":
    await device(() => capture(resolve(opt("--out", `${ROOT}/.pocket-build/validation/psp/capture-${Date.now()}.png`))));
    break;
  case "bench":
    await device(() => bench(Number(opt("--seconds", "60")), opt("--ctl", "")));
    break;
  case "tour":
    await device(() => tour(Number(opt("--seconds", "3"))));
    break;
  case "emu": {
    // PPSSPP mounts `--root` as host0:. Its software renderer follows the GE's clipping rules.
    const headless = process.env.PPSSPP_HEADLESS ?? `${process.env.HOME}/ppsspp-src/build/PPSSPPHeadless`;
    if (!existsSync(headless)) throw new Error(`no PPSSPPHeadless at ${headless} (set PPSSPP_HEADLESS)`);
    const root = `${ROOT}/.pocket-build/psp/emu`;
    // `--standalone` puts the pack beside the EBOOT, as on a Memory Stick, instead of on the share.
    const standalone = argv.includes("--standalone");
    mkdirSync(`${root}/duty`, { recursive: true });
    unstage(standalone ? `${root}/duty` : root);
    stage(standalone ? root : `${root}/duty`);
    // The interface's files and what it kept go where the pack is.
    // `--keep` leaves what the interface kept in the last run (`interface.json`) for this one to read.
    for (const f of [...UI_FILES, ...(argv.includes("--keep") ? [] : ["interface.json"])]) for (const dir of [root, `${root}/duty`]) rmSync(`${dir}/${f}`, { force: true });
    if (!argv.includes("--no-interface")) stageInterface(standalone ? root : `${root}/duty`);
    cpSync(`${OUT}/EBOOT.PBP`, `${root}/EBOOT.PBP`);
    const frames = Number(opt("--frames", "240"));
    for (const f of ["status.json", "shot.raw"]) rmSync(`${root}/duty/${f}`, { force: true });
    writeFileSync(`${root}/duty/boot.txt`, `title=0 ${opt("--ctl", "")} shot=${frames} exit=${frames + 3}\n`);
    const run = await $`${headless} --root ${root} --graphics=${opt("--graphics", "software")} --timeout=${opt("--timeout", "240")} ${root}/EBOOT.PBP`.nothrow().quiet();
    const log = (run.stdout.toString() + run.stderr.toString()).trim();
    if (log) console.log(log.split("\n").slice(-12).join("\n"));
    if (existsSync(`${root}/duty/status.json`)) console.log(readFileSync(`${root}/duty/status.json`, "utf8"));
    if (!existsSync(`${root}/duty/shot.raw`)) throw new Error("the emulator run wrote no frame");
    const raw = readFileSync(`${root}/duty/shot.raw`);
    const rgba = new Uint8Array(480 * 272 * 4);
    // The frame buffer is 16-bit: red in the low five bits, then six of green, five of blue.
    for (let i = 0; i < 480 * 272; i++) {
      const p = raw[i * 2]! | (raw[i * 2 + 1]! << 8);
      rgba.set([((p & 31) * 255) / 31, (((p >> 5) & 63) * 255) / 63, ((p >> 11) * 255) / 31, 255], i * 4);
    }
    const out = resolve(opt("--out", `${ROOT}/.pocket-build/psp/emu/frame.png`));
    writeFileSync(out, encodePng(rgba, 480, 272));
    console.log(out);
    break;
  }
  case "package": {
    const dir = `${OUT}/PSP/GAME/PocketDuty`;
    mkdirSync(dir, { recursive: true });
    cpSync(`${OUT}/EBOOT.PBP`, `${dir}/EBOOT.PBP`);
    // (every cooked map's pack; no `map.txt`: a copy opens on the first map until a person chooses another)
    stage(dir, null);
    stageInterface(dir);
    for (const pack of packs()) rmSync(`${dir}/${pack.name}.json`, { force: true });
    console.log(`psp: copy ${OUT}/PSP to the root of a Memory Stick`);
    break;
  }
  default:
    console.log("usage: bun tools/psp.ts <build|serve|run|status|ctl|capture|bench|tour|film|emu|package> [--share DIR] [--take]");
    process.exit(cmd ? 1 : 0);
}
