// DutyIR export: opens web/export.html in headless Chrome and writes what the page sends into a map's
// export directory (.pocket-build/duty/ir for the street).
//
//   bun tools/duty.ts export [--map yards] [--only world,collision,nav,sky,scene,…] [--q high]
//   bun tools/duty.ts look --map yards [--views maps/yards/views.json] [--out DIR] [--q high]
//
// Each run starts a dev server of its own for the map (web/src/pocket/vite.config.js puts the map's
// layout in the place of the reference's) on a free port, and stops it when it is done.

import { spawn, type ChildProcess } from "node:child_process";
import { existsSync, mkdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { chromium } from "playwright-core";
import { irDir, mapNamed, ROOT } from "./maps.ts";

async function freePort(): Promise<number> {
  const probe = Bun.serve({ port: 0, hostname: "127.0.0.1", fetch: () => new Response("") });
  const port = probe.port;
  probe.stop(true);
  return port!;
}

/** Starts the Vite server of web/ for `map` on a free port. `stop` ends it. */
export async function serveWeb(map: string): Promise<{ url: string; stop: () => void }> {
  const port = await freePort();
  const url = `http://127.0.0.1:${port}`;
  const env = { ...process.env };
  if (mapNamed(map).reference) delete env.DUTY_MAP;
  else env.DUTY_MAP = map;
  const child: ChildProcess = spawn("bunx", ["vite", "--config", "src/pocket/vite.config.js", "--port", String(port), "--strictPort", "--host", "127.0.0.1"], { cwd: join(ROOT, "web"), env, stdio: "ignore" });
  const stop = () => {
    if (!child.killed) child.kill();
  };
  for (let i = 0; i < 150; i++) {
    try {
      if ((await fetch(`${url}/export.html`)).ok) return { url, stop };
    } catch {
      // (not up yet)
    }
    if (child.exitCode !== null) break;
    await Bun.sleep(200);
  }
  stop();
  throw new Error("vite did not start");
}

/** Chrome with a GPU: the reference forges its textures and its sky in fragment programs. */
export async function launch() {
  return chromium.launch({ channel: "chrome", headless: true, args: ["--use-angle=metal", "--enable-gpu", "--ignore-gpu-blocklist", "--no-proxy-server", "--autoplay-policy=no-user-gesture-required"] });
}

/** A camera of `look`: level coordinates, metres. */
interface View {
  name: string;
  pos: [number, number, number];
  look: [number, number, number];
  fov?: number;
}

/**
 * Runs the export page for `map`. Without `views` the page's files go to the map's export directory.
 * With them nothing is exported: the page builds the map and the reference's own renderer draws it from
 * each view, a PNG a view in `out`.
 */
async function run(map: string, argv: string[], look?: { views: View[]; out: string }) {
  const arg = (name: string) => {
    const i = argv.indexOf(`--${name}`);
    return i >= 0 ? argv[i + 1] : undefined;
  };
  const into = irDir(map);
  if (!look) mkdirSync(into, { recursive: true });

  let received = 0;
  let bytes = 0;
  let finished: () => void = () => {};
  const done = new Promise<void>((r) => (finished = r));
  const cors = { "Access-Control-Allow-Origin": "*", "Access-Control-Allow-Methods": "PUT, OPTIONS", "Access-Control-Allow-Headers": "*" };
  const server = Bun.serve({
    // Any free port: two exports may run at once.
    port: 0,
    hostname: "127.0.0.1",
    maxRequestBodySize: 1 << 30,
    async fetch(req) {
      if (req.method === "OPTIONS") return new Response(null, { headers: cors });
      const name = new URL(req.url).pathname.slice(1);
      if (req.method !== "PUT" || !/^[\w.-]+$/.test(name)) return new Response("no", { status: 400, headers: cors });
      if (name === "done") finished();
      else if (!look) {
        const body = await req.arrayBuffer();
        await Bun.write(join(into, name), body);
        received++;
        bytes += body.byteLength;
      }
      return new Response("ok", { headers: cors });
    },
  });

  const web = await serveWeb(map);
  const query = new URLSearchParams({ sink: `http://127.0.0.1:${server.port}` });
  if (!mapNamed(map).reference) query.set("map", map);
  if (look) query.set("only", "look");
  for (const k of look ? ["q"] : ["only", "q"]) if (arg(k)) query.set(k, arg(k)!);
  const browser = await launch();
  const t0 = performance.now();
  try {
    const page = await browser.newPage({ viewport: { width: 960, height: 544 }, deviceScaleFactor: 1 });
    const errors: string[] = [];
    page.on("pageerror", (e: Error) => errors.push(String(e)));
    page.on("console", (m: { type(): string; text(): string }) => {
      if (m.type() === "error") errors.push(m.text());
      else if (m.type() === "log" || (look && m.type() === "info")) console.log(`  ${m.text()}`);
    });
    await page.goto(`${web.url}/export.html?${query}`);
    let over = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const failed = (async () => {
      while (!over) {
        await Bun.sleep(500);
        if (errors.some((e) => !/404|Failed to load resource/.test(e))) throw new Error(errors.join(" | "));
      }
    })();
    const late = new Promise<void>((_, reject) => (timer = setTimeout(() => reject(new Error("export timed out")), 20 * 60_000)));
    try {
      await Promise.race([done, failed, late]);
      if (look) {
        mkdirSync(look.out, { recursive: true });
        for (const view of look.views) {
          await page.evaluate((v) => (window as unknown as { __LOOK__: (v: unknown) => Promise<boolean> }).__LOOK__(v), view);
          await page.locator("#game").screenshot({ path: join(look.out, `${view.name}.png`) });
          console.log(`look: ${view.name}`);
        }
      }
    } finally {
      over = true;
      clearTimeout(timer);
    }
    if (errors.length) console.log("page errors:", errors.slice(0, 4).join(" | "));
  } finally {
    await browser.close();
    server.stop(true);
    web.stop();
  }
  if (look) console.log(`look: ${look.views.length} pictures of ${map} in ${((performance.now() - t0) / 1000).toFixed(1)} s → ${look.out}`);
  else console.log(`export: ${map}: ${received} files, ${(bytes / 1e6).toFixed(1)} MB in ${((performance.now() - t0) / 1000).toFixed(1)} s → ${into}`);
}

export async function exportIr(map: string, argv: string[]) {
  await run(map, argv);
}

/** Pictures of a map as the reference draws it, from the views in `--views` (default: the map's own `views.json`). */
export async function lookAt(map: string, argv: string[]) {
  const arg = (name: string, fallback: string) => {
    const i = argv.indexOf(`--${name}`);
    return i >= 0 && argv[i + 1] ? argv[i + 1]! : fallback;
  };
  const file = resolve(arg("views", join(ROOT, "maps", map, "views.json")));
  if (!existsSync(file)) throw new Error(`${file} is missing: a list of { name, pos, look, fov } in level coordinates`);
  const views = JSON.parse(readFileSync(file, "utf8")) as View[];
  await run(map, argv, { views, out: resolve(arg("out", join(ROOT, ".pocket-build/validation/maps", map, "reference"))) });
}
