// What the browser build's tool takes for granted of the page and the renderer.
import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { DEVICES } from "./ui.ts";
import { ROOT } from "./wgpu.ts";

const read = (path: string) => readFileSync(join(ROOT, path), "utf8");

// The page's `id` of a handheld is a shell of PocketJS's player; a package's target is Pocket Studio's
// name for the device. They differ for one device.
const TARGET_OF: Record<string, string> = { ipod: "ipod-touch" };

/** What each place that lists the game's devices says. */
function platforms() {
  const page = read("wgpu/page/main.js");
  const devices = page.match(/^const DEVICES = \[\n?([\s\S]*?)\];$/m)![1]!;
  return {
    devices,
    // wgpu/page/main.js: the handhelds the page shows, and the targets it says the game has packages for
    shown: [...devices.matchAll(/\bid: "([a-z0-9-]+)"/g)].map((m) => m[1]!),
    runsOn: [...page.match(/^\s*runsOn: \[([^\]]*)\],$/m)![1]!.matchAll(/"([a-z0-9-]+)"/g)].map((m) => m[1]!),
    // wgpu/src/app.rs: the screens the renderer draws
    shapes: [...read("wgpu/src/app.rs").matchAll(/Shape \{ name: "([a-z0-9]+)"/g)].map((m) => m[1]!),
    // tools/wgpu.ts: the interface bundles the site is built with, and the packages its own host answers
    built: [...read("tools/wgpu.ts").match(/const SHOWN = \[([^\]]*)\]/)![1]!.matchAll(/"([a-z0-9-]+)"/g)].map((m) => m[1]!),
    hosted: [...read("tools/wgpu.ts").match(/^\s*packages: \[(.*)\],$/m)![1]!.matchAll(/target: "([a-z0-9-]+)"/g)].map((m) => m[1]!),
    // tools/release.ts: the packages a release builds
    released: [...read("tools/release.ts").match(/^const TARGETS = \[([^\]]*)\] as const;$/m)![1]!.matchAll(/"([a-z0-9-]+)"/g)].map((m) => m[1]!),
  };
}

test("the page's handhelds are shapes of the renderer and devices the interface compiles for", () => {
  const { devices, shown, shapes, built } = platforms();
  // (the player says one sentence for every game where a page of an earlier one gave a note a device)
  expect(devices).not.toContain("note");
  expect(shown.length).toBeGreaterThan(0);
  expect(shapes).toEqual(shown);
  expect(built).toEqual(shown);
  for (const id of shown) expect(DEVICES as readonly string[]).toContain(id);
});

// A device the game has a package for is a device the page plays as, and the other way round: a release
// that gains a target gains its shape, its interface bundle and its place in the page's list in the same
// change. (Pocket Studio's `site` and `package` commands hold the deployed page and the uploaded packages
// to the same rule.)
test("every package's device is one the page plays as, and every device of the page has a package", () => {
  const { shown, runsOn, hosted, released } = platforms();
  const targets = shown.map((id) => TARGET_OF[id] ?? id);
  expect([...released].sort()).toEqual([...targets].sort());
  expect(runsOn).toEqual(targets);
  expect([...hosted].sort()).toEqual([...targets].sort());
});

test("the page says which devices it plays as where Pocket Studio reads it", () => {
  const { shown } = platforms();
  const said = read("wgpu/page/index.html").match(/<meta name="pocket-devices" content="([^"]*)">/)?.[1];
  expect(said?.split(" ")).toEqual(shown.map((id) => TARGET_OF[id] ?? id));
});

test("the page names what `dist` rewrites: its packs, its script and its two stylesheets", () => {
  const page = read("wgpu/page/index.html");
  for (const part of [`<meta name="pocket-pack" content="duty.pack">`, `<meta name="pocket-packs" content="">`, `src="main.js"`, `href="pocket3d-stage.css"`, `href="pocket3d-player.css"`]) expect(page).toContain(part);
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
