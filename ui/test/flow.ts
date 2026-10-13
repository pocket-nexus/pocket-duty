// Drives one device's bundle through a whole session against the mock
// renderer and checks what it asked for.
//   bun ui/test/flow.ts <psp|vita|3ds>     (after `bun tools/ui.ts <device>`)
import assert from "node:assert/strict";
import { BTN } from "../../vendor/pocketjs/contracts/spec/spec.ts";
import { T, type Command } from "../app/protocol.ts";
import { boot, type Device } from "./harness.ts";

const device = (process.argv[2] ?? "psp") as Device;
const rig = await boot(device);
const mock = rig.mock;
/** The commands since the last call, without the pacing ones (`idle`, `wake`). */
let read = 0;
const asked = (): Command[] => {
  const all = mock.log.slice(read).filter((command) => command.type !== "idle" && command.type !== "wake");
  read = mock.log.length;
  return all;
};
const wakes = () => mock.log.filter((command): command is Extract<Command, { type: "wake" }> => command.type === "wake");
const press = (button: number, frames = 4) => {
  rig.press(button);
  rig.step(frames);
};
/** Chooses the row `index` below the focus of the list that is up. */
const choose = (index: number) => {
  for (let i = 0; i < index; i++) rig.press(BTN.DOWN);
  press(BTN.CIRCLE);
};
const EVERY = [BTN.SELECT, BTN.UP, BTN.RIGHT, BTN.DOWN, BTN.LEFT, BTN.LTRIGGER, BTN.RTRIGGER, BTN.TRIANGLE, BTN.CIRCLE, BTN.CROSS, BTN.SQUARE];

// While the level loads no button means anything, and what was stored comes back: the settings are told again.
mock.state.prefs = JSON.stringify({ invert: 1, look: 9, bogus: "x" });
rig.step(6);
for (const button of [...EVERY, BTN.START]) press(button, 1);
assert.deepEqual(asked(), [{ type: "option", key: "invert", value: 1 }]);
assert.equal(mock.state.options[1].value, 1);
assert.equal(mock.idle, true);

// The title. About is a sheet: it asks for nothing, and the back button closes it.
mock.loaded();
rig.step(10);
choose(2);
press(BTN.CROSS);
assert.deepEqual(asked(), []);
// The focus is back on the row that opened the sheet. One up is Settings: a choice steps to its next value.
press(BTN.UP, 1);
press(BTN.CIRCLE);
press(BTN.CIRCLE);
let kept: Record<string, number | string> = { invert: 1, look: 2 };
assert.deepEqual(asked(), [{ type: "option", key: "look", value: 2 }, { type: "prefs", value: JSON.stringify(kept) }]);
assert.equal(mock.state.options[0].value, 2);
assert.equal(mock.state.prefs, JSON.stringify(kept));
press(BTN.CROSS);
// The title's first row starts the match. Where the screen takes a finger, a tap on the row does.
press(BTN.UP, 1);
if (device === "vita") {
  rig.tap(100, 127);
  rig.step(4);
} else press(BTN.CIRCLE);
assert.deepEqual(asked(), [{ type: "start" }]);
assert.equal(mock.state.mode, "play");

// Play: nothing is scheduled, and START is the one button the interface hears.
rig.step(30);
assert.equal(mock.idle, true);
for (const button of EVERY) press(button, 1);
mock.set({ health: 84, ammo: 22, heading: 135, us: 3, them: 2, clock: 548, spread: 9 });
mock.blips([[700, 820], [520, 760]]);
rig.step(4);
assert.deepEqual(asked(), []);
assert.equal(mock.idle, true);
assert.equal(wakes().length, 0);
// The numbers stand in their cells.
for (const text of ["84", "22", "/ 120", "3", "2", "9:08", "M4A1", "AUTO"]) assert.ok(rig.shows(text), `no "${text}" on the screen`);

// A hit flashes for a fraction of a second, on a timer of the guest's own.
mock.hit(1);
rig.step(2);
assert.equal(mock.idle, false);
rig.step(40);
assert.equal(mock.idle, true);
assert.equal(wakes().length, 0);

// Where a hit came from stands for a second and a half, which the renderer counts. Between the
// fade in and the fade out nothing is scheduled.
mock.hurt(90, 61);
rig.step(2);
assert.deepEqual(wakes(), [{ type: "wake", id: 1, seconds: 1.5 }]);
assert.equal(mock.idle, false);
rig.step(26);
assert.equal(mock.idle, true);
assert.equal(mock.state.woke, 0);
rig.step(64);
assert.equal(mock.state.woke, 1);
assert.equal(mock.idle, false);
rig.step(30);
assert.equal(mock.idle, true);
assert.equal(wakes().length, 1);
// Under fire, a hit every tenth of a second for two seconds renews the mark on one wake at a
// time: one when the first hit lands, one when each comes due with hits behind it.
for (let i = 0; i < 20; i++) {
  mock.hurt(40 + i, 60 - i);
  rig.step(6);
}
rig.step(200);
assert.deepEqual(wakes().map((wake) => wake.seconds), [1.5, 1.5, 1.5, 1.5]);
assert.equal(mock.most, 1);
assert.equal(mock.idle, true);

// A banner stands for two seconds and a line of the feed for five, on two wakes.
let before = wakes().length;
mock.banner("Enemy down", "Headshot +150");
mock.kill("You", "Vanguard", true, 1);
rig.step(2);
assert.deepEqual(wakes().slice(before).map((wake) => wake.seconds).sort(), [2, 2.5]);
for (const text of ["ENEMY DOWN", "HEADSHOT +150", "VANGUARD", "YOU"]) assert.ok(rig.shows(text), `no "${text}" on the screen`);
assert.equal(mock.idle, false);
rig.step(40);
assert.equal(mock.idle, true);
rig.step(250);
assert.ok(rig.shows("VANGUARD"));
rig.step(30);
assert.ok(!rig.shows("VANGUARD"));
assert.equal(mock.idle, true);
assert.equal(wakes().length - before, 1 + 2);
// A banner whose words the renderer took away (a match begins again) is not one to show.
before = wakes().length;
mock.state.banner = [mock.state.banner[0], "", ""];
rig.step(4);
assert.equal(wakes().length, before);
assert.equal(mock.idle, true);
// A banner, a hit and a kill at once: three wakes are pending, and no more.
mock.banner("Enemy down", "+100");
mock.kill("Breacher", "You", false, 2);
mock.hurt(180, 40);
rig.step(400);
assert.equal(wakes().length - before, 1 + 2 + 1);
assert.equal(mock.most, 3);
assert.equal(mock.idle, true);
assert.deepEqual(wakes().map((wake) => wake.id), wakes().map((_, i) => i + 1));
assert.deepEqual(asked(), []);

// The touch screen's keys, where there is one: each asks for one press, and the menu's key opens the menu.
if (device === "3ds") {
  // (the lower screen's column: x 244…316; keys at y 90, 127, 164, 201, each 34 tall)
  for (const y of [107, 144, 181]) {
    rig.tap(280, y);
    rig.step(4);
  }
  assert.deepEqual(asked(), [{ type: "tap", key: "weapon" }, { type: "tap", key: "mode" }, { type: "tap", key: "reload" }]);
  assert.ok(rig.shows("MP7") && rig.shows("SEMI"));
  // A tap on the map asks for nothing.
  rig.tap(120, 120);
  rig.step(4);
  assert.deepEqual(asked(), []);
  rig.tap(280, 218);
  rig.step(8);
  assert.deepEqual(asked(), [{ type: "menu", on: true }]);
  assert.equal(mock.state.mode, "menu");
  // In the menu the keys of a match are gone, and the one that stands resumes it.
  rig.tap(280, 107);
  rig.step(4);
  assert.deepEqual(asked(), []);
  rig.tap(280, 218);
  rig.step(8);
  assert.deepEqual(asked(), [{ type: "menu", on: false }]);
  assert.equal(mock.state.mode, "play");
  rig.step(30);
  assert.equal(mock.idle, true);
}

// Down: the interface hears no button at all.
mock.die(3.4);
rig.step(6);
assert.ok(rig.shows("YOU WERE KILLED") && rig.shows("4"));
for (const button of [...EVERY, BTN.START]) press(button, 1);
assert.deepEqual(asked(), []);
mock.respawn();
rig.step(6);

// START opens the menu. A setting is changed and kept; the back button returns to the menu's rows.
press(BTN.START, 8);
assert.deepEqual(asked(), [{ type: "menu", on: true }]);
assert.equal(mock.state.mode, "menu");
choose(1);
choose(5);
kept = { ...kept, stats: 1 };
assert.deepEqual(asked(), [{ type: "option", key: "stats", value: 1 }, { type: "prefs", value: JSON.stringify(kept) }]);
assert.ok(mock.state.stats.length > 0);
press(BTN.CROSS, 6);
// Resume is one row up from Settings.
press(BTN.UP, 1);
press(BTN.CIRCLE, 6);
assert.deepEqual(asked(), [{ type: "menu", on: false }]);
assert.equal(mock.state.mode, "play");

// START closes the menu it opened, and so does the back button.
press(BTN.START, 8);
press(BTN.START, 8);
press(BTN.START, 8);
press(BTN.CROSS, 8);
assert.deepEqual(asked(), [{ type: "menu", on: true }, { type: "menu", on: false }, { type: "menu", on: true }, { type: "menu", on: false }]);
assert.equal(mock.state.mode, "play");

// Restart, from the menu: the same ground with nothing scored.
mock.set({ us: 31, them: 28 });
press(BTN.START, 8);
choose(2);
assert.deepEqual(asked(), [{ type: "menu", on: true }, { type: "restart" }]);
assert.equal(mock.state.mode, "play");
assert.equal(mock.state.t[T.us], 0);

// The match ends. Play again starts another; from the next results the title is one row down.
mock.end({ won: true, us: 75, them: 62, kills: 18, deaths: 9, headshots: 6, accuracy: 37, seconds: 512 });
rig.step(10);
for (const text of ["VICTORY", "75", "62", "18", "9", "6", "37%", "8:32"]) assert.ok(rig.shows(text), `no "${text}" on the screen`);
for (const button of [BTN.START, BTN.CROSS, BTN.SELECT]) press(button, 1);
assert.deepEqual(asked(), []);
choose(0);
assert.deepEqual(asked(), [{ type: "restart" }]);
assert.equal(mock.state.mode, "play");
mock.end({ won: false, us: 58, them: 75, kills: 11, deaths: 14, headshots: 2, accuracy: 24, seconds: 600 });
rig.step(10);
choose(1);
assert.deepEqual(asked(), [{ type: "title" }]);
assert.equal(mock.state.mode, "title");
rig.step(30);
assert.equal(mock.idle, true);

// Every character the session put on the screen is in the bundle's fonts.
assert.deepEqual(rig.missing(), []);
const paced = mock.log.filter((command) => command.type === "idle").length;
console.log(`${device}: the interface asked for what the flow expects (${mock.log.length - paced - wakes().length} commands, ${wakes().length} wakes, ${paced} changes of idle)`);
