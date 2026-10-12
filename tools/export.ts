// DutyIR export: opens web/export.html in headless Chrome and writes what the page sends into
// .pocket-build/duty/ir.
//
//   bun tools/duty.ts export [--only world,collision,nav,sky,scene,…] [--q high]

import { spawn } from "node:child_process";
import { mkdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { chromium } from "playwright-core";

const ROOT = resolve(import.meta.dir, "..");
const PORT = 5390;
const SINK = 5391;

async function up(): Promise<boolean> {
  try {
    return (await fetch(`http://127.0.0.1:${PORT}/export.html`)).ok;
  } catch {
    return false;
  }
}

/** Starts the Vite server of web/ unless one already answers on the port. It keeps running for later commands. */
export async function serveWeb() {
  if (await up()) return;
  const child = spawn("bunx", ["vite", "--port", String(PORT), "--strictPort", "--host", "127.0.0.1"], { cwd: join(ROOT, "web"), detached: true, stdio: "ignore" });
  child.unref();
  for (let i = 0; i < 100; i++) {
    if (await up()) return;
    await Bun.sleep(200);
  }
  throw new Error("vite did not start");
}

export const IR = join(ROOT, ".pocket-build/duty/ir");

/** Chrome with a GPU: the reference forges its textures and its sky in fragment programs. */
export async function launch() {
  return chromium.launch({ channel: "chrome", headless: true, args: ["--use-angle=metal", "--enable-gpu", "--ignore-gpu-blocklist", "--no-proxy-server", "--autoplay-policy=no-user-gesture-required"] });
}

export async function exportIr(argv: string[]) {
  const arg = (name: string) => {
    const i = argv.indexOf(`--${name}`);
    return i >= 0 ? argv[i + 1] : undefined;
  };
  mkdirSync(IR, { recursive: true });

  let received = 0;
  let bytes = 0;
  let finished: () => void = () => {};
  const done = new Promise<void>((r) => (finished = r));
  const cors = { "Access-Control-Allow-Origin": "*", "Access-Control-Allow-Methods": "PUT, OPTIONS", "Access-Control-Allow-Headers": "*" };
  const server = Bun.serve({
    port: SINK,
    hostname: "127.0.0.1",
    maxRequestBodySize: 1 << 30,
    async fetch(req) {
      if (req.method === "OPTIONS") return new Response(null, { headers: cors });
      const name = new URL(req.url).pathname.slice(1);
      if (req.method !== "PUT" || !/^[\w.-]+$/.test(name)) return new Response("no", { status: 400, headers: cors });
      if (name === "done") finished();
      else {
        const body = await req.arrayBuffer();
        await Bun.write(join(IR, name), body);
        received++;
        bytes += body.byteLength;
      }
      return new Response("ok", { headers: cors });
    },
  });

  await serveWeb();
  const query = new URLSearchParams({ sink: `http://127.0.0.1:${SINK}` });
  for (const k of ["only", "q"]) if (arg(k)) query.set(k, arg(k)!);
  const browser = await launch();
  const t0 = performance.now();
  try {
    const page = await browser.newPage({ viewport: { width: 960, height: 544 }, deviceScaleFactor: 1 });
    const errors: string[] = [];
    page.on("pageerror", (e: Error) => errors.push(String(e)));
    page.on("console", (m: { type(): string; text(): string }) => {
      if (m.type() === "error") errors.push(m.text());
      else if (m.type() === "log") console.log(`  ${m.text()}`);
    });
    await page.goto(`http://127.0.0.1:${PORT}/export.html?${query}`);
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
    } finally {
      over = true;
      clearTimeout(timer);
    }
    if (errors.length) console.log("page errors:", errors.slice(0, 4).join(" | "));
  } finally {
    await browser.close();
    server.stop(true);
  }
  console.log(`export: ${received} files, ${(bytes / 1e6).toFixed(1)} MB in ${((performance.now() - t0) / 1000).toFixed(1)} s → ${IR}`);
}
