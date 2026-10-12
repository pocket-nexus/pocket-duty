// Pocket Duty in a browser tab: the wgpu renderer (../src, built to pkg/) and
// the game's own interface (../../ui, the bundle the PSP loads, compiled by
// tools/ui.ts), which a realm of the page runs on PocketJS's UI core and the
// renderer lays over the street. The page itself is the Pocket3D player of
// PocketJS's browser kernel (vendor/pocketjs/devices/web/pocket-web-wgpu,
// staged beside this file): the bar, the PSP's shell with its keys, the dock
// that leads to the game's files and to a remix of it on Pocket Studio. This
// file says what the game is and draws into the player's canvas.
//
// The Pocket3D title card plays first; the interface and the pack are read
// while it plays.
//
// The address chooses the pack and what a development host would say:
//
//   ?pack=URL                   the pack (the PSP's): its file, on a server that answers byte ranges,
//                               or the manifest (.json) of one cut into pieces. Without it, the page's
//                               own (<meta name="pocket-pack">)
//   ?words=mode=play+auto=1     what a development host would send the game (App::control)
//   ?interface=off              the street alone, played by the pad: no guest is started
//   ?sound=off                  no sound
//
// `window.pocketDuty` is the running game, for a console and for tools/wgpu.ts.
import { playTitle } from "./pocket3d-title.js";
import { frames, hasWebGPU, titleCard } from "./pocket3d-shell.js";
import { openInterface, screens } from "./pocket3d-interface.js";
import { createPlayer } from "./pocket3d-player.js";
import init, { Duty, shapes, sound_rate } from "./pkg/duty_wgpu.js";

// The one handheld the game runs on. Its screen is the renderer's shape of the same name and its
// interface the bundle under ui/psp/.
const DEVICES = [{ id: "psp", label: "PSP", sticks: 1 }];
// Where the interface's settings are kept between visits (a PSP keeps them in a file beside the pack).
const KEPT = "pocket-duty.interface";

const query = new URLSearchParams(location.search);
const beside = (name) => new URL(name, import.meta.url).href;
const message = (error) => String(error?.message ?? error);
const started = performance.now();

// The page: PocketJS's player.
let device = DEVICES[0];
const player = createPlayer({
  title: "Pocket Duty",
  tagline: { en: "A firefight on a market street.", ja: "市場通りでの銃撃戦。" },
  devices: DEVICES,
  device: device.id,
  // (the targets `bun tools/release.ts` builds a package for)
  runsOn: ["psp"],
  pick: () => {},
});
const { canvas, stage, controls } = player;
const say = (text) => player.say(text);

function kept() {
  try {
    return localStorage.getItem(KEPT) ?? "";
  } catch {
    return "";
  }
}

async function start() {
  // The card is the first picture of every launch, and covers the page while the street is read. No frame
  // is drawn while it plays.
  const title = titleCard(playTitle);
  if (!hasWebGPU()) {
    await title;
    player.sayNoWebGPU();
    return;
  }
  await init();
  const shape = JSON.parse(shapes()).find((s) => s.name === device.id);

  // The shell, on the PSP's screen. It draws before there is a world: the interface says how much of the
  // pack has arrived.
  stage.show({ device: device.id, width: shape.width, height: shape.height });
  const duty = await Duty.open(canvas, shape.name, kept());
  // (the flow waits at the title for a guest: one is on its way)
  duty.interface_opened();

  // What a frame's parts cost, in milliseconds summed since the start: the guest's turns, and the redraws
  // of its picture (`redrawMs`: the UI core's drawing, `drawMs` of it, then the upload).
  const timing = { turns: 0, turnMs: 0, redraws: 0, redrawMs: 0, drawMs: 0 };
  // (milliseconds from the page's start: the pack read, the interface up, the first frame, the first of the street)
  const report = { duty, device: () => device.id, shape: () => shape, ready: 0, interfaceReady: 0, firstFrame: 0, firstWorld: 0, frames: 0, failure: "", timing };
  window.pocketDuty = report;

  // The street, read beside everything else.
  const pack = query.get("pack") ?? document.querySelector('meta[name="pocket-pack"]').content;
  duty.reader().read(new URL(pack, location.href).href).then((world) => {
    duty.enter(world);
    if (query.get("words")) duty.control(query.get("words"));
    report.ready = performance.now() - started;
  }).catch((error) => {
    report.failure = message(error);
    duty.fail(report.failure);
    if (!ui) say(report.failure);
  });

  // The PSP on the page: its screen, its controls, and the interface as the PSP loads it, in a realm.
  let ui = null;
  const present = async () => {
    const plan = await (await fetch(beside(`ui/${device.id}/plan.json`))).json();
    const of = screens(plan);
    // The shell with its screen in it and its keys as the controls.
    player.show(device.id, { width: shape.width, height: shape.height, sticks: device.sticks, glyphs: of.glyphs, touch: of.touch === "none" ? null : of.touch, viewport: of.viewport });
    duty.overlay_hide();
    if (query.get("interface") === "off") return duty.interface_closed();
    try {
      // (`simHz`: the turns a second the PSP's own host gives its interface)
      ui = await openInterface({ realm: beside("app-instance.html"), wasm: beside("pocketjs.wasm"), bundle: beside(`ui/${device.id}/duty.js`), pak: beside(`ui/${device.id}/duty.pak`), plan, simHz: shape.turns });
      duty.interface_opened();
      report.interfaceReady ||= performance.now() - started;
    } catch (error) {
      // Without its interface the game is played by the pad alone: START keeps the flow.
      report.failure = message(error);
      duty.interface_closed();
    }
  };
  const presented = present();

  // Sound starts on the first key or pointer: a browser asks for a gesture. The mixer is the simulation
  // crate's, as on the PSP, and makes blocks at its own rate: the page's output is opened at that rate
  // and pulls a block at a time.
  let sound = null;
  const startSound = () => {
    if (sound || query.get("sound") === "off") return;
    try {
      sound = new AudioContext({ sampleRate: sound_rate() });
      const node = sound.createScriptProcessor(512, 0, 2);
      node.onaudioprocess = (event) => {
        const pcm = duty.audio(512);
        const [left, right] = [event.outputBuffer.getChannelData(0), event.outputBuffer.getChannelData(1)];
        for (let i = 0; i < 512; i++) {
          left[i] = pcm[i * 2] / 32768;
          right[i] = pcm[i * 2 + 1] / 32768;
        }
      };
      node.connect(sound.destination);
    } catch {
      // A browser without an output at that rate plays the game without sound.
      sound = null;
    }
  };
  for (const type of ["keydown", "pointerdown"]) addEventListener(type, startSound, { once: true, capture: true });

  // One frame: the match, the guest's turn when it is worth one, its picture when that has changed, the street.
  const frame = (now) => {
    const held = controls.read();
    duty.step(now, held.buttons, held.left[0], held.left[1]);
    // (a turn is offered as often as the PSP offers one, and is that many sixtieths of a second)
    const ticks = ui !== null ? duty.guest_due() : 0;
    const turned = ticks > 0;
    if (turned) {
      const from = performance.now();
      const line = duty.heard();
      if (line) ui.send(line);
      ui.turn(duty.guest_buttons(), held.contacts, ticks);
      for (const said of ui.drain()) duty.say(said);
      const turnedAt = performance.now();
      if (ui.changed()) {
        // One drawing by the UI core, with its alpha, and the upload.
        const picture = ui.picture();
        const drew = performance.now();
        duty.overlay(picture.pixels, picture.width, picture.height);
        timing.redraws++;
        timing.drawMs += drew - turnedAt;
        timing.redrawMs += performance.now() - turnedAt;
      }
      timing.turns++;
      timing.turnMs += turnedAt - from;
    }
    controls.next(turned);
    const settings = duty.prefs_take();
    if (settings) {
      try {
        localStorage.setItem(KEPT, settings);
      } catch {
        // A browser that keeps nothing starts from the defaults next time.
      }
    }
    duty.draw();
    report.frames++;
    report.firstFrame ||= performance.now() - started;
    if (!report.firstWorld && duty.entered()) {
      report.firstWorld = performance.now() - started;
      // (the street is on the screen: the player may read what it kept back)
      player.ready();
    }
  };

  await title;
  canvas.hidden = false;
  stage.fit();
  const loop = frames(() => 60, (now) => {
    try {
      frame(now);
    } catch (error) {
      // A frame the canvas had no texture for is skipped; anything else stops the game and says why.
      report.failure = message(error);
      if (!/Outdated|Lost|Timeout/.test(report.failure)) {
        loop.stop();
        say(report.failure);
      }
    }
  });

  // For a console and for tools/wgpu.ts.
  report.presented = () => presented;
  report.interface = () => ui;
  report.sound = () => sound?.state ?? "none";
  // The frame as the canvas holds it, one pixel of the street to a pixel: a PNG as a data URL.
  report.capture = () => {
    frame(performance.now());
    return canvas.toDataURL("image/png");
  };
  // A held game for a picture (tools/listing.ts): the display's loop stops, `words` go to the game
  // (App::control), `count` frames of a sixtieth of a second are made one after another, and the screen
  // comes back as the last of them left it.
  report.take = (words, count) => {
    loop.stop();
    duty.control(words);
    const from = performance.now();
    for (let i = 0; i < count; i++) frame(from + (i * 1000) / 60);
    return canvas.toDataURL("image/png");
  };
  // Milliseconds a frame costs the processor and the GPU together, over `count` frames made without
  // waiting for the display: the match, the guest's turns and redraws, the street.
  report.burst = async (count = 300) => {
    const gpu = canvas.getContext("webgpu").getConfiguration().device;
    const from = performance.now();
    for (let i = 0; i < count; i++) frame(from + ((i + 1) * 1000) / 60);
    await gpu.queue.onSubmittedWorkDone();
    return (performance.now() - from) / count;
  };
}

start().catch((error) => {
  window.pocketDuty = { failure: message(error) };
  say(window.pocketDuty.failure);
});
