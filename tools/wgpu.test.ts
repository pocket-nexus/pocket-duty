// What the browser build's tool takes for granted of the page and the renderer.
import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { DEVICES } from "./ui.ts";
import { ROOT } from "./wgpu.ts";

const read = (path: string) => readFileSync(join(ROOT, path), "utf8");

test("the page's handhelds are shapes of the renderer and devices the interface compiles for", () => {
  const shown = [...read("wgpu/page/main.js").matchAll(/^\s+id: "([a-z0-9]+)",$/gm)].map((m) => m[1]!);
  const shapes = [...read("wgpu/src/app.rs").matchAll(/Shape \{ name: "([a-z0-9]+)"/g)].map((m) => m[1]!);
  const built = read("tools/wgpu.ts").match(/const SHOWN = \[([^\]]*)\]/)![1]!.split(",").map((name) => name.trim().replaceAll('"', ""));
  expect(shown).toEqual(["psp"]);
  expect(shapes).toEqual(shown);
  expect(built).toEqual(shown);
  for (const id of shown) expect(DEVICES as readonly string[]).toContain(id);
});

test("the page names what `dist` rewrites: its pack, its script and its two stylesheets", () => {
  const page = read("wgpu/page/index.html");
  for (const part of [`<meta name="pocket-pack" content="duty.pack">`, `src="main.js"`, `href="pocket3d-stage.css"`, `href="pocket3d-player.css"`]) expect(page).toContain(part);
  // The body is the kernel's player's to build.
  expect(page).toMatch(/<body>\s*<script type="module" src="main.js"><\/script>\s*<\/body>/);
});

test("the sights' stick and the sound's rate are said where the page and the README say them", () => {
  const app = read("wgpu/src/app.rs");
  expect(app).toContain("const SIGHTS_STICK: f32 = 0.9;");
  expect(read("wgpu/README.md")).toContain("nine tenths (`SIGHTS_STICK` in `src/app.rs`)");
  expect(read("crates/duty-sim/src/mix.rs")).toContain("pub const OUTPUT_RATE: u32 = 22_050;");
  expect(read("wgpu/README.md")).toContain("22 050 frames a second");
});
