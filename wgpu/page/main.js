// Pocket Duty in a browser tab: the wgpu renderer (../src, built to pkg/) and
// the game's own interface (../../ui, the bundle the PSP loads, compiled by
// tools/ui.ts), which a realm of the page runs on PocketJS's UI core and the
// renderer lays over the street. The page itself is the Pocket3D player of
// PocketJS's browser kernel (vendor/pocketjs/devices/web/pocket-web-wgpu,
// staged beside this file): the bar, a handheld's shell with its keys, the dock
// that leads to the game's files and to a remix of it on Pocket Studio. This
// file says what the game is and draws into the player's canvas.
//
// The Pocket3D title card plays first; the interface and the pack are read
// while it plays.
//
// The address chooses the pack and what a development host would say:
//
//   ?device=psp|3ds|vita        the handheld the page opens as (the bar chooses another at any time).
//                               Without it: the PS Vita on a computer, the PSP on a phone or for a
//                               visitor who asked for less data
//   ?pack=URL                   the first map's pack (the PSP's): its file, on a server that answers
//                               byte ranges, or the manifest of the packs in pieces and the map (`….packs#yards`). Without it,
//                               the page's own (<meta name="pocket-pack">)
//   ?map=yards                  the map the page opens on, of those it has a pack for
//                               (<meta name="pocket-packs">: `<map>=<pack>` with spaces between)
//
// The PSP and the 3DS draw the PSP's pack. A device with packs of its own (the PS Vita:
// <meta name="pocket-packs-vita">, every map's as `<map>=<pack>`) draws those, where the tab's GPU reads
// their pictures' blocks; choosing such a device in the bar reads the held map again from its pack.
//   ?words=mode=play+auto=1     what a development host would send the game (App::control)
//   ?interface=off              the street alone, played by the pad: no guest is started
//   ?sound=off                  no sound
//
// `window.pocketDuty` is the running game, for a console and for tools/wgpu.ts.
import { playTitle } from "./pocket3d-title.js";
import { frames, hasWebGPU, titleCard } from "./pocket3d-shell.js";
import { openInterface, screens } from "./pocket3d-interface.js";
import { createPlayer } from "./pocket3d-player.js";
import init, { Duty, maps, shapes, sound_rate } from "./pkg/duty_wgpu.js";

// The handhelds the game runs on. Each one's screen is the renderer's shape of the same name and its
// interface the bundle under ui/<id>/, as that device's own build compiles it. A 3DS has a second screen,
// which is the interface's alone: the level from above and four keys under a pointer or a finger.
// (tools/platforms.test.ts holds this list, `runsOn` below, the renderer's shapes and the release's
// targets to one another: a device with a package is a device here.)
const DEVICES = [
  { id: "psp", label: "PSP", sticks: 1 },
  { id: "3ds", label: "Nintendo 3DS", sticks: 1 },
  { id: "vita", label: "PS Vita", sticks: 2 },
];
// Where the interface's settings are kept between visits (a PSP keeps them in a file beside the pack).
const KEPT = "pocket-duty.interface";

const query = new URLSearchParams(location.search);
const beside = (name) => new URL(name, import.meta.url).href;
const message = (error) => String(error?.message ?? error);
const started = performance.now();

// The packs a device has of its own, by map (`<meta name="pocket-packs-<device>">`), or none.
function packsOf(id) {
  const own = new Map();
  for (const word of (document.querySelector(`meta[name="pocket-packs-${id}"]`)?.content ?? "").split(/\s+/).filter(Boolean)) {
    const at = word.indexOf("=");
    if (at > 0) own.set(word.slice(0, at), word.slice(at + 1));
  }
  return own;
}

// The page: PocketJS's player, with the device the address asks for. Without one it opens as the
// PS Vita, whose pack is the richest picture, where the page has that pack and the visitor sits at a
// computer; as the PSP on a small screen or under a finger (a phone's GPU reads no blocks, and the
// Vita's packs are three to seven times the PSP's bytes) and for a visitor who asked for less data.
const wanted = query.get("device") ?? query.get("shape");
const roomy = Math.min(screen.width, screen.height) >= 700 && matchMedia("(pointer: fine)").matches && navigator.connection?.saveData !== true;
let device = DEVICES.find((d) => d.id === wanted) ?? ((roomy && packsOf("vita").size > 0 && DEVICES.find((d) => d.id === "vita")) || DEVICES[0]);
let present = () => {};
const player = createPlayer({
  title: "Pocket Duty",
  tagline: { en: "A firefight on a market street.", ja: "市場通りでの銃撃戦。" },
  devices: DEVICES,
  device: device.id,
  // (the targets `bun tools/release.ts` builds a package for)
  runsOn: ["psp", "3ds", "vita"],
  pick: (id) => present(DEVICES.find((d) => d.id === id)),
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
  const all = JSON.parse(shapes());
  let shape = all.find((s) => s.name === device.id);

  // The shell, on the first device's screen. It draws before there is a world: the interface says how much of the
  // pack has arrived.
  stage.show({ device: device.id, width: shape.width, height: shape.height });
  const duty = await Duty.open(canvas, shape.name, kept());
  // (the flow waits at the title for a guest: one is on its way)
  duty.interface_opened();

  // What a frame's parts cost, in milliseconds summed since the start: the guest's turns, and the redraws
  // of its picture (`redrawMs`: the UI core's drawing, `drawMs` of it, then the upload).
  // (`lowers`, `lowerMs`: the second screen's redraws)
  const timing = { turns: 0, turnMs: 0, redraws: 0, redrawMs: 0, drawMs: 0, lowers: 0, lowerMs: 0 };
  // (milliseconds from the page's start: the pack read, the interface up, the first frame, the first of the street)
  const report = { duty, device: () => device.id, shape: () => shape, ready: 0, interfaceReady: 0, firstFrame: 0, firstWorld: 0, frames: 0, failure: "", timing };
  window.pocketDuty = report;

  // The packs by map: the first map's, and those of the other maps the page names. They are the PSP's,
  // which the 3DS draws too.
  const packs = new Map([[JSON.parse(maps())[0].id, query.get("pack") ?? document.querySelector('meta[name="pocket-pack"]').content]]);
  for (const word of (document.querySelector('meta[name="pocket-packs"]')?.content ?? "").split(/\s+/).filter(Boolean)) {
    const at = word.indexOf("=");
    if (at > 0) packs.set(word.slice(0, at), word.slice(at + 1));
  }
  // A device's packs: its own where the page has them and this GPU reads their blocks, the PSP's otherwise.
  const own = new Map(DEVICES.map((d) => [d.id, packsOf(d.id)]));
  const packsFor = (d) => (own.get(d.id).size > 0 && duty.reads_blocks() ? own.get(d.id) : packs);
  let using = packsFor(device);
  duty.maps([...using.keys()].join(","));
  report.packs = () => ({ own: using !== packs, maps: [...using.keys()] });
  // A map's pack, read beside everything else. The shell holds one world: it let the last one go
  // before it asked for this map. A read that another has overtaken (another device's pack was asked
  // for while it was under way) gives its world back.
  let reads = 0;
  const read = (map) => {
    const turn = ++reads;
    return duty.reader().read(new URL(using.get(map), location.href).href).then((world) => {
      if (turn !== reads) return world.free();
      duty.enter(world);
      if (!report.ready && query.get("words")) duty.control(query.get("words"));
      report.ready ||= performance.now() - started;
    }).catch((error) => {
      if (turn !== reads) return;
      report.failure = message(error);
      duty.fail(report.failure);
      if (!ui) say(report.failure);
    });
  };
  const first = using.has(query.get("map")) ? query.get("map") : using.keys().next().value;
  duty.reads(first);
  read(first);

  // A device on the page: its screens, its controls, and its presentation of the interface in a new realm.
  let ui = null;
  let lower = null;
  present = async (next) => {
    device = next;
    const plan = await (await fetch(beside(`ui/${next.id}/plan.json`))).json();
    if (device !== next) return;
    const of = screens(plan);
    const to = all.find((s) => s.name === next.id);
    // The device's shell with its screens in it, its keys as the controls, and the screen that takes touch.
    player.show(next.id, { width: to.width, height: to.height, lower: of.auxiliary, sticks: next.sticks, glyphs: of.glyphs, touch: of.touch === "none" ? null : of.touch, viewport: of.viewport });
    shape = JSON.parse(duty.reshape(to.name, to.width, to.height));
    lower = of.auxiliary ? new ImageData(of.auxiliary[0], of.auxiliary[1]) : null;
    // A device whose packs are others than the held one's: the held map is read again from its own, as
    // another map is (the loading words, then the title or the match it left).
    if (packsFor(next) !== using) {
      using = packsFor(next);
      duty.maps([...using.keys()].join(","));
      duty.repack();
    }
    // The guest of the device before goes with its realm; the new one is told the whole state on its first turn.
    ui?.close();
    ui = null;
    duty.overlay_hide();
    if (query.get("interface") === "off") return duty.interface_closed();
    try {
      // (`simHz`: the turns a second the device's own host gives its interface)
      const opened = await openInterface({ realm: beside("app-instance.html"), wasm: beside("pocketjs.wasm"), bundle: beside(`ui/${next.id}/duty.js`), pak: beside(`ui/${next.id}/duty.pak`), plan, simHz: shape.turns });
      // (another device was chosen while this one's interface was read)
      if (device !== next) return opened.close();
      ui = opened;
      duty.interface_opened();
      report.interfaceReady ||= performance.now() - started;
    } catch (error) {
      // Without its interface the game is played by the pad alone: START keeps the flow.
      report.failure = message(error);
      duty.interface_closed();
    }
  };
  let presented = present(device);
  const chosen = present;
  present = (next) => (presented = chosen(next));

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
    // The interface asked for a match on another map: its pack is read, and the match starts when it is there.
    const map = duty.map_wanted();
    if (map) {
      // (a map the shown device has no pack for gives way to the first it has)
      const has = using.has(map) ? map : using.keys().next().value;
      if (has !== map) duty.reads(has);
      read(has);
    }
    // (a turn is offered as often as the PSP offers one, and is that many sixtieths of a second)
    const ticks = ui !== null ? duty.guest_due(held.touching) : 0;
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
      const drawn = performance.now();
      if (lower && ui.lowerChanged()) {
        // The second screen is the interface's alone: its pixels go to its canvas as they are.
        lower.data.set(ui.lower());
        stage.lower.putImageData(lower, 0, 0);
        timing.lowers++;
        timing.lowerMs += performance.now() - drawn;
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
  // Another device while the game runs: `pocketDuty.present("3ds")`.
  report.present = (id) => present(DEVICES.find((d) => d.id === id));
  // The second screen as its canvas holds it (a PNG as a data URL), or null on a device with one screen.
  report.captureLower = () => (stage.second.hidden ? null : stage.second.toDataURL("image/png"));
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
