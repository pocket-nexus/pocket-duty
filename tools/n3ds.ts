#!/usr/bin/env bun
// Pocket Duty on Nintendo 3DS: build the Rust core, the interface (ui/) and
// the C host into a .3dsx (devkitARM in PocketJS's pinned container), install
// and start it over PocketJS's paired LAN wire, steer and measure it.
//
//   bun tools/n3ds.ts build [--no-ui] [--embed street|none]
//                                               # dist/3ds/pocket-duty.3dsx with the interface in its ROMFS and one
//                                               # map's pack as pieces there (the first map's; `none` for a
//                                               # program that reads every pack from the card)
//                                               # (--no-ui keeps the interface as last compiled; --map <id>
//                                               # is --embed <id>)
//   bun tools/n3ds.ts card <directory>          # what goes on a memory card, under <directory>: the .3dsx and
//                                               # every cooked map's pack as pieces (3ds/pocket-duty/packs/),
//                                               # brought up to date with the pieces <directory> lacks
//   bun tools/n3ds.ts install [--no-build]      # build, send, start, wait for the program to report
//   bun tools/n3ds.ts status
//   bun tools/n3ds.ts ctl "ui=start auto=1"     # the core's words (n3ds/core/src/lib.rs `dt_control`: mode=, ui=, pad=,
//                                               # at=, near=, auto=, god=, hurt=, clock=, skill=, budget=, option=), and:
//                                               #   press=MASK (PocketJS BTN bits on the interface; 0 rests)
//                                               #   touch=X,Y | touch=off (a stylus on the lower screen)
//   bun tools/n3ds.ts capture [--out f.png] [--surface top|auxiliary]
//   bun tools/n3ds.ts bench [--seconds 60] [--install]   # a match played by the bench's player → .pocket-build/validation/3ds/
//   bun tools/n3ds.ts emu [--frames 90] [--ctl "at=..."] [--out f.png] [--prefs JSON] [--no-card] [--map <id>]
//                                               # the same .3dsx in Azahar (software PICA): both screens of a
//                                               # frame (the lower one under the upper) and the status. Every
//                                               # cooked map's pack is on the emulated card unless --no-card;
//                                               # --map changes to that map at the start
//
// `--host ADDRESS` (default 192.168.8.159, or POCKET_3DS_HOST). The console
// must run a Pocket Runtime build with the paired wire: this program itself
// once installed, or another Pocket Nexus .3dsx. Installs are .3dsx only.

import { $ } from "bun";
import { cpSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
import { deflateSync } from "node:zlib";
import { pocketRuntimeDeviceId } from "../vendor/pocketjs/contracts/spec/pocket-runtime-wire.ts";
import { extractHostBuildInputs } from "../vendor/pocketjs/framework/src/manifest/index.ts";
import { withDeviceLease } from "../vendor/pocketjs/tools/device-lease.ts";
import { ensureQuickJs, runContainer, THREE_DS_CONTAINER_IMAGE } from "../vendor/pocketjs/tools/3ds-toolchain.ts";
import { bring, writePacks } from "../vendor/pocketjs/tools/pocket3d-packs.ts";
import { FIRST, mapNamed, packPath } from "./maps.ts";
import { bringMaps, cooked, said } from "./packs.ts";
import { compileInterface } from "./ui.ts";

const ROOT = resolve(import.meta.dir, "..");
const POCKETJS = join(ROOT, "vendor/pocketjs");
const DIR = join(ROOT, ".pocket-build/3ds");
const ARTIFACT = join(ROOT, "dist/3ds/pocket-duty.3dsx");
const NAME = "pocket-duty.3dsx";
const RECEIPTS = join(ROOT, ".pocket-build/validation/3ds");
/** Pocket3D's app icon for the 3DS (48 and 24 pixels), from the repository's root: `ICONS` in n3ds/Makefile. */
const ICONS = "vendor/pocketjs/engine/pocket3d/icon/3ds";
/** The compiler of n3ds/core (its rust-toolchain.toml names the same). */
const TOOLCHAIN = "nightly-2026-07-02";
const argv = process.argv.slice(2);
const cmd = argv[0] ?? "";
const opt = (key: string, fallback: string) => {
  const at = argv.indexOf(key);
  return at < 0 ? fallback : (argv[at + 1] ?? fallback);
};
// The maps' packs are pieces named by their bytes with one manifest (tools/packs.ts). The program reads the
// card's copy when it is there (`sdmc:/3ds/pocket-duty/packs/`, every cooked map: `card`), and otherwise the
// one map the `.3dsx` carries in its ROMFS (`--embed`, the first map's). The paired wire installs a file of
// 32 MiB at most, so the `.3dsx` carries one pack or none; the interface lists every map that is cooked,
// and the program offers the ones it finds a whole pack for.
const EMBED = opt("--embed", opt("--map", FIRST));
const HELD = cooked("n3ds30").map((each) => mapNamed(each.id));
/** Where the card's files lie on it. */
const CARD = "3ds/pocket-duty";
const host = opt("--host", process.env.POCKET_3DS_HOST ?? "192.168.8.159");

// Loaded by path at run time: PocketJS's client module is checked by PocketJS's own compiler settings.
const clientModule: string = join(POCKETJS, "tools/3ds-runtime-client.ts");
const { discoverPocketRuntimes, parsePocketRuntimeToken, PocketRuntimeClient } = await import(clientModule);
type Client = any;

const sha = (path: string) => {
  const h = new Bun.CryptoHasher("sha256");
  h.update(readFileSync(path));
  return h.digest("hex");
};

function encodePng(rgba: Uint8Array, w: number, h: number): Uint8Array {
  const table = new Uint32Array(256).map((_, n) => {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    return c >>> 0;
  });
  const crc = (b: Uint8Array) => {
    let c = 0xffffffff;
    for (const x of b) c = table[(c ^ x) & 255]! ^ (c >>> 8);
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

/** PocketJS's UI core for the 3DS: a Rust static library, built on the host with the compiler its crate pins. */
async function interfaceCore(): Promise<string> {
  const { RUSTUP_TOOLCHAIN: _toolchain, RUSTC: _rustc, ...environment } = process.env;
  await $`cargo build --release --locked`.cwd(join(POCKETJS, "hosts/3ds/core")).env({ ...environment, CARGO_TARGET_DIR: join(DIR, "ui-core") });
  return join(DIR, "ui-core/armv6k-nintendo-3ds/release/libpocketjs_3ds_core.a");
}

async function build() {
  if (HELD[0]?.id !== FIRST) throw new Error(`no pack at ${packPath(FIRST, "n3ds30")}: run \`bun tools/duty.ts cook --profile n3ds30\` first`);
  if (EMBED !== "none" && !existsSync(packPath(mapNamed(EMBED).id, "n3ds30"))) throw new Error(`no pack of "${EMBED}" for the 3DS: bun tools/duty.ts cook --map ${EMBED} --profile n3ds30`);
  // The interface: its bundle and pak for this device, and the plan they were resolved against.
  const uiDirectory = join(ROOT, ".pocket-build/ui/3ds");
  const ui = argv.includes("--no-ui") ? extractHostBuildInputs(JSON.parse(readFileSync(join(uiDirectory, "plan.json"), "utf8"))) : (await compileInterface("3ds", HELD)).inputs;
  const aux = ui.surfaces?.auxiliary.logical;
  if (ui.viewport.logical.join("x") !== "400x240" || aux?.join("x") !== "320x240") throw new Error("the interface's 3DS presentation is not 400x240 over 320x240");
  await $`rustup run ${TOOLCHAIN} cargo build --release`.cwd(join(ROOT, "n3ds/core"));
  const core = "n3ds/core/target/armv6k-nintendo-3ds/release/libduty_n3ds_core.a";
  const uiCore = await interfaceCore();
  const mounts = [{ hostPath: ROOT, containerPath: "/duty" }];
  await ensureQuickJs(join(DIR, "quickjs"), THREE_DS_CONTAINER_IMAGE, mounts);
  const romfs = join(DIR, "romfs");
  mkdirSync(romfs, { recursive: true });
  mkdirSync(join(DIR, "build"), { recursive: true });
  // The ROMFS's packs: the embedded map's pieces and a manifest that lists that map alone, or nothing.
  rmSync(join(romfs, "duty.pack"), { force: true });
  rmSync(join(DIR, "romfs.json"), { force: true });
  let packSha = "no pack";
  if (EMBED === "none") rmSync(join(romfs, "packs"), { recursive: true, force: true });
  else {
    const cut = writePacks(join(DIR, "embed"), [{ name: mapNamed(EMBED).id, file: packPath(EMBED, "n3ds30") }]);
    bring(join(DIR, "embed"), cut.manifest, join(romfs, "packs"), { prune: true });
    packSha = cut.packs[0]!.sha256;
  }
  mkdirSync(join(romfs, "packs"), { recursive: true });
  // The build's identity: the sources, both Rust libraries, the interface and the pack.
  const id = new Bun.CryptoHasher("sha256");
  for (const f of readdirSync(join(ROOT, "n3ds/src")).sort()) id.update(readFileSync(join(ROOT, "n3ds/src", f)));
  for (const f of [join(ROOT, core), uiCore, join(uiDirectory, "duty.js"), join(uiDirectory, "duty.pak")]) id.update(readFileSync(f));
  id.update(packSha);
  const buildId = id.digest("hex").slice(0, 12);
  // The guest refuses a bundle resolved for another target or host ABI: both come from the interface's plan.
  // PocketJS names an application's directory on the card by the first 16 hex digits of SHA-256 of its id.
  const appId = JSON.parse(readFileSync(join(ROOT, "ui/pocket.json"), "utf8")).id as string;
  const slot = new Bun.CryptoHasher("sha256").update(appId).digest("hex").slice(0, 16);
  const header = `#define POCKETJS_HOST_ABI ${ui.hostAbi}\n#define POCKETJS_TARGET_ID "${ui.target}"\n#define DUTY_BUILD_ID "${buildId}"\n#define DUTY_SLOT "${slot}"\n`;
  const config = join(DIR, "build/config.h");
  if (!existsSync(config) || readFileSync(config, "utf8") !== header) writeFileSync(config, header);
  // Compile a snapshot on the container's own filesystem: the shared mount can show a stale size for a file
  // that was just rewritten, on either side. The snapshot's name is new each build. It holds everything this
  // build wrote: the sources, both Rust libraries and the interface. The pack is copied through the mount
  // and checked against its hash; PocketJS's sources and QuickJS are read through the mount as they are.
  const snapshot = `source-${buildId}-${Date.now()}.tar`;
  for (const f of readdirSync(DIR).filter((f) => f.startsWith("source-"))) rmSync(join(DIR, f));
  const uiCoreInTar = ".pocket-build/3ds/ui-core/armv6k-nintendo-3ds/release/libpocketjs_3ds_core.a";
  // The app icon is Pocket3D's: the snapshot takes its two sizes under the path n3ds/Makefile reads them from.
  await $`tar --no-xattrs -cf ${join(DIR, snapshot)} n3ds/src n3ds/Makefile ${ICONS}/icon.png ${ICONS}/icon-small.png .pocket-build/3ds/build/config.h ${core} ${uiCoreInTar} .pocket-build/ui/3ds/duty.js .pocket-build/ui/3ds/duty.pak`.cwd(ROOT);
  await runContainer(
    `mkdir -p /tmp/source /tmp/build /tmp/romfs && tar -xf /duty/.pocket-build/3ds/${snapshot} -C /tmp/source
cp /tmp/source/.pocket-build/3ds/build/config.h /tmp/build/config.h
cp /tmp/source/.pocket-build/ui/3ds/duty.js /tmp/source/.pocket-build/ui/3ds/duty.pak /tmp/romfs/
mkdir -p /tmp/romfs/packs && cp -r /duty/.pocket-build/3ds/romfs/packs/. /tmp/romfs/packs/
for piece in /tmp/romfs/packs/*.bin; do [ -e "$piece" ] || continue; [ "$(sha256sum "$piece" | cut -c1-20)" = "$(basename "$piece" .bin)" ] || { echo "$piece is not the piece its name says"; exit 1; }; done
make -f /tmp/source/n3ds/Makefile -j8 BUILD=/tmp/build SOURCE=/tmp/source/n3ds/src ROMFS=/tmp/romfs CORE=/tmp/source/${core} UI_CORE=/tmp/source/${uiCoreInTar}
cp /tmp/build/duty.elf /tmp/build/duty.map /duty/.pocket-build/3ds/build/`,
    mounts,
    "/duty",
    {},
    "Pocket Duty build",
  );
  const bytes = readFileSync(ARTIFACT).length;
  if (bytes > 32 * 1024 * 1024) throw new Error(`the .3dsx is ${bytes} bytes; the wire installs at most 32 MiB`);
  mkdirSync(RECEIPTS, { recursive: true });
  const receipt = {
    target: "3ds", buildId, bytes, sha256: sha(ARTIFACT), packSha256: packSha,
    interface: { target: ui.target, hostAbi: ui.hostAbi, js: sha(join(uiDirectory, "duty.js")), pak: sha(join(uiDirectory, "duty.pak")) },
  };
  writeFileSync(join(RECEIPTS, "build.json"), JSON.stringify(receipt, null, 1) + "\n");
  console.log(`3ds: ${ARTIFACT} ${(bytes / 1e6).toFixed(1)} MB, build ${buildId}`);
  return receipt;
}

async function connect(): Promise<Client> {
  const keys = opt("--keys", join(POCKETJS, ".pocket/3ds/devices"));
  let devices: any[] = await discoverPocketRuntimes({ addresses: [host] });
  for (let attempt = 0; attempt < 3 && !devices.some((d) => d.address === host); attempt++) {
    await Bun.sleep(400);
    devices = await discoverPocketRuntimes({ addresses: [host] });
  }
  const device = devices.find((d) => d.address === host);
  if (!device) throw new Error(`no Pocket Runtime answers at ${host}:8131`);
  let token: Uint8Array | undefined;
  for (const name of existsSync(keys) ? readdirSync(keys).filter((n) => n.endsWith(".key")) : []) {
    const t = parsePocketRuntimeToken(readFileSync(join(keys, name), "utf8"));
    if (pocketRuntimeDeviceId(t) === device.deviceId) token = t;
  }
  if (!token) throw new Error(`no pairing key in ${keys} matches the console; pass --keys DIR`);
  const client = new PocketRuntimeClient({ host, port: device.port, token, timeoutMs: 20000, heartbeatTimeoutMs: 30000 });
  client.on("ctrl", (m: any) => {
    if (m.t === "log" || m.t === "runtime.native") console.log(JSON.stringify(m));
  });
  try {
    await client.connect();
    return client;
  } catch (e) {
    client.close();
    throw e;
  }
}

async function status(c: Client, text = ""): Promise<any> {
  const reply = c.waitForCtrl((m: any) => m.t === "duty.status", 8000);
  await c.sendCtrl({ t: "duty.control", ...(text ? { text } : {}) });
  return await reply;
}

/** Connects, retrying: right after another client leaves, the first connect can time out. */
async function session<T>(f: (c: Client) => Promise<T>): Promise<T> {
  let last: unknown;
  for (let attempt = 0; attempt < 4; attempt++) {
    let c: Client | undefined;
    try {
      c = await connect();
      return await f(c);
    } catch (e) {
      last = e;
      await Bun.sleep(1200);
    } finally {
      c?.close();
    }
  }
  throw last;
}

/** Runs `f` holding the console's lease. A child process started with `lease.environment` shares it. */
async function device<T>(f: (lease: { environment: NodeJS.ProcessEnv }) => Promise<T>): Promise<T> {
  return await withDeviceLease("3ds:wire", f);
}

/** Sends the .3dsx and waits until the console reports this build, inside one lease. */
async function install(lease: { environment: NodeJS.ProcessEnv }, receipt: { buildId: string }) {
  await $`bun ${join(POCKETJS, "tools/3ds-dev.ts")} install --host ${host} --file ${ARTIFACT} --name ${NAME}`.cwd(POCKETJS).env(lease.environment as Record<string, string>);
  const end = Date.now() + 180_000;
  let last: any;
  while (Date.now() < end) {
    await Bun.sleep(1500);
    try {
      last = await session((c) => status(c));
      if (last.build === receipt.buildId && (last.stage === "running" || last.phase === "load-error")) break;
    } catch (e) {
      last = { error: String(e) };
    }
  }
  if (last?.build !== receipt.buildId) throw new Error(`the console did not come up with this build: ${JSON.stringify(last)}`);
  return last;
}

async function shot(c: Client, surface: string) {
  const pending = c.waitForScreenshot();
  await c.sendCtrl({ t: "screenshot", surface });
  return await pending;
}

switch (cmd) {
  case "build":
    await build();
    break;
  case "install": {
    const receipt = argv.includes("--no-build") ? JSON.parse(readFileSync(join(RECEIPTS, "build.json"), "utf8")) : await build();
    // One lease for the transfer and the wait: another checkout's tool cannot put its own program in between.
    await device(async (lease) => console.log(JSON.stringify(await install(lease, receipt), null, 1)));
    break;
  }
  case "status":
    await device(async () => console.log(JSON.stringify(await session((c) => status(c)), null, 1)));
    break;
  case "ctl":
    await device(async () => console.log(JSON.stringify(await session((c) => status(c, argv[1] ?? "")), null, 1)));
    break;
  case "capture":
    await device(async () => {
      const out = resolve(opt("--out", join(RECEIPTS, `capture-${Date.now()}.png`)));
      mkdirSync(resolve(out, ".."), { recursive: true });
      const text = opt("--ctl", "");
      const picture = await session(async (c) => {
        if (text) {
          await status(c, text);
          await Bun.sleep(Number(opt("--wait", "6")) * 1000);
        }
        return await shot(c, opt("--surface", "top"));
      });
      await Bun.write(out, picture.png);
      console.log(out);
    });
    break;
  case "bench":
    await device(async (lease) => {
      const seconds = Number(opt("--seconds", "60"));
      const extra = opt("--ctl", "");
      const build = JSON.parse(readFileSync(join(RECEIPTS, "build.json"), "utf8"));
      // With --install the transfer and the measurement share the lease.
      if (argv.includes("--install")) await install(lease, build);
      await session(async (c) => {
        // A match as a person sees it, played by the bench's player: he turns to the nearest soldier in sight
        // and fires, so soldiers, effects and sounds are all in the frames measured.
        await status(c, "ui=title");
        await Bun.sleep(1500);
        const first = await status(c, `ui=start auto=1 god=0 ${extra}`);
        if (first.build !== build.buildId) throw new Error(`the console runs build ${first.build}, not ${build.buildId}`);
        await Bun.sleep(4000);
        const start = Date.now();
        const begin = await status(c);
        const samples: any[] = [];
        while (Date.now() - start < seconds * 1000) {
          // Each sample costs the console a late frame or so: answering takes it a few milliseconds.
          await Bun.sleep(5000);
          const s = await status(c);
          samples.push({ t: (Date.now() - start) / 1000, frameMs: s.frameMs, worstMs: s.worstMs, late: s.late, frames: s.frames, cpuMs: s.cpuMs, gpuMs: s.gpuMs, simMs: s.simMs, selectMs: s.selectMs, listMs: s.listMs, busyMs: s.busyMs, draws: s.draws, tris: s.tris, worldTris: s.worldTris, figureTris: s.figureTris, nearer: s.nearer, eye: s.eye, match: s.match, interface: s.interface });
        }
        await status(c, "auto=0");
        const last = samples.at(-1)!;
        const mean = (f: (s: any) => number) => samples.reduce((n, s) => n + f(s), 0) / samples.length;
        const frames = last.frames - begin.frames;
        const late = last.late - begin.late;
        const summary = {
          seconds,
          frames,
          lateFrames: late,
          lateShare: late / Math.max(frames, 1),
          averageFrameMs: mean((s) => s.frameMs),
          fps: frames / last.t,
          worstFrameMs: Math.max(...samples.map((s) => s.worstMs)),
          triangles: { least: Math.min(...samples.map((s) => s.tris)), mean: Math.round(mean((s) => s.tris)), most: Math.max(...samples.map((s) => s.tris)) },
          draws: { mean: Math.round(mean((s) => s.draws)), most: Math.max(...samples.map((s) => s.draws)) },
          cpuMs: { all: mean((s) => s.cpuMs), match: mean((s) => s.simMs), choosing: mean((s) => s.selectMs), drawing: mean((s) => s.listMs) },
          gpuMs: { mean: mean((s) => s.gpuMs), most: Math.max(...samples.map((s) => s.gpuMs)) },
          nearer: { least: Math.min(...samples.map((s) => s.nearer)), mean: mean((s) => s.nearer) },
          match: last.match,
          // The interface's share of a frame, a turn of its guest, and how many turns a second it took.
          interface: last.interface && {
            cpuMs: mean((s) => s.interface?.cpuMs ?? 0),
            turnMs: mean((s) => s.interface?.turnMs ?? 0),
            worstTurnMs: last.interface.worstTurnMs,
            scriptMs: mean((s) => s.interface?.scriptMs ?? 0),
            turnsPerSecond: (last.interface.turns - (begin.interface?.turns ?? 0)) / last.t,
            lowerDrawsPerSecond: (last.interface.lowerDraws - (begin.interface?.lowerDraws ?? 0)) / last.t,
            heapBytes: last.interface.heapBytes,
            linearBytes: last.interface.linearBytes,
          },
          linearFree: begin.linearFree,
          control: extra,
        };
        const dir = join(RECEIPTS, `bench-${new Date().toISOString().replace(/[:.]/g, "-")}`);
        mkdirSync(dir, { recursive: true });
        writeFileSync(join(dir, "device.json"), JSON.stringify({ summary, identity: { device: `3ds:${host}`, ...build, new3ds: begin.new3ds }, samples }, null, 1));
        console.log(join(dir, "device.json"));
        console.log(JSON.stringify(summary, null, 1));
      });
    });
    break;
  case "card": {
    // What a memory card takes: the program where the Homebrew Launcher lists it, and beside it every cooked
    // map's pack as pieces with their manifest. A directory that has some of the pieces gets the others.
    const to = argv[1];
    if (!to || to.startsWith("--")) throw new Error("usage: bun tools/n3ds.ts card <directory>");
    if (!existsSync(ARTIFACT)) throw new Error(`${ARTIFACT} is missing: bun tools/n3ds.ts build --embed none`);
    mkdirSync(join(to, CARD), { recursive: true });
    cpSync(ARTIFACT, join(to, CARD, NAME));
    const brought = bringMaps("n3ds30", join(to, CARD, "packs"), { prune: true });
    console.log(`${said(brought.written)}\n${resolve(to)}: ${CARD}/${NAME}, and ${brought.sent} pieces copied to ${CARD}/packs/ (${brought.had} were there)`);
    break;
  }
  case "emu": {
    // Azahar with a home of its own: the emulated memory card holds the run's control words, and takes the
    // frame and the status the program writes.
    const app = process.env.AZAHAR ?? "/Applications/Azahar.app";
    const source = `${homedir()}/Library/Application Support/Azahar`;
    if (!existsSync(app) || !existsSync(`${source}/config/qt-config.ini`)) throw new Error(`no Azahar at ${app} with a configuration of its own`);
    mkdirSync(join(DIR, "emu"), { recursive: true });
    const fixture = mkdtempSync(join(DIR, "emu/azahar-"));
    const user = `${fixture}/Library/Application Support/Azahar`;
    mkdirSync(`${user}/config`, { recursive: true });
    for (const dir of ["nand", "sysdata"]) if (existsSync(`${source}/${dir}`)) cpSync(`${source}/${dir}`, `${user}/${dir}`, { recursive: true });
    let config = readFileSync(`${source}/config/qt-config.ini`, "utf8");
    for (const [key, value] of Object.entries({ graphics_api: "0", resolution_factor: "1", frame_limit: "1000", use_vsync: "false", check_for_update_on_start: "false", instant_debug_log: "true" })) {
      config = config.replace(new RegExp(`^${key}=.*$`, "m"), `${key}=${value}`).replace(new RegExp(`^${key}\\\\default=.*$`, "m"), `${key}\\default=false`);
    }
    writeFileSync(`${user}/config/qt-config.ini`, config);
    const card = `${user}/sdmc/pocket-duty`;
    mkdirSync(card, { recursive: true });
    // The card's packs: every cooked map's (`--no-card`: the `.3dsx` and what its ROMFS carries, alone).
    if (!argv.includes("--no-card")) bringMaps("n3ds30", `${user}/sdmc/${CARD}/packs`);
    const frames = Number(opt("--frames", "90"));
    // `--map <id>`: the run changes to that map at its start, as a choice in the interface's list does.
    const onMap = argv.includes("--map") ? `map=${mapNamed(opt("--map", FIRST)).id} ` : "";
    writeFileSync(`${card}/boot.txt`, `title=0 ${onMap}${opt("--ctl", "")} shot=${frames} exit=${frames + 3}\n`);
    // What the interface kept in an earlier run, when the run is to start from it.
    const kept = `${user}/sdmc/3ds/pocket-duty/interface.json`;
    if (opt("--prefs", "")) {
      mkdirSync(resolve(kept, ".."), { recursive: true });
      writeFileSync(kept, opt("--prefs", ""));
    }
    const rom = `${fixture}/${NAME}`;
    cpSync(ARTIFACT, rom);
    const launch = Bun.spawnSync(["open", "-n", "-g", "-a", app, "--env", `HOME=${fixture}`, "--stdout", `${fixture}/console.log`, "--stderr", `${fixture}/console.log`, "--args", rom]);
    if (launch.exitCode) throw new Error(launch.stderr.toString());
    const owned = () =>
      Bun.spawnSync(["ps", "-axo", "pid=,command="])
        .stdout.toString()
        .split("\n")
        .filter((line) => line.includes(`${app}/Contents/MacOS/azahar`) && line.includes(rom))
        .map((line) => Number(line.trim().split(/\s+/)[0]));
    try {
      const start = Date.now();
      const limit = Number(opt("--timeout", "240")) * 1000;
      let asked = false;
      while (!existsSync(`${card}/done`)) {
        // After an emulator that was killed, macOS asks whether to reopen its windows, and the run waits for the
        // answer.
        if (!asked && Date.now() - start > 4000 && !existsSync(`${card}/boot.log`)) {
          asked = true;
          Bun.spawnSync(["osascript", "-e", 'tell application "System Events" to tell (first process whose name contains "zahar") to click button 2 of window 1']);
        }
        if (Date.now() - start > limit || (Date.now() - start > 15000 && !owned().length)) {
          if (existsSync(`${card}/boot.log`)) console.log(readFileSync(`${card}/boot.log`, "utf8"));
          throw new Error(`the emulator run did not finish: ${fixture}`);
        }
        await Bun.sleep(500);
      }
      if (existsSync(`${card}/status.json`)) console.log(readFileSync(`${card}/status.json`, "utf8"));
      if (existsSync(kept)) console.log(`kept: ${readFileSync(kept, "utf8")}`);
      const bytes = readFileSync(`${card}/shot.bgr`);
      if (bytes.length !== 400 * 240 * 3) throw new Error("the frame is not 400 by 240");
      // The lower screen stands under the upper one, as on the console. A run without an interface has none.
      const lower = existsSync(`${card}/shot-low.bgr`) ? readFileSync(`${card}/shot-low.bgr`) : undefined;
      const height = lower ? 480 : 240;
      const rgba = new Uint8Array(400 * height * 4);
      for (let i = 3; i < rgba.length; i += 4) rgba[i] = 255;
      // Columns from the left, each from the bottom up: blue, green, red.
      const place = (from: Uint8Array, width: number, left: number, top: number) => {
        for (let y = 0; y < 240; y++)
          for (let x = 0; x < width; x++) {
            const s = (x * 240 + 239 - y) * 3;
            rgba.set([from[s + 2]!, from[s + 1]!, from[s]!, 255], ((top + y) * 400 + left + x) * 4);
          }
      };
      place(bytes, 400, 0, 0);
      if (lower) place(lower, 320, 40, 240);
      const out = resolve(opt("--out", join(DIR, "emu/frame.png")));
      mkdirSync(resolve(out, ".."), { recursive: true });
      writeFileSync(out, encodePng(rgba, 400, height));
      console.log(out);
      // What the emulator forgave: a read of unmapped memory stops the console.
      const log = `${user}/log/azahar_log.txt`;
      if (existsSync(log)) {
        const unmapped = readFileSync(log, "utf8").split("\n").filter((line) => /unmapped/i.test(line));
        console.log(unmapped.length ? `emulator log: ${unmapped.length} line(s) name unmapped memory, the first: ${unmapped[0]}` : "emulator log: no read or write of unmapped memory");
      }
    } finally {
      // Asked to quit first: an emulator that is killed leaves macOS a question for the next run.
      for (const signal of ["SIGTERM", "SIGKILL"] as const) {
        for (const pid of owned()) {
          try {
            process.kill(pid, signal);
          } catch {}
        }
        for (let i = 0; i < 30 && owned().length; i++) await Bun.sleep(100);
      }
      if (!argv.includes("--keep")) rmSync(fixture, { recursive: true, force: true });
    }
    break;
  }
  default:
    console.log("usage: bun tools/n3ds.ts <build|card|install|status|ctl|capture|bench|emu> [--host ADDRESS]");
    process.exit(cmd ? 1 : 0);
}
