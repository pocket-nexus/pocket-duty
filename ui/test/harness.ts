// Runs a compiled interface bundle on the host: PocketJS's wasm core and
// software rasterizer, with a renderer that exists only as state (`Mock`).
// It answers commands the way a device's renderer does, so a test can press
// buttons and read what the interface drew and asked for.
import { existsSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { createCanvas, ImageData, loadImage, type Canvas } from "../../vendor/pocketjs/node_modules/@napi-rs/canvas";
import { PROP } from "../../vendor/pocketjs/contracts/spec/spec.ts";
import { createBakedFontCoverage } from "../../vendor/pocketjs/framework/src/font-coverage.ts";
import { entries, loadPack } from "../../vendor/pocketjs/framework/src/pak.ts";
import { __packTouch, createTouchHitFacts } from "../../vendor/pocketjs/framework/src/touch.ts";
import { createWasmUi } from "../../vendor/pocketjs/hosts/web/wasm-ops.js";
import { BLIPS, FLAG, T, type Command, type HostState, type Kill, type Result } from "../app/protocol.ts";

const root = resolve(import.meta.dir, "../..");
export type Device = "psp" | "vita";

/** The numbers of a match that has just begun. */
function fresh(): number[] {
  const t = new Array<number>(T.blip + BLIPS * 2).fill(0);
  t[T.health] = 100;
  t[T.ammo] = 30;
  t[T.reserve] = 120;
  t[T.heading] = 0;
  t[T.x] = 640;
  t[T.z] = 900;
  t[T.spread] = 6;
  t[T.clock] = 600;
  return t;
}

/** A device's renderer, reduced to the state the interface sees. */
export class Mock {
  state: HostState = {
    mode: "loading", message: "Reading the level",
    options: [
      { key: "look", value: 1, choices: ["Slow", "Medium", "Fast"] },
      { key: "invert", value: 0 },
      { key: "assist", value: 1 },
      { key: "layout", value: 0, choices: ["Buttons look", "Stick looks"] },
      { key: "skill", value: 1, choices: ["Recruit", "Regular", "Veteran"] },
      { key: "stats", value: 0 },
    ],
    stats: "", prefs: "", woke: 0, weapon: "M4A1", fire: "AUTO", feed: [], banner: [0, "", ""], result: null, t: fresh(),
  };
  /** Every command received, oldest first. */
  log: Command[] = [];
  /** The interface said it has nothing scheduled. */
  idle = false;
  /** Seconds this renderer has run, and what the interface asked to be woken for. */
  private seconds = 0;
  private wakes: { id: number; due: number }[] = [];
  /** The most wakes that were pending at once. */
  most = 0;
  private sent: Partial<HostState> = {};
  private hits = 0;
  private hurts = 0;
  private kills = 0;
  private banners = 0;

  /** Forgets a wake without saying it: what a renderer does when two come due between two turns
   *  and its state member holds the later one. */
  lose(id: number) {
    this.wakes = this.wakes.filter((wake) => wake.id !== id);
  }

  /** One frame of the renderer's clock: a wake that is due is said back, one a frame. */
  tick(dt: number) {
    this.seconds += dt;
    const at = this.wakes.findIndex((wake) => wake.due <= this.seconds);
    if (at >= 0) this.state.woke = this.wakes.splice(at, 1)[0].id;
  }

  /** A match on the same ground with nothing scored. */
  private begin() {
    this.hits = this.hurts = this.kills = 0;
    Object.assign(this.state, { mode: "play", feed: [], banner: [0, "", ""], result: null, weapon: "M4A1", fire: "AUTO", t: fresh() });
  }

  receive(command: Command) {
    this.log.push(command);
    const s = this.state;
    switch (command.type) {
      case "start":
        if (s.mode === "title") this.begin();
        break;
      case "menu":
        if (s.mode === "play" && command.on) s.mode = "menu";
        else if (s.mode === "menu" && !command.on) s.mode = "play";
        break;
      case "restart":
        if (s.mode === "menu" || s.mode === "results") this.begin();
        break;
      case "title": s.mode = "title"; break;
      case "option":
        s.options = s.options.map((setting) => (setting.key === command.key ? { ...setting, value: command.value } : setting));
        s.stats = s.options.find((setting) => setting.key === "stats")?.value ? "30.0 fps · 33.3 ms · late 0 · 412 draws · 9.6k tris" : "";
        break;
      case "prefs": s.prefs = command.value; break;
      case "idle": this.idle = command.on; break;
      case "wake":
        this.wakes.push({ id: command.id, due: this.seconds + command.seconds });
        this.most = Math.max(this.most, this.wakes.length);
        break;
    }
  }

  /** The level has loaded: the title is up. */
  loaded() {
    Object.assign(this.state, { mode: "title", message: "" });
  }

  /** Numbers of this moment, by name: `set({ health: 62, heading: 135 })`. */
  set(values: Partial<Record<Exclude<keyof typeof T, "blip">, number>>) {
    const t = [...this.state.t];
    for (const [name, value] of Object.entries(values)) t[T[name as keyof typeof T]] = value as number;
    this.state.t = t;
  }

  flag(bits: number, on: boolean) {
    this.set({ flags: on ? this.state.t[T.flags] | bits : this.state.t[T.flags] & ~bits });
  }

  /** The enemies the map shows, as decimetre pairs. */
  blips(places: [number, number][]) {
    const t = [...this.state.t];
    t[T.blips] = Math.min(BLIPS, places.length);
    places.slice(0, BLIPS).forEach(([x, z], i) => {
      t[T.blip + i * 2] = x;
      t[T.blip + i * 2 + 1] = z;
    });
    this.state.t = t;
  }

  /** A shot of the player's lands: 0 a body, 1 a head, 2 a kill. */
  hit(kind: 0 | 1 | 2) {
    this.set({ hit: ++this.hits * 4 + kind });
  }

  /** The player is hit from `degrees` clockwise of straight ahead. */
  hurt(degrees: number, health: number) {
    this.set({ hurt: ++this.hurts * 360 + (((degrees % 360) + 360) % 360), health });
  }

  kill(by: string, whom: string, head = false, mine = 0) {
    const line: Kill = { id: ++this.kills, by, whom, head, mine };
    this.state.feed = [line, ...this.state.feed].slice(0, 4);
  }

  banner(title: string, subtitle = "") {
    this.state.banner = [++this.banners, title, subtitle];
  }

  /** The player is killed, and is put back in `seconds`. */
  die(seconds: number) {
    this.state.mode = "down";
    this.set({ health: 0, respawn: Math.round(seconds * 10), spread: 0 });
  }

  respawn() {
    this.state.mode = "play";
    this.set({ health: 100, respawn: 0, spread: 6, ammo: 30 });
  }

  /** The match ends. */
  end(result: Result) {
    this.state.result = result;
    this.state.mode = "results";
  }

  /** The line of state members to deliver, or undefined while nothing changed. */
  poll(): string | undefined {
    const changed: Record<string, unknown> = {};
    for (const [key, value] of Object.entries(this.state)) {
      const text = JSON.stringify(value);
      if (JSON.stringify((this.sent as Record<string, unknown>)[key]) === text) continue;
      changed[key] = (this.sent as Record<string, unknown>)[key] = JSON.parse(text);
    }
    return Object.keys(changed).length ? JSON.stringify({ type: "state", value: changed }) : undefined;
  }
}

export { FLAG };

const VIEW: Record<Device, { w: number; h: number; density: number }> = {
  psp: { w: 480, h: 272, density: 1 },
  vita: { w: 480, h: 272, density: 2 },
};

export interface Rig {
  mock: Mock;
  view: (typeof VIEW)[Device];
  /** Advance `frames`, holding `buttons`; `touch` is the contacts on the panel. */
  step(frames?: number, input?: { buttons?: number; touch?: { x: number; y: number; id?: number }[] }): void;
  /** Press and release. */
  press(buttons: number): void;
  tap(x: number, y: number): void;
  /** The screen over `backdrop`, at the device's density. */
  shot(backdrop?: string | Canvas): Promise<Canvas>;
  /** Whether a text node of the interface holds exactly `text` now. */
  shows(text: string): boolean;
  /** The characters the interface has shown that no font of the bundle holds. */
  missing(): string[];
}

/** Boots the bundle `bun tools/ui.ts <device>` wrote. One per process: the bundle owns `globalThis.frame`. */
export async function boot(device: Device): Promise<Rig> {
  const view = VIEW[device];
  const directory = join(root, ".pocket-build/ui", device);
  const wasmPath = join(root, "vendor/pocketjs/hosts/web/pocketjs.wasm");
  if (!existsSync(wasmPath)) throw new Error("run `bun tools/wasm.ts` in vendor/pocketjs first");
  const wasm = await createWasmUi(readFileSync(wasmPath), { width: view.w, height: view.h, rasterDensity: view.density });
  const mock = new Mock();
  const globals = globalThis as Record<string, any>;
  const pending: string[] = [];
  // Every text the interface hands the core: its characters for `missing()`, and what each node holds for `shows()`.
  const shown = new Set<string>();
  const held = new Map<number, string>();
  const ops = wasm.ops as any;
  for (const op of ["setText", "replaceText"]) {
    const native = ops[op];
    ops[op] = (id: number, text: string) => {
      for (const character of text) shown.add(character);
      held.set(id, text);
      return native(id, text);
    };
  }
  const destroy = ops.destroyNode;
  ops.destroyNode = (id: number) => {
    held.delete(id);
    return destroy(id);
  };
  Object.assign(ops, {
    svcOpen: (name: string) => name === "pocket.overlay",
    svcPoll: () => mock.poll(),
    svcSend: (line: string) => pending.push(line),
  });
  const pak = readFileSync(join(directory, "duty.pak"));
  globals.ui = ops;
  globals.__pak = pak.buffer.slice(pak.byteOffset, pak.byteOffset + pak.byteLength);
  (0, eval)(readFileSync(join(directory, "duty.js"), "utf8"));

  const facts = createTouchHitFacts((x, y) => ops.hitTestBounds(x, y));
  const frame = (buttons: number, touch: { x: number; y: number; id?: number }[]) => {
    mock.tick(1 / 60);
    const packed = touch.map((t) => __packTouch(t.id ?? 1, t.x, t.y));
    globals.frame(buttons, undefined, packed, facts(packed), packed.map(() => 0));
    for (const line of pending.splice(0)) mock.receive(JSON.parse(line));
    wasm.tick();
  };

  // The rasterizer has no alpha; render over black and over white and take
  // the difference (the root's own colour is the only thing that changes).
  const matte = (render: () => Uint8Array, rootNode: number, w: number, h: number, scale: number): Canvas => {
    ops.setProp(rootNode, PROP.bgColor, 0xff000000);
    const black = render().slice();
    ops.setProp(rootNode, PROP.bgColor, 0xffffffff);
    const white = render().slice();
    ops.setProp(rootNode, PROP.bgColor, 0);
    const pixels = new Uint8ClampedArray(black.length);
    for (let i = 0; i < black.length; i += 4) {
      const alpha = 255 - (white[i + 1] - black[i + 1]);
      pixels[i + 3] = alpha;
      for (let c = 0; c < 3; c++) pixels[i + c] = alpha ? Math.min(255, (black[i + c] * 255) / alpha) : 0;
    }
    const canvas = createCanvas(w * scale, h * scale);
    canvas.getContext("2d").putImageData(new ImageData(pixels, w * scale, h * scale), 0, 0);
    return canvas;
  };

  return {
    mock, view,
    step(frames = 1, input = {}) {
      for (let i = 0; i < frames; i++) frame(input.buttons ?? 0, input.touch ?? []);
    },
    press(buttons) {
      frame(buttons, []);
      frame(buttons, []);
      frame(0, []);
    },
    tap(x, y) {
      frame(0, [{ x, y }]);
      frame(0, [{ x, y }]);
      frame(0, []);
      frame(0, []);
    },
    async shot(backdrop) {
      const scale = view.density;
      const top = matte(() => wasm.renderScaled(scale), 1, view.w, view.h, scale);
      const canvas = createCanvas(view.w * scale, view.h * scale);
      const ctx = canvas.getContext("2d");
      ctx.fillStyle = "#000000";
      ctx.fillRect(0, 0, canvas.width, canvas.height);
      const image = typeof backdrop === "string" ? (existsSync(backdrop) ? await loadImage(backdrop) : undefined) : backdrop;
      if (image) {
        // The scene behind the interface: the capture cropped to this screen's shape.
        const width = Math.min(image.width, (image.height * view.w) / view.h), height = (width * view.h) / view.w;
        ctx.drawImage(image as any, (image.width - width) / 2, (image.height - height) / 2, width, height, 0, 0, view.w * scale, view.h * scale);
      }
      ctx.drawImage(top, 0, 0);
      return canvas;
    },
    shows(text) {
      for (const value of held.values()) if (value === text) return true;
      return false;
    },
    missing() {
      // The bundle's own fonts, read from its pak: a character is there when every size and weight
      // the interface uses holds it.
      loadPack(globals.__pak);
      const baked = createBakedFontCoverage();
      const slots = entries("ui:font.").map((key) => Number(key.slice("ui:font.".length)));
      return [...shown].filter((character) => character !== "\n" && slots.some((slot) => !baked(character, slot)));
    },
  };
}
