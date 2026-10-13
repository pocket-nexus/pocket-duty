// Writes pictures of the interface on one device to .pocket-build/ui/preview/.
//   bun ui/test/preview.ts <psp|vita|3ds>     (after `bun tools/ui.ts <device>`)
//
// Each screen is laid over a picture of the scene: a capture of the device's
// own frame when one stands at .pocket-build/ui/backdrops/scene.png, else a
// street drawn here, sunlit and pale, which is the ground the readouts are
// hardest to read on.
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { BTN } from "../../vendor/pocketjs/contracts/spec/spec.ts";
import { createCanvas, loadImage, type Canvas } from "../../vendor/pocketjs/node_modules/@napi-rs/canvas";
import { boot, FLAG, type Device } from "./harness.ts";

const device = (process.argv[2] ?? "psp") as Device;
const build = resolve(import.meta.dir, "../../.pocket-build/ui");
const out = join(build, "preview");
mkdirSync(out, { recursive: true });
const rig = await boot(device);
const mock = rig.mock;

/** A market street at noon, as flat shapes: a pale sky, sunlit walls to a vanishing point, awnings, a bright road. */
function street(): Canvas {
  const w = 960, h = 544;
  const canvas = createCanvas(w, h);
  const ctx = canvas.getContext("2d");
  const sky = ctx.createLinearGradient(0, 0, 0, h * 0.55);
  sky.addColorStop(0, "#8fb7dc");
  sky.addColorStop(1, "#f1ede2");
  ctx.fillStyle = sky;
  ctx.fillRect(0, 0, w, h);
  const quad = (color: string, points: [number, number][]) => {
    ctx.fillStyle = color;
    ctx.beginPath();
    points.forEach(([x, y], i) => (i ? ctx.lineTo(x, y) : ctx.moveTo(x, y)));
    ctx.closePath();
    ctx.fill();
  };
  const vx = w * 0.52, vy = h * 0.47;
  // The road and the far end of the street.
  quad("#cfc2a6", [[0, h], [w, h], [vx + 46, vy + 6], [vx - 46, vy + 6]]);
  quad("#b9ab8e", [[w * 0.3, h], [w * 0.62, h], [vx + 10, vy + 6], [vx - 8, vy + 6]]);
  quad("#e4d8bf", [[vx - 60, vy - 96], [vx + 60, vy - 96], [vx + 60, vy + 6], [vx - 60, vy + 6]]);
  quad("#6a6253", [[vx - 20, vy - 30], [vx + 14, vy - 30], [vx + 14, vy + 6], [vx - 20, vy + 6]]);
  // Walls at the left in shade, at the right in the sun.
  quad("#a5916f", [[0, 0], [w * 0.2, h * 0.02], [vx - 60, vy - 96], [vx - 60, vy + 6], [0, h * 0.88]]);
  quad("#8a7758", [[0, h * 0.2], [w * 0.16, h * 0.24], [w * 0.16, h * 0.62], [0, h * 0.7]]);
  quad("#f3e7cc", [[w, 0], [w * 0.8, h * 0.03], [vx + 60, vy - 96], [vx + 60, vy + 6], [w, h * 0.9]]);
  quad("#e9d9b6", [[w, h * 0.1], [w * 0.86, h * 0.16], [w * 0.86, h * 0.66], [w, h * 0.74]]);
  // Doors, windows, awnings and crates.
  quad("#3d3830", [[w * 0.9, h * 0.42], [w * 0.97, h * 0.4], [w * 0.97, h * 0.8], [w * 0.9, h * 0.72]]);
  quad("#4a4336", [[w * 0.06, h * 0.36], [w * 0.12, h * 0.38], [w * 0.12, h * 0.62], [w * 0.06, h * 0.66]]);
  quad("#b8402f", [[w * 0.66, h * 0.36], [w * 0.84, h * 0.3], [w * 0.86, h * 0.4], [w * 0.67, h * 0.43]]);
  quad("#2f6f8f", [[w * 0.18, h * 0.34], [w * 0.36, h * 0.38], [w * 0.35, h * 0.45], [w * 0.17, h * 0.43]]);
  quad("#fbf6e8", [[w * 0.56, h * 0.33], [w * 0.64, h * 0.31], [w * 0.645, h * 0.37], [w * 0.565, h * 0.385]]);
  quad("#7b5a34", [[w * 0.7, h * 0.66], [w * 0.8, h * 0.66], [w * 0.81, h * 0.8], [w * 0.69, h * 0.8]]);
  quad("#5d4526", [[w * 0.22, h * 0.6], [w * 0.3, h * 0.6], [w * 0.3, h * 0.72], [w * 0.21, h * 0.72]]);
  // The sun on the road.
  quad("#fff7e0", [[w * 0.5, h * 0.62], [w * 0.74, h * 0.62], [w * 0.96, h], [w * 0.56, h]]);
  return canvas;
}

const capture = join(build, "backdrops/scene.png");
const scene: Canvas | string = existsSync(capture) ? capture : street();
if (typeof scene !== "string") console.log(`preview: no capture at ${capture}: a drawn street stands behind the interface`);

const names: string[] = [];
const save = async (name: string) => {
  const file = join(out, `${device}-${name}.png`);
  writeFileSync(file, (await rig.shot(scene)).toBuffer("image/png"));
  names.push(file);
};
const press = (button: number, frames = 6) => {
  rig.press(button);
  rig.step(frames);
};
/** Chooses the row `index` below the focus of the list that is up. */
const choose = (index: number) => {
  for (let i = 0; i < index; i++) rig.press(BTN.DOWN);
  press(BTN.CIRCLE, 12);
};

// Loading, and a level that could not be read.
rig.step(6);
await save("loading");
Object.assign(mock.state, { mode: "error", message: "No duty.pack beside the program" });
rig.step(4);
await save("error");

// The title and the two sheets it opens.
mock.loaded();
rig.step(20);
await save("title");
choose(2);
await save("about");
press(BTN.CROSS);
press(BTN.UP, 2);
press(BTN.CIRCLE, 12);
rig.press(BTN.DOWN);
rig.press(BTN.DOWN);
rig.step(10);
await save("settings");
press(BTN.CROSS);
press(BTN.UP, 2);
press(BTN.CIRCLE, 20);

// Play, at full health with a full magazine.
mock.set({ health: 100, ammo: 30, reserve: 120, heading: 0, x: 640, z: 900, spread: 6, us: 0, them: 0, clock: 600 });
rig.step(8);
await save("play");

// Later in the match: turned to the north-west, health and the magazine low, the feed full, statistics on.
mock.receive({ type: "option", key: "stats", value: 1 });
mock.set({ health: 48, ammo: 7, reserve: 90, heading: 304, x: 420, z: 520, spread: 11, us: 23, them: 19, clock: 371 });
mock.blips([[560, 380], [300, 610]]);
mock.kill("You", "Irregular", false, 1);
mock.kill("Grenade", "You", false, 2);
mock.kill("You", "Vanguard", true, 1);
mock.kill("Breacher", "You", false, 2);
rig.step(10);
await save("play-late");
mock.receive({ type: "option", key: "stats", value: 0 });

// A fight: health low, hit from the right, a kill landed, the banner up, enemies on the map, a reload running.
mock.set({ health: 22, ammo: 4, reserve: 60, heading: 138, x: 880, z: 300, spread: 14, us: 24, them: 21, clock: 356, reload: 62 });
mock.flag(FLAG.reloading, true);
mock.blips([[760, 240], [1010, 420], [900, 150], [640, 330]]);
mock.hurt(90, 22);
mock.hit(2);
mock.kill("You", "Breacher", true, 1);
mock.banner("Enemy down", "Headshot +150");
rig.step(13);
await save("fight");
// Hit from straight ahead under the same banner, and a shot to the head.
mock.hurt(0, 20);
mock.hit(1);
rig.step(12);
await save("fight-ahead");
rig.step(200);

// The magazine is empty: the prompt to reload.
mock.flag(FLAG.reloading, false);
mock.flag(FLAG.empty, true);
mock.set({ health: 71, ammo: 0, reserve: 60, heading: 221, spread: 8, reload: 0 });
mock.blips([]);
rig.step(320);
await save("empty");
mock.flag(FLAG.empty, false);

// Down.
mock.die(3.4);
rig.step(8);
await save("down");
mock.respawn();
rig.step(8);

// The menu over the match, and its settings.
press(BTN.START, 14);
await save("menu");
choose(1);
await save("menu-settings");
press(BTN.CROSS);
press(BTN.UP, 2);
press(BTN.CIRCLE, 10);

// The two ways a match ends.
mock.end({ won: true, us: 75, them: 62, kills: 18, deaths: 9, headshots: 6, accuracy: 37, seconds: 512 });
rig.step(12);
await save("results-won");
choose(0);
mock.end({ won: false, us: 58, them: 75, kills: 11, deaths: 14, headshots: 2, accuracy: 24, seconds: 600 });
rig.step(12);
rig.press(BTN.DOWN);
rig.step(10);
await save("results-lost");

// Every picture on one sheet, four across, at one sample a logical pixel.
const across = 4, w = rig.view.w, h = rig.view.h + (rig.view.aux?.[1] ?? 0), gap = 6;
const sheet = createCanvas(across * (w + gap) + gap, Math.ceil(names.length / across) * (h + gap) + gap);
const ctx = sheet.getContext("2d");
ctx.fillStyle = "#333333";
ctx.fillRect(0, 0, sheet.width, sheet.height);
for (const [i, file] of names.entries()) ctx.drawImage(await loadImage(file), gap + (i % across) * (w + gap), gap + Math.floor(i / across) * (h + gap), w, h);
const sheetPath = join(out, `sheet-${device}.png`);
writeFileSync(sheetPath, sheet.toBuffer("image/png"));
for (const file of [...names, sheetPath]) console.log(file);
const missing = rig.missing();
if (missing.length) throw new Error(`preview: no glyph for ${missing.map((character) => JSON.stringify(character)).join(" ")}`);
