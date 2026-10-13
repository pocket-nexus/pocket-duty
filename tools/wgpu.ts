#!/usr/bin/env bun
// Pocket Duty drawn with wgpu (wgpu/): in a browser tab over WebGPU, and on
// this machine, where frames go to files.
//
//   bun tools/wgpu.ts build                        wasm32 + wasm-bindgen + the page + the interface as each
//                                                  device loads it (tools/ui.ts) on PocketJS's UI core → .pocket-build/wgpu/site
//   bun tools/wgpu.ts serve [--port 8802]          the site and the packs, with byte ranges
//   bun tools/wgpu.ts dist [--piece 2]             the directory a static host serves → .pocket-build/wgpu/dist:
//                                                  the page, the module under its build's name, and every
//                                                  map's pack cut into pieces of that many MiB with its manifest
//   bun tools/wgpu.ts serve --dist                 that directory as such a host serves it: no byte ranges
//   bun tools/wgpu.ts shot [--map yards] [--out f.png] [--size WxH] [--frames N] [--words "mode=play auto=1"] [--cue N:WORDS]
//                                                  one frame on this machine's GPU (Metal) → a PNG and the status
//   bun tools/wgpu.ts check [--headed] [--seconds 5] [--dist]   the page in Chrome, driven by keys and pointer:
//                                                  from the title into a match, the menu, the results, the same
//                                                  as a Nintendo 3DS with its lower screen's keys, what a
//                                                  frame costs, what the first frame needs on a slow line,
//                                                  and, when a second map is cooked, a match on it chosen
//                                                  from the map list → .pocket-build/validation/web/
//
// The site has the pack of every map that is cooked for the PSP (maps/maps.json, tools/maps.ts): the first
// map's is what the page opens on, and the map list offers the others. Every command takes [--pack PATH]
// for the first map's pack (default: .pocket-build/duty/psp30/duty.pack, which `bun tools/duty.ts cook`
// writes). `build` compiles the interface as tools/ui.ts does: it needs `bun install` in vendor/pocketjs.
// The site and the captures stay under the ignored .pocket-build/.

import { $ } from "bun";
import { cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { gzipSync } from "node:zlib";
import { join, resolve } from "node:path";
import { POCKET3D_ICON } from "../vendor/pocketjs/tools/pocket3d-icon.ts";
import { cutPack, stagePocket3dWeb } from "../vendor/pocketjs/tools/pocket3d-web.ts";
import { FIRST, mapNamed, MAPS, packPath } from "./maps.ts";
import { compileInterface, type Device } from "./ui.ts";

export const ROOT = resolve(import.meta.dir, "..");
const CRATE = join(ROOT, "wgpu");
const BUILD = join(ROOT, ".pocket-build/wgpu");
const SITE = join(BUILD, "site");
const DIST = join(BUILD, "dist");
export const PACK = packPath(FIRST, "psp30");
// The handhelds the page shows (`DEVICES` in wgpu/page/main.js, `SHAPES` in wgpu/src/app.rs).
export const SHOWN = ["psp", "3ds"] as const satisfies readonly Device[];
// What the host a build is deployed to allows (Pocket Studio's site deployments): the size of a file, the
// files and the bytes of a deployment, and the top-level names it keeps for itself.
const HOST = { file: 32 << 20, files: 4000, bytes: 1 << 30, reserved: ["play", "runtime"] };

const [command, ...rest] = import.meta.main ? process.argv.slice(2) : [];
const option = (flag: string, fallback = "") => {
  const i = rest.indexOf(flag);
  return i >= 0 && rest[i + 1] ? rest[i + 1]! : fallback;
};
const options = (flag: string) => rest.flatMap((word, i) => (word === flag && rest[i + 1] ? [rest[i + 1]!] : []));
const pack = resolve(option("--pack", PACK));

function needPack() {
  if (!existsSync(pack)) throw new Error(`${pack} is missing: bun tools/duty.ts export, then bun tools/duty.ts cook`);
}

/** The packs the site has: the first map's, and every other map's that is cooked. `name`: its file on the server here. */
export function packs(first = pack): { id: string; file: string; name: string }[] {
  return MAPS.flatMap((map) => {
    const file = map.id === FIRST ? first : packPath(map.id, "psp30");
    return map.id === FIRST || existsSync(file) ? [{ id: map.id, file, name: map.id === FIRST ? "duty.pack" : `${map.id}.pack` }] : [];
  });
}

/** The page with the other maps' packs named (`<meta name="pocket-packs">`: `<map>=<pack>` with spaces between). */
export function namePacks(page: string, named: { id: string; pack: string }[]): string {
  const meta = /<meta name="pocket-packs" content="[^"]*">/;
  if (!meta.test(page)) throw new Error(`wgpu/page/index.html has no <meta name="pocket-packs">`);
  return page.replace(meta, `<meta name="pocket-packs" content="${named.map((each) => `${each.id}=${each.pack}`).join(" ")}">`);
}

/** The wasm module, its JavaScript side and the page, as one directory a static server can serve. */
export async function build() {
  // The crate and the command line tool write two halves of one interface: their versions must be the same.
  const lock = readFileSync(join(CRATE, "Cargo.lock"), "utf8").match(/name = "wasm-bindgen"\nversion = "([^"]+)"/)?.[1];
  const tool = (await $`wasm-bindgen --version`.text()).trim().split(" ")[1];
  if (lock !== tool) throw new Error(`wasm-bindgen ${tool} is installed and wgpu/Cargo.lock has ${lock}: cargo install wasm-bindgen-cli --version ${lock}`);
  await $`cargo build --release --lib --target wasm32-unknown-unknown`.cwd(CRATE);
  rmSync(SITE, { recursive: true, force: true });
  mkdirSync(join(SITE, "pkg"), { recursive: true });
  await $`wasm-bindgen --target web --no-typescript --out-dir ${join(SITE, "pkg")} ${join(CRATE, "target/wasm32-unknown-unknown/release/duty_wgpu.wasm")}`;
  cpSync(join(CRATE, "page/main.js"), join(SITE, "main.js"));
  // The page names the packs the server here answers: every cooked map's.
  writeFileSync(join(SITE, "index.html"), namePacks(readFileSync(join(CRATE, "page/index.html"), "utf8"), packs().slice(1).map((each) => ({ id: each.id, pack: each.name }))));
  // What the page loads from PocketJS's browser kernel (vendor/pocketjs/devices/web/pocket-web-wgpu), as
  // PocketJS stages it: the page's modules, the Pocket3D title card, the realm of the interface's guest
  // with the UI core, and the host helpers of the framework. Then the icon of the tab.
  await stagePocket3dWeb(SITE);
  cpSync(POCKET3D_ICON.ios2x, join(SITE, "icon.png"));
  // The game's interface as the PSP's own build compiles it, with the plan PocketJS resolved.
  for (const device of SHOWN) {
    const built = await compileInterface(device);
    mkdirSync(join(SITE, "ui", device), { recursive: true });
    for (const file of ["duty.js", "duty.pak", "plan.json"]) cpSync(join(built.directory, file), join(SITE, "ui", device, file));
  }

  const sizes: Record<string, { bytes: number; gzip: number }> = {};
  for (const file of files(SITE).sort()) {
    const bytes = readFileSync(join(SITE, file));
    sizes[file] = { bytes: bytes.length, gzip: gzipSync(bytes, { level: 9 }).length };
  }
  writeFileSync(join(BUILD, "site.json"), JSON.stringify({ wasmBindgen: tool, sizes }, null, 1));
  return sizes;
}

const sha256 = (bytes: Uint8Array | string) => new Bun.CryptoHasher("sha256").update(bytes).digest("hex");

/** Every file under a directory, as paths from it. */
function files(directory: string, under = ""): string[] {
  return readdirSync(join(directory, under), { withFileTypes: true }).flatMap((entry) => (entry.isDirectory() ? files(directory, join(under, entry.name)) : [join(under, entry.name)]));
}

/**
 * The directory a static host serves, for a host that limits a file's size and keeps a file for ten minutes
 * in a browser's cache. Only the page is asked for again at every visit, so everything it names has a name of
 * its own contents: the module and its scripts under `app/<build>/`, the pack's manifest by the pack's hash,
 * a piece by its own. A deployment of new code leaves the pack's files as they are.
 */
async function dist(pieceBytes: number) {
  needPack();
  await build();
  rmSync(DIST, { recursive: true, force: true });
  // (everything of the site but the page and its icon: the module, the scripts, the interface)
  const app = files(SITE).filter((file) => !["index.html", "icon.png"].includes(file)).sort().map((file) => [file, readFileSync(join(SITE, file))] as const);
  const id = sha256(Buffer.concat(app.flatMap(([file, bytes]) => [Buffer.from(file), bytes]))).slice(0, 12);
  for (const [file, bytes] of app) {
    mkdirSync(join(DIST, "app", id, file, ".."), { recursive: true });
    writeFileSync(join(DIST, "app", id, file), bytes);
  }
  // Each map's pack in pieces of one size, each named by its hash, and the manifest that lists them
  // (pocket_web_wgpu::source::Manifest), named by the pack's.
  const cuts = packs().map((each) => ({ id: each.id, ...cutPack(each.file, join(DIST, "pack"), pieceBytes) }));
  const cut = cuts[0]!;
  const manifest = `pack/${cut.manifest}`;
  // The page names its build and its packs.
  let page = namePacks(readFileSync(join(SITE, "index.html"), "utf8"), cuts.slice(1).map((each) => ({ id: each.id, pack: `pack/${each.manifest}` })));
  for (const [from, to] of [[`<meta name="pocket-pack" content="duty.pack">`, `<meta name="pocket-pack" content="${manifest}">`], [`src="main.js"`, `src="app/${id}/main.js"`], [`href="pocket3d-stage.css"`, `href="app/${id}/pocket3d-stage.css"`], [`href="pocket3d-player.css"`, `href="app/${id}/pocket3d-player.css"`]] as const) {
    if (!page.includes(from)) throw new Error(`wgpu/page/index.html has no ${from}`);
    page = page.replace(from, to);
  }
  // The game in Pocket Studio, for a host that answers no /app.json: the project this checkout is
  // registered as (`pocket-studio register` wrote .pocket-studio.json, which Git ignores). The game's host
  // on Pocket Studio answers one, and the player's dock takes its links from that.
  const link = join(ROOT, ".pocket-studio.json");
  const project = existsSync(link) ? (JSON.parse(readFileSync(link, "utf8")) as { kind?: string; server?: string; app?: string }) : null;
  const registered = project?.kind === "site" && /^[A-Za-z0-9_-]+$/.test(project.app ?? "") && /^https:\/\/[A-Za-z0-9.-]+$/.test(project.server ?? "") ? project : null;
  if (registered) page = page.replace(`<meta name="pocket-pack"`, `<meta name="pocket-app" content="${registered.app}">\n<meta name="pocket-studio" content="${registered.server}">\n<meta name="pocket-pack"`);
  writeFileSync(join(DIST, "index.html"), page);
  cpSync(join(SITE, "icon.png"), join(DIST, "icon.png"));

  // What was written is what the host takes.
  const all = files(DIST).map((file) => ({ file, bytes: statSync(join(DIST, file)).size }));
  const total = all.reduce((sum, f) => sum + f.bytes, 0);
  const largest = all.reduce((a, b) => (b.bytes > a.bytes ? b : a));
  const part = (prefix: string) => all.filter((f) => f.file.startsWith(prefix)).reduce((sum, f) => ({ files: sum.files + 1, bytes: sum.bytes + f.bytes }), { files: 0, bytes: 0 });
  const refused = [
    ...all.filter((f) => f.bytes > HOST.file).map((f) => `${f.file} is ${f.bytes} bytes (a file is at most ${HOST.file})`),
    ...(all.length > HOST.files ? [`${all.length} files (at most ${HOST.files})`] : []),
    ...(total > HOST.bytes ? [`${total} bytes (at most ${HOST.bytes})`] : []),
    ...readdirSync(DIST).filter((name) => HOST.reserved.includes(name)).map((name) => `${name}/ is the host's own`),
  ];
  if (refused.length) throw new Error(`the host would refuse the directory: ${refused.join("; ")}`);
  const report = { directory: DIST, files: all.length, bytes: total, largest, build: id, studio: registered ? { app: registered.app, server: registered.server } : null, page: part("index.html"), app: part("app/"), pack: { ...part("pack/"), manifest, pieces: cut.pieces.length, piece: pieceBytes, sha256: cut.sha256 }, maps: Object.fromEntries(cuts.map((each) => [each.id, { manifest: `pack/${each.manifest}`, bytes: each.bytes, pieces: each.pieces.length, sha256: each.sha256 }])) };
  writeFileSync(join(BUILD, "dist.json"), JSON.stringify(report, null, 1));
  return report;
}

/** What Chrome is started with for a page that draws with WebGPU: the real GPU, through Metal. */
export const CHROME = ["--use-angle=metal", "--enable-gpu", "--ignore-gpu-blocklist", "--enable-unsafe-webgpu", "--no-proxy-server", "--autoplay-policy=no-user-gesture-required"];

const TYPES: Record<string, string> = { html: "text/html; charset=utf-8", js: "text/javascript; charset=utf-8", css: "text/css; charset=utf-8", wasm: "application/wasm", json: "application/json", png: "image/png", webp: "image/webp", woff2: "font/woff2", txt: "text/plain; charset=utf-8", pak: "application/octet-stream" };

/**
 * What a game's host of Pocket Studio answers at `/app.json`, for the server here: the player reads the
 * game's name and its packages from it, and the address it tells that it opened (`opened`, which the
 * server here answers itself and counts).
 */
const OPENED = "/api/events/player";
const opens: { query: string; mode: string }[] = [];
const APP = {
  kind: "site", id: "local", slug: "duty", url: null, title: "Pocket Duty", author: "local", tagline: "A firefight on a market street.", verified: true, status: "published",
  packages: [{ target: "psp", filename: "pocket-duty-0.1.0-psp.zip", size: 16_000_000, version: "0.1.0" }, { target: "3ds", filename: "pocket-duty-0.1.0.3dsx", size: 31_000_000, version: "0.1.0" }],
  remixable: true, repository: "https://github.com/pocket-nexus/pocket-duty",
};

/** The deployable directory as its host serves it: whole files, the page asked for again at every visit. */
function serveDist(port: number) {
  return Bun.serve({
    port,
    hostname: "127.0.0.1",
    fetch(request) {
      const path = decodeURIComponent(new URL(request.url).pathname);
      // (this host names no address for it: a page that asks is heard, and the check says so)
      if (path === OPENED) opens.push({ query: new URL(request.url).search, mode: request.headers.get("sec-fetch-mode") ?? "" });
      const file = join(DIST, path === "/" ? "index.html" : path);
      if (!file.startsWith(DIST) || !existsSync(file) || !statSync(file).isFile()) return new Response("not found", { status: 404 });
      const type = file.split(".").pop()!;
      return new Response(Bun.file(file), { headers: { "Content-Type": TYPES[type] ?? "application/octet-stream", "Cache-Control": type === "html" ? "no-cache" : "public, max-age=600" } });
    },
  });
}

/** The site and the packs. A pack is answered a range at a time, as a tab asks for it. */
export function serve(port: number) {
  const served = new Map(packs().map((each) => [`/${each.name}`, each.file]));
  return Bun.serve({
    port,
    hostname: "127.0.0.1",
    fetch(request) {
      const path = decodeURIComponent(new URL(request.url).pathname);
      if (path === "/app.json") return Response.json({ ...APP, opened: new URL(OPENED, request.url).href }, { headers: { "Cache-Control": "no-store" } });
      if (path === OPENED) {
        opens.push({ query: new URL(request.url).search, mode: request.headers.get("sec-fetch-mode") ?? "" });
        return new Response(null, { status: 204 });
      }
      const pack = served.get(path);
      if (pack) {
        const size = statSync(pack).size;
        const head = { "Accept-Ranges": "bytes", "Content-Type": "application/octet-stream", "Cache-Control": "no-store" };
        const range = request.headers.get("range")?.match(/^bytes=(\d+)-(\d*)$/);
        if (!range) return new Response(Bun.file(pack), { headers: head });
        const from = Number(range[1]);
        const to = Math.min(range[2] ? Number(range[2]) : size - 1, size - 1);
        if (from > to) return new Response(null, { status: 416, headers: { "Content-Range": `bytes */${size}` } });
        return new Response(Bun.file(pack).slice(from, to + 1), { status: 206, headers: { ...head, "Content-Range": `bytes ${from}-${to}/${size}`, "Content-Length": String(to - from + 1) } });
      }
      const file = join(SITE, path === "/" ? "index.html" : path);
      if (!file.startsWith(SITE) || !existsSync(file) || !statSync(file).isFile()) return new Response("not found", { status: 404 });
      return new Response(Bun.file(file), { headers: { "Content-Type": TYPES[file.split(".").pop()!] ?? "application/octet-stream", "Cache-Control": "no-store" } });
    },
  });
}

/** The program that draws frames on this machine's GPU (wgpu/src/bin/shot.rs), built. */
export async function shotProgram(): Promise<string> {
  await $`cargo build --release --bin duty-shot`.cwd(CRATE).quiet();
  return join(CRATE, "target/release/duty-shot");
}

/** One frame on this machine's GPU, from the pack's file or from a manifest of its pieces. Returns the status
 * the run printed. */
async function shot(out: string, extra: string[], from = pack) {
  const program = await shotProgram();
  mkdirSync(resolve(out, ".."), { recursive: true });
  return JSON.parse(await $`${program} --pack ${from} --out ${out} ${extra}`.text());
}

const stamp = () => new Date().toISOString().replace(/[:.]/g, "-");
const validation = (run: string) => {
  const directory = join(ROOT, ".pocket-build/validation/web", run);
  mkdirSync(directory, { recursive: true });
  return directory;
};

if (command === "build") {
  console.log(JSON.stringify(await build(), null, 1));
} else if (command === "dist") {
  console.log(JSON.stringify(await dist(Math.round(Number(option("--piece", "2")) * (1 << 20))), null, 1));
} else if (command === "serve" && rest.includes("--dist")) {
  if (!existsSync(join(DIST, "index.html"))) await dist(2 << 20);
  const server = serveDist(Number(option("--port", "8802")));
  console.log(`http://127.0.0.1:${server.port}/   (${DIST})`);
} else if (command === "serve") {
  needPack();
  if (!existsSync(join(SITE, "pkg/duty_wgpu_bg.wasm")) || rest.includes("--build")) await build();
  const server = serve(Number(option("--port", "8802")));
  console.log(`http://127.0.0.1:${server.port}/   (${pack})`);
} else if (command === "shot") {
  needPack();
  // (--map: a frame of another map's pack)
  const map = mapNamed(option("--map", FIRST)).id;
  const from = packs().find((each) => each.id === map)?.file;
  if (!from) throw new Error(`${packPath(map, "psp30")} is missing: bun tools/duty.ts export --map ${map}, then bun tools/duty.ts cook --map ${map}`);
  const out = resolve(option("--out", join(validation(`shot-${stamp()}`), "frame.png")));
  const passed = [...["--size", "--frames", "--words", "--status", "--shape"].flatMap((flag) => (option(flag) ? [flag, option(flag)] : [])), ...options("--cue").flatMap((cue) => ["--cue", cue]), "--map", map];
  console.log(JSON.stringify(await shot(out, passed, from), null, 1));
  console.log(out);
} else if (command === "check") {
  needPack();
  // (--dist: the deployable directory, served whole files only, with the pack in pieces)
  const deployed = rest.includes("--dist") ? await dist(Math.round(Number(option("--piece", "2")) * (1 << 20))) : null;
  const sizes = deployed ? JSON.parse(readFileSync(join(BUILD, "site.json"), "utf8")).sizes : await build();
  const { chromium } = await import("playwright-core");
  const server = deployed ? serveDist(0) : serve(0);
  const directory = validation(`check-${deployed ? "dist-" : ""}${stamp()}`);
  const seconds = Number(option("--seconds", "5"));
  const origin = `http://127.0.0.1:${server.port}`;
  // WebGPU needs the real GPU: headless Chrome is given Metal through ANGLE; --headed opens a window instead.
  const browser = await chromium.launch({ channel: "chrome", headless: !rest.includes("--headed"), args: CHROME });
  const report: Record<string, any> = { chrome: browser.version(), pack, deployed };
  const expect = (what: string, ok: boolean) => {
    if (!ok) throw new Error(`the page: ${what}`);
  };
  type Page = Awaited<ReturnType<typeof browser.newPage>>;
  /** A page on the site, and what a person does to it: keys held for a moment, the pointer on the shell. */
  const visit = async (address: string, context?: Awaited<ReturnType<typeof browser.newContext>>) => {
    const page: Page = await (context ?? browser).newPage({ viewport: { width: 1280, height: 800 } });
    const problems: string[] = [];
    // (the deployable directory's host answers no /app.json: the browser writes that one reply to the
    // console, and the player goes on with the page's own words)
    page.on("console", (message) => message.type() === "error" && !message.location().url.endsWith("/app.json") && problems.push(message.text()));
    page.on("pageerror", (error) => problems.push(String(error)));
    await page.goto(`${origin}/${address}`);
    const status = async () => JSON.parse((await page.evaluate("pocketDuty.duty.status()")) as string);
    return {
      page,
      problems,
      status,
      say: (words: string) => page.evaluate(`pocketDuty.duty.control(${JSON.stringify(words)})`),
      /** The street is behind the title, the interface is up, and the card has left. */
      async up() {
        await page.waitForFunction("window.pocketDuty && ((pocketDuty.firstWorld && pocketDuty.interface?.()) || pocketDuty.failure)", undefined, { timeout: 90_000 });
        const failure = (await page.evaluate("pocketDuty.failure")) as string;
        if (failure) throw new Error(`${address}: ${failure}`);
      },
      async mode(want: string, within = 5_000) {
        await page.waitForFunction(`JSON.parse(pocketDuty.duty.status()).mode === ${JSON.stringify(want)}`, undefined, { timeout: within }).catch(() => {});
        const now = (await status()).mode;
        expect(`${address}: the flow is "${now}", not "${want}"`, now === want);
      },
      async key(code: string, hold = 90) {
        await page.keyboard.down(code);
        await page.waitForTimeout(hold);
        await page.keyboard.up(code);
        await page.waitForTimeout(200);
      },
      /** The screen as its canvas holds it, a pixel to a pixel. */
      async save(name: string) {
        const url = (await page.evaluate("pocketDuty.capture()")) as string;
        writeFileSync(join(directory, `${name}.png`), Buffer.from(url.slice(url.indexOf(",") + 1), "base64"));
        return join(directory, `${name}.png`);
      },
    };
  };
  const program = await shotProgram();
  // With several maps the title's first row opens their list: its first row, the map behind the title,
  // is chosen by the same key.
  const offered = packs();
  const several = offered.length > 1;
  const onTheFirst = async (p: Awaited<ReturnType<typeof visit>>) => {
    if (!several) return;
    await p.page.waitForTimeout(350);
    await p.key("KeyZ");
  };
  const compare = async (a: string, b: string) => JSON.parse(await $`${program} --compare ${a} ${b}`.text());
  const far = (a: number[], b: number[]) => Math.hypot(a[0]! - b[0]!, a[1]! - b[1]!, a[2]! - b[2]!);

  try {
    // The GPU the tab is given.
    const probe = await browser.newPage();
    await probe.goto(`${origin}/index.html?interface=off`);
    report.gpu = await probe.evaluate(`(async () => {
      const adapter = await navigator.gpu?.requestAdapter({ powerPreference: "high-performance" });
      if (!adapter) return null;
      const { vendor, architecture, device, description } = adapter.info ?? {};
      return { vendor, architecture, device, description, fallback: adapter.info?.isFallbackAdapter ?? adapter.isFallbackAdapter ?? false, format: navigator.gpu.getPreferredCanvasFormat() };
    })()`);
    await probe.close();

    // ---- the street alone: the tab's frame of a held eye beside this machine's own, and the title card first
    const start = JSON.parse(await $`${program} --pack ${deployed ? join(DIST, deployed.pack.manifest) : pack} --out ${join(directory, "start.png")} --frames 2`.text());
    const words = `view=${[start.eye[0], start.eye[1], start.eye[2], start.yaw + 20, -3, 65].map((n: number) => n.toFixed(2)).join(",")}`;
    {
      const p = await visit(`?interface=off&sound=off&words=${encodeURIComponent(words)}`);
      await p.page.waitForTimeout(1200);
      const during = (await p.page.evaluate("[document.querySelectorAll('[aria-label=Pocket3D]').length, document.querySelector('[data-pocket-screen=upper]').hidden]")) as [number, boolean];
      await p.page.screenshot({ path: join(directory, "title-card.png") });
      expect(`the Pocket3D title card covers the page before the game is shown (${during})`, during[0] === 1 && during[1] === true);
      await p.page.waitForFunction("window.pocketDuty?.firstWorld > 0", undefined, { timeout: 90_000 });
      await p.page.waitForTimeout(600);
      const afterwards = (await p.page.evaluate("[document.querySelectorAll('[aria-label=Pocket3D]').length, document.querySelector('[data-pocket-screen=upper]').hidden]")) as [number, boolean];
      expect(`the card has left and the game is shown (${afterwards})`, afterwards[0] === 0 && afterwards[1] === false);
      const tab = await p.save("scene-tab");
      await shot(join(directory, "scene-here.png"), ["--frames", "30", "--words", words], deployed ? join(DIST, deployed.pack.manifest) : pack);
      report.sceneTabAgainstHere = await compare(tab, join(directory, "scene-here.png"));
      expect(`the tab's frame is this machine's (${JSON.stringify(report.sceneTabAgainstHere)})`, report.sceneTabAgainstHere.mean < 1.5);
      expect(`no error on the page (${p.problems.join("; ")})`, p.problems.length === 0);
      await p.page.close();
    }

    // ---- a match by the keys: from the title through the interface, the walk, the look, the weapon,
    // the menu opened and closed, a firefight, the results
    {
      const p = await visit("");
      await p.up();
      await p.mode("title");
      await p.page.waitForTimeout(600);
      await p.save("title");
      await p.page.screenshot({ path: join(directory, "page-title.png") });
      // The title's first row starts the match (the face button at the right chooses: ○).
      await p.key("KeyZ");
      if (several) {
        await p.page.waitForTimeout(500);
        await p.save("maps");
        await p.page.screenshot({ path: join(directory, "page-maps.png") });
      }
      await onTheFirst(p);
      await p.mode("play");
      // (nothing hurts the player while the keys are tried)
      await p.say("god=1");
      const before = await p.status();
      // The stick forward: the player runs up the street.
      await p.page.keyboard.down("KeyW");
      await p.page.waitForTimeout(900);
      const running = await p.status();
      await p.page.keyboard.up("KeyW");
      expect(`the stick moves the player (${far(before.player.pos, running.player.pos).toFixed(1)} m), all the way forward at a run (${running.player.sprinting})`, far(before.player.pos, running.player.pos) > 2 && running.player.sprinting === true);
      // The face button at the right turns the view to the right.
      await p.page.keyboard.down("KeyL");
      await p.page.waitForTimeout(600);
      await p.page.keyboard.up("KeyL");
      const turned = await p.status();
      expect(`a face button turns the view (${running.player.yaw.toFixed(2)} to ${turned.player.yaw.toFixed(2)})`, Math.abs(turned.player.yaw - running.player.yaw) > 0.2);
      // L raises the sights, with the stick held too: the player walks.
      await p.page.keyboard.down("KeyQ");
      await p.page.keyboard.down("KeyW");
      await p.page.waitForTimeout(700);
      const aiming = await p.status();
      await p.save("sights");
      await p.page.keyboard.up("KeyW");
      await p.page.keyboard.up("KeyQ");
      expect(`L raises the sights while the stick is held (${aiming.player.sights.toFixed(2)}, sprinting ${aiming.player.sprinting})`, aiming.player.sights > 0.6 && aiming.player.sprinting === false);
      // R fires.
      await p.page.keyboard.down("KeyE");
      await p.page.waitForTimeout(500);
      await p.save("firing");
      // (what the mixer hands the page's output while the rifle fires: a block's loudest sample, of 32 768)
      report.loudest = await p.page.evaluate("Array.from(pocketDuty.duty.audio(2048)).reduce((most, sample) => Math.max(most, Math.abs(sample)), 0)");
      await p.page.keyboard.up("KeyE");
      const fired = await p.status();
      expect(`R fires (${fired.match.shots} rounds, ${fired.match.magazine} left)`, fired.match.shots > 2 && fired.match.magazine < 30);
      // Left on the pad changes the magazine; right takes the next weapon.
      await p.key("ArrowLeft");
      await p.page.waitForFunction("JSON.parse(pocketDuty.duty.status()).match.magazine === 30", undefined, { timeout: 8_000 }).catch(() => {});
      const loaded = await p.status();
      expect(`left on the pad changes the magazine (${fired.match.magazine} to ${loaded.match.magazine} rounds)`, loaded.match.magazine === 30 && loaded.match.reserve < fired.match.reserve);
      await p.key("ArrowRight");
      await p.page.waitForTimeout(1200);
      const next = await p.status();
      expect(`right on the pad takes the next weapon (${loaded.match.weapon} to ${next.match.weapon})`, next.match.weapon !== loaded.match.weapon);
      await p.save("next-weapon");
      // Sound: the page's output runs after the first key.
      report.sound = await p.page.evaluate("pocketDuty.sound()");
      expect(`the sound's output runs after a key (${report.sound}), sounds were started (${loaded.sound.played}) and a block of them is not silence (${report.loudest})`, report.sound === "running" && loaded.sound.played > 3 && report.loudest > 1000);
      // START stops the match under the menu: no tick runs. Its first row resumes.
      await p.key("Space");
      await p.mode("menu");
      const stopped = await p.status();
      await p.page.waitForTimeout(500);
      expect("the menu stops the match", (await p.status()).match.tick === stopped.match.tick);
      await p.save("menu");
      // Its second row is the settings: a switch changed there is kept for the next visit.
      await p.key("ArrowDown");
      await p.key("KeyZ");
      await p.page.waitForTimeout(400);
      await p.save("settings");
      await p.key("ArrowDown");
      await p.key("KeyZ");
      await p.page.waitForTimeout(400);
      report.kept = await p.page.evaluate(`localStorage.getItem("pocket-duty.interface")`);
      await p.key("KeyX");
      await p.page.waitForTimeout(300);
      await p.key("ArrowUp");
      await p.key("KeyZ");
      await p.mode("play");
      // A firefight: the player in front of a soldier, the bench's aim and trigger.
      await p.say("near=0,7 auto=1");
      await p.page.waitForFunction("JSON.parse(pocketDuty.duty.status()).match.kills > 0", undefined, { timeout: 30_000 }).catch(() => {});
      const fought = await p.status();
      expect(`the bench's player fells a soldier (${fought.match.kills} kills, ${fought.match.hits} hits of ${fought.match.shots})`, fought.match.kills > 0 && fought.match.us > 0);
      await p.save("firefight");
      await p.page.screenshot({ path: join(directory, "page-play.png") });
      report.match = fought.match;
      // What a frame costs with the interface over the street.
      const read = `({ frames: pocketDuty.frames, at: performance.now(), timing: { ...pocketDuty.timing } })`;
      const from = (await p.page.evaluate(read)) as { frames: number; at: number; timing: Record<string, number> };
      await p.page.waitForTimeout(seconds * 1000);
      const to = (await p.page.evaluate(read)) as typeof from;
      const span = (to.at - from.at) / 1000;
      const d = (key: string) => to.timing[key]! - from.timing[key]!;
      const now = await p.status();
      const round = (n: number) => +n.toFixed(3);
      report.measured = {
        fps: round((to.frames - from.frames) / span),
        // (300 frames made without waiting for the display: the match, the guest's turns and redraws, the street)
        frameMs: round((await p.page.evaluate("pocketDuty.burst(300)")) as number),
        turnsPerSecond: round(d("turns") / span),
        turnMs: round(d("turnMs") / Math.max(1, d("turns"))),
        redrawsPerSecond: round(d("redraws") / span),
        redrawMs: round(d("redrawMs") / Math.max(1, d("redraws"))),
        drawMs: round(d("drawMs") / Math.max(1, d("redraws"))),
        draws: now.draws,
        tris: now.tris,
        figureTris: now.figureTris,
        late: now.late,
      };
      // A soldier's rounds fell the player: the match goes on, and puts him back after four seconds.
      const standing = (await p.status()).soldiers.find((soldier: { alive: boolean }) => soldier.alive);
      await p.say(`auto=0 god=0 skill=400 near=${standing.number},6`);
      await p.page.waitForFunction(`JSON.parse(pocketDuty.duty.status()).mode === "down"`, undefined, { timeout: 60_000 }).catch(async () => console.error("not down:", JSON.stringify(await p.status())));
      await p.mode("down");
      await p.page.waitForTimeout(700);
      await p.save("down");
      await p.mode("play", 8_000);
      report.fallen = (await p.status()).match.deaths;
      expect(`the player fell once (${report.fallen})`, report.fallen === 1);
      // The clock run out: the results.
      await p.say("god=1 clock=1");
      await p.mode("results", 12_000);
      await p.page.waitForTimeout(900);
      await p.save("results");
      // The results' first row plays again.
      await p.key("KeyZ");
      await p.mode("play");
      expect(`no error on the page (${p.problems.join("; ")})`, p.problems.length === 0);
      await p.page.close();
    }

    // ---- another map, chosen from the list: its pack takes the place of the first map's, and the match
    // starts on it when it is there; then the first map again, from the title
    report.maps = {};
    for (const other of offered.slice(1)) {
      const p = await visit("");
      await p.up();
      await p.mode("title");
      await p.page.waitForTimeout(400);
      await p.key("KeyZ");
      await p.page.waitForTimeout(400);
      for (let row = 0; row < offered.indexOf(other); row++) await p.key("ArrowDown");
      // The shell lets its world go and says what it reads. (From this computer's own disk the read is
      // a few frames long: whether the loading screen was caught is recorded, and not required.)
      const from = performance.now();
      await p.page.keyboard.down("KeyZ");
      const caught = await p.page.waitForFunction(`JSON.parse(pocketDuty.duty.status()).stage === "loading"`, undefined, { timeout: 3_000, polling: 8 }).then(() => true, () => false);
      if (caught) await p.save(`${other.id}-reading`);
      await p.page.keyboard.up("KeyZ");
      const reading = await p.status();
      expect(`${other.id}: chosen from the list, its pack is the one read (${reading.stage}, ${reading.map})`, reading.map === other.id);
      await p.page.waitForFunction(`JSON.parse(pocketDuty.duty.status()).stage === "running"`, undefined, { timeout: 90_000 });
      await p.mode("play");
      // (from the key to the match on the other map: the read, the upload to the GPU, the first frame)
      const playingAfter = Math.round(performance.now() - from);
      const there = await p.status();
      expect(`${other.id}: the match is on its pack (${there.map}, ${there.pack.bytes} bytes)`, there.map === other.id && there.pack.bytes === statSync(other.file).size);
      await p.page.waitForTimeout(700);
      await p.save(`${other.id}-play`);
      await p.page.screenshot({ path: join(directory, `page-${other.id}-play.png`) });
      // A firefight on it: the bench's player in front of a soldier.
      await p.say("god=1 near=0,7 auto=1");
      await p.page.waitForFunction("JSON.parse(pocketDuty.duty.status()).match.kills > 0", undefined, { timeout: 30_000 }).catch(() => {});
      const fought = await p.status();
      expect(`${other.id}: the bench's player fells a soldier (${fought.match.kills} kills, ${fought.match.hits} hits of ${fought.match.shots})`, fought.match.kills > 0);
      await p.save(`${other.id}-firefight`);
      const read = `({ frames: pocketDuty.frames, at: performance.now() })`;
      const a = (await p.page.evaluate(read)) as { frames: number; at: number };
      await p.page.waitForTimeout(seconds * 1000);
      const b = (await p.page.evaluate(read)) as typeof a;
      const now = await p.status();
      report.maps[other.id] = { loadingCaught: caught, playingAfterMs: playingAfter, bytes: there.pack.bytes, fps: +((b.frames - a.frames) / ((b.at - a.at) / 1000)).toFixed(2), frameMs: +((await p.page.evaluate("pocketDuty.burst(300)")) as number).toFixed(3), draws: now.draws, tris: now.tris, late: now.late, match: fought.match };
      // The menu's last row leaves for the title; its list's first row is the first map, read again.
      await p.say("auto=0");
      await p.key("Space");
      await p.mode("menu");
      await p.say("ui=title");
      await p.mode("title");
      await p.page.waitForTimeout(400);
      await p.save(`${other.id}-title`);
      await p.key("KeyZ");
      await p.page.waitForTimeout(400);
      await p.save(`${other.id}-maps`);
      await p.key("KeyZ");
      await p.page.waitForFunction(`(() => { const s = JSON.parse(pocketDuty.duty.status()); return s.stage === "running" && s.map === ${JSON.stringify(offered[0]!.id)}; })()`, undefined, { timeout: 90_000 });
      await p.mode("play");
      const back = await p.status();
      expect(`${other.id}: and the first map again (${back.map}, ${back.pack.bytes} bytes)`, back.map === offered[0]!.id && back.pack.bytes === statSync(offered[0]!.file).size);
      expect(`${other.id}: no error on the page (${p.problems.join("; ")})`, p.problems.length === 0);
      await p.page.close();
    }

    // ---- the same match as a Nintendo 3DS: its two screens in its shell, the lower screen's keys under
    // the pointer, and another device chosen from the bar while the match runs
    {
      const heard = opens.length;
      const p = await visit("?device=3ds");
      await p.up();
      await p.mode("title");
      await p.page.waitForTimeout(600);
      const told = opens.slice(heard);
      expect(`3ds: the page tells the host it opened as a 3DS (${JSON.stringify(told)})`, deployed ? told.length === 0 : told.length === 1 && told[0]!.query === "?app=local&layout=3ds");
      const shape = (await p.status()).shape;
      expect(`3ds: the renderer draws the upper screen's 400 by 240 (${JSON.stringify(shape)})`, shape.name === "3ds" && shape.width === 400 && shape.height === 240);
      const screens = (await p.page.evaluate(`[...document.querySelectorAll("[data-pocket-screen]:not([hidden])")].map((c) => [c.dataset.pocketScreen, c.width, c.height])`)) as [string, number, number][];
      expect(`3ds: the shell holds both screens (${JSON.stringify(screens)})`, JSON.stringify(screens) === JSON.stringify([["upper", 400, 240], ["lower", 320, 240]]));
      const lowerSaved = async (name: string) => {
        const url = (await p.page.evaluate("pocketDuty.captureLower()")) as string;
        writeFileSync(join(directory, `${name}.png`), Buffer.from(url.slice(url.indexOf(",") + 1), "base64"));
      };
      await p.save("3ds-title");
      await lowerSaved("3ds-title-lower");
      await p.page.screenshot({ path: join(directory, "page-3ds-title.png") });
      // The lower screen's START key, under the pointer, starts the match.
      const low = (await p.page.locator("[data-pocket-screen=lower]").boundingBox())!;
      const tap = async (x: number, y: number) => {
        await p.page.mouse.move(low.x + (x / 320) * low.width, low.y + (y / 240) * low.height);
        await p.page.mouse.down();
        await p.page.waitForTimeout(150);
        await p.page.mouse.up();
        await p.page.waitForTimeout(400);
      };
      await tap(280, 217);
      if (several) {
        // The list of maps is up on the upper screen; the lower screen's key plays the map behind it.
        await p.page.waitForTimeout(400);
        await p.save("3ds-maps");
        await lowerSaved("3ds-maps-lower");
        await tap(280, 217);
      }
      await p.mode("play");
      await p.say("god=1");
      // The interface drew the level from above on the lower screen: it is not one colour.
      const colours = (await p.page.evaluate(`(() => { const c = document.querySelector("[data-pocket-screen=lower]"); const d = c.getContext("2d").getImageData(0, 0, 240, 240).data; const seen = new Set(); for (let i = 0; i < d.length; i += 4 * 97) seen.add((d[i] << 16) | (d[i + 1] << 8) | d[i + 2]); return seen.size; })()`)) as number;
      expect(`3ds: the lower screen shows the level from above (${colours} colours)`, colours > 20);
      // Its keys: the next weapon, the fire mode, the menu.
      const before = await p.status();
      await tap(280, 107);
      await p.page.waitForFunction(`JSON.parse(pocketDuty.duty.status()).match.weapon !== ${before.match.weapon}`, undefined, { timeout: 5_000 }).catch(() => {});
      const armed = await p.status();
      expect(`3ds: the lower screen's WEAPON key takes the next weapon (${before.match.weapon} to ${armed.match.weapon})`, armed.match.weapon !== before.match.weapon);
      await p.page.waitForTimeout(900);
      await p.save("3ds-play");
      await lowerSaved("3ds-play-lower");
      await p.page.screenshot({ path: join(directory, "page-3ds-play.png") });
      // The upper screen is played by the same keys as a PSP's: R fires.
      await p.page.keyboard.down("KeyE");
      await p.page.waitForTimeout(500);
      await p.page.keyboard.up("KeyE");
      expect(`3ds: R fires (${(await p.status()).match.shots} rounds)`, (await p.status()).match.shots > 0);
      await tap(280, 217);
      await p.mode("menu");
      await p.save("3ds-menu");
      await p.key("KeyZ");
      await p.mode("play");
      // The mark's sentence names the device shown.
      const mark = (await p.page.locator("[data-pocket-mark]").first().boundingBox())!;
      await p.page.mouse.move(mark.x + mark.width / 2, mark.y + mark.height / 2);
      await p.page.waitForTimeout(300);
      const said = (await p.page.evaluate(`document.getElementById("pocket-simulated").textContent`)) as string;
      expect(`3ds: the mark says the picture is simulated ("${said}")`, said === "Your browser draws this picture. A real Nintendo 3DS draws Pocket Duty with its own hardware, so it looks and runs differently there.");
      await p.page.mouse.move(4, 400);
      // Another device from the bar: the match goes on in a PSP's shell, with one screen.
      const tick = (await p.status()).match.tick;
      await p.page.evaluate(`pocketDuty.present("psp")`);
      await p.page.waitForFunction(`pocketDuty.device() === "psp" && pocketDuty.interface() && JSON.parse(pocketDuty.duty.status()).shape.name === "psp"`, undefined, { timeout: 20_000 });
      await p.page.waitForTimeout(800);
      const after = await p.status();
      const left = (await p.page.evaluate(`[...document.querySelectorAll("[data-pocket-screen]:not([hidden])")].map((c) => [c.dataset.pocketScreen, c.width, c.height])`)) as [string, number, number][];
      expect(`3ds: a PSP chosen while the match runs keeps the match (${after.mode}, tick ${tick} to ${after.match.tick}) on one screen (${JSON.stringify(left)})`, after.mode === "play" && after.match.tick > tick && after.match.weapon === armed.match.weapon && JSON.stringify(left) === JSON.stringify([["upper", 480, 272]]));
      await p.save("3ds-then-psp");
      expect(`3ds: no error on the page (${p.problems.join("; ")})`, p.problems.length === 0);
      await p.page.close();
    }

    // ---- the player around the game, in three windows: the PSP in its shell, the mark that says the
    // picture is simulated, the dock's Download and Remix, the shell's own key under the pointer
    report.player = {};
    const WINDOWS = [
      { name: "1440", options: { viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 }, coarse: false },
      { name: "1280", options: { viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1 }, coarse: false },
      { name: "phone", options: { viewport: { width: 390, height: 844 }, deviceScaleFactor: 3, hasTouch: true, isMobile: true }, coarse: true },
    ];
    for (const window of WINDOWS) {
      const context = await browser.newContext(window.options);
      const heard = opens.length;
      const p = await visit("", context);
      await p.up();
      const tag = window.name;
      // The host is told once that a player opened, as this device: the host here names the address in
      // /app.json; the deployable directory's names none, and its page tells no one.
      const told = opens.slice(heard);
      expect(`${tag}: the page tells the host once that a player opened (${JSON.stringify(told)})`, deployed ? told.length === 0 : told.length === 1 && told[0]!.query === "?app=local&layout=psp" && told[0]!.mode === "no-cors");
      const seen = (await p.page.evaluate(`(async () => {
        const box = (selector) => { const el = document.querySelector(selector); if (!el || el.hidden) return null; const r = el.getBoundingClientRect(); return [r.left, r.top, r.right, r.bottom].map((n) => +n.toFixed(2)); };
        const art = document.querySelector("[data-pocket-shell-art]");
        await art.decode().catch(() => {});
        const covered = [...document.querySelectorAll("[data-pocket-screen]:not([hidden])")].map((canvas) => {
          const r = canvas.getBoundingClientRect();
          const top = document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2);
          return top === canvas ? "" : (top?.outerHTML ?? "").slice(0, 80);
        });
        const screen = document.querySelector("[data-pocket-screen=upper]").getBoundingClientRect();
        return { view: [innerWidth, innerHeight], bar: box("[data-pocket-bar]"), stage: box("[data-pocket-stage=root]"), dock: box("[data-pocket-studio]"), shell: box("[data-pocket-shell]"), covered,
          screen: [screen.width, screen.height], scroll: [document.documentElement.scrollWidth, document.documentElement.scrollHeight] };
      })()`)) as Record<string, any>;
      const { shell, stage, bar, dock } = seen;
      report.player[tag] = { screen: seen.screen };
      expect(`${tag}: the page is one window, with no scroll (${seen.scroll} in ${seen.view})`, seen.scroll[0] <= seen.view[0] && seen.scroll[1] <= seen.view[1]);
      expect(`${tag}: the shell is whole inside the stage, under the bar and over the dock (${JSON.stringify({ shell, stage, bar, dock })})`,
        shell[0] >= stage[0] - 0.5 && shell[2] <= stage[2] + 0.5 && shell[1] >= bar[3] - 0.5 && shell[3] <= dock[1] + 0.5 && shell[2] - shell[0] > 100);
      expect(`${tag}: nothing lies over the screen (${seen.covered})`, seen.covered.every((over: string) => over === ""));
      await p.page.screenshot({ path: join(directory, `player-${window.name}.png`) });

      // The mark beside the device's name: a pointer that rests on it, or a finger, shows what it says.
      const mark = (await p.page.locator("[data-pocket-mark]").first().boundingBox())!;
      if (window.coarse) await p.page.touchscreen.tap(mark.x + mark.width / 2, mark.y + mark.height / 2);
      else await p.page.mouse.move(mark.x + mark.width / 2, mark.y + mark.height / 2);
      await p.page.waitForTimeout(300);
      const said = (await p.page.evaluate(`(() => { const tip = document.getElementById("pocket-simulated"); return { hidden: tip.hidden, text: tip.textContent }; })()`)) as Record<string, any>;
      expect(`${tag}: the mark says the picture is simulated, in the player's one sentence (${JSON.stringify(said)})`, said.hidden === false && said.text === "Your browser draws this picture. A real PSP draws Pocket Duty with its own hardware, so it looks and runs differently there.");
      if (window.name === "1440") await p.page.screenshot({ path: join(directory, "player-simulated.png") });
      if (window.coarse) await p.page.touchscreen.tap(mark.x + mark.width / 2, mark.y + mark.height / 2);
      else await p.page.mouse.move(4, seen.view[1] / 2);

      // The dock: Download leads to the game's page on Pocket Studio with its files open, Remix to the same
      // page with its remix open. The host here answers /app.json with the game's address there; the
      // deployable directory's answers none, and its page knows the Studio alone: Download leads to its
      // front door, and nothing says the game can be remixed.
      const links = (await p.page.evaluate(`Object.fromEntries(["get", "remix", "make"].map((action) => { const link = document.querySelector('[data-pocket-action=' + action + ']'); return [action, { href: link.href, text: link.textContent, hidden: link.hidden }]; }))`)) as Record<string, { href: string; text: string; hidden: boolean }>;
      report.player.dock ??= links;
      const [get, remix] = [new URL(links.get!.href), new URL(links.remix!.href)];
      const studio = deployed?.studio?.server ?? "https://studio.pocket.nexus";
      expect(`${tag}: the dock's first link is Download (${JSON.stringify(links.get)})`, !links.get!.hidden && links.get!.text === "Download" && get.origin === studio && get.searchParams.get("from") === "player" && (deployed ? get.pathname === "/" : get.pathname === "/games/duty" && get.searchParams.has("get")));
      expect(`${tag}: Remix is there when the host says the game can be remixed (${JSON.stringify(links.remix)})`, deployed ? links.remix!.hidden : !links.remix!.hidden && links.remix!.text === "Remix" && remix.pathname === "/games/duty" && remix.searchParams.has("remix"));
      expect(`${tag}: the Studio's front door stands in the dock only when it has no other link (${JSON.stringify(links.make)})`, links.make!.hidden);

      // From the title into a match by the shell's own key, which goes down while it is held.
      if (!window.coarse) {
        const key = (await p.page.locator("[data-pocket-control=circle]").first().boundingBox())!;
        await p.page.mouse.move(key.x + key.width / 2, key.y + key.height / 2);
        await p.page.mouse.down();
        await p.page.waitForTimeout(160);
        const down = (await p.page.evaluate(`document.querySelectorAll("[data-pocket-part][data-held]").length`)) as number;
        await p.page.mouse.up();
        expect(`${tag}: the shell's key goes down under the pointer (${down})`, down >= 1);
        await onTheFirst(p);
        await p.mode("play");
      } else {
        // Under a finger: the shell's key at the right starts the match.
        // (held for a tenth of a second, as a thumb holds it: a frame reads what is down when it is made)
        const key = (await p.page.locator("[data-pocket-control=circle]").first().boundingBox())!;
        const touch = await p.page.context().newCDPSession(p.page);
        const thumb = async () => {
          await touch.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ x: key.x + key.width / 2, y: key.y + key.height / 2 }] });
          await p.page.waitForTimeout(120);
          await touch.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
        };
        await thumb();
        if (several) {
          // (the list of maps is up: the same key plays its first row, the map behind the title)
          await p.page.waitForTimeout(450);
          await thumb();
        }
        await p.mode("play");
      }
      expect(`${tag}: no error on the page (${p.problems.join("; ")})`, p.problems.length === 0);
      await p.page.close();
      await context.close();
    }

    // ---- what the first frame of the street needs: the page on a line of 16 Mbit/s
    {
      const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
      const cdp = await page.context().newCDPSession(page);
      await cdp.send("Network.enable");
      await cdp.send("Network.emulateNetworkConditions", { offline: false, latency: 20, downloadThroughput: 2_000_000, uploadThroughput: 1_000_000 });
      let received = 0;
      let counting = true;
      cdp.on("Network.dataReceived", (event: { encodedDataLength: number; dataLength: number }) => {
        if (counting) received += event.encodedDataLength || event.dataLength;
      });
      await page.goto(`${origin}/`);
      await page.waitForFunction("window.pocketDuty?.firstFrame > 0", undefined, { timeout: 60_000 });
      await page.waitForTimeout(3500);
      await page.screenshot({ path: join(directory, "reading-the-street.png") });
      await page.waitForFunction("window.pocketDuty && (pocketDuty.firstWorld || pocketDuty.failure)", undefined, { timeout: 180_000 });
      counting = false;
      const first = (await page.evaluate("({ ready: pocketDuty.ready, interfaceReady: pocketDuty.interfaceReady, firstFrame: pocketDuty.firstFrame, firstWorld: pocketDuty.firstWorld, failure: pocketDuty.failure })")) as Record<string, any>;
      await page.screenshot({ path: join(directory, "first-street-frame.png") });
      report.firstFrame = { bytesPerSecond: 2_000_000, bytesReceived: received, interfaceMs: Math.round(first.interfaceReady), firstFrameMs: Math.round(first.firstFrame), firstWorldFrameMs: Math.round(first.firstWorld), failure: first.failure };
      await page.close();
    }

    // ---- a browser without WebGPU is told so in one sentence, after the card
    {
      const without = await browser.newPage();
      await without.addInitScript("Object.defineProperty(Navigator.prototype, 'gpu', { get: undefined, configurable: true }); delete Navigator.prototype.gpu;");
      await without.goto(`${origin}/`);
      await without.waitForFunction("document.querySelector('[data-pocket-say]')?.textContent !== ''", undefined, { timeout: 20_000 });
      report.withoutWebGPU = await without.locator("[data-pocket-say]").textContent();
      expect(`a browser without WebGPU is told so ("${report.withoutWebGPU}")`, report.withoutWebGPU === "This browser has no WebGPU, which Pocket Duty draws with.");
      await without.close();
    }
  } finally {
    await browser.close();
    server.stop(true);
    report.sizes = sizes;
    report.opened = { reports: opens.length, last: opens.at(-1) ?? null };
    writeFileSync(join(directory, "report.json"), JSON.stringify(report, null, 1));
  }
  const { sizes: _, ...brief } = report;
  console.log(JSON.stringify({ ...brief, module: sizes["pkg/duty_wgpu_bg.wasm"], uiCore: sizes["pocketjs.wasm"] }, null, 1));
  console.log(directory);
} else if (import.meta.main) throw new Error("usage: build | serve [--port N] [--dist] [--build] | dist [--piece MiB] | shot [--map ID] [--out PNG] [--size WxH] [--frames N] [--words WORDS] [--cue N:WORDS] | check [--headed] [--seconds N] [--dist]");
