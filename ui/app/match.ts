// The match as the interface sees it, on any device: which list is up, what
// its rows do, what is kept between runs (the settings), which lines of the
// kill feed still stand, and when something on the screen is due to leave.
import { createEffect, createMemo, createSignal, on, onCleanup, untrack, type Accessor } from "solid-js";
import { after } from "@pocketjs/framework/clock";
import { glyph, modality } from "@pocketjs/framework/modality";
import type { Host } from "./host.ts";
import type { Kill, Mode, Setting } from "./protocol.ts";
import { MAPS } from "./generated/level.ts";

/** What the settings a renderer may offer are called. One it lists that is not here shows its key. */
const LABELS: Record<string, string> = {
  look: "Look speed",
  invert: "Invert look",
  assist: "Aim assist",
  layout: "Controls",
  skill: "Difficulty",
  stats: "Statistics",
};

/**
 * A row of a list. What can change while the row stands (a switch, a named
 * value) is a function: a list is built once, kept, and its rows are written
 * in place. Building a list takes tenths of a second on the PSP.
 */
export interface Row {
  label: string | (() => string);
  /** A switch's state, or undefined for a row with a named value or none. */
  on?: () => boolean;
  /** The named value of a choice, or what stands at the right of the row. */
  value?: string | (() => string);
  /** A row that only informs takes no press and no focus mark. */
  press?: () => void;
}

/** What a row's member reads now. */
export function read<T extends string | boolean>(member: T | (() => T)): T {
  return typeof member === "function" ? member() : member;
}

/** The list on the screen: the mode's own, or one opened from it. `maps`: the maps a match can be
 *  played on, where the renderer has more than one. */
export type Sheet = "menu" | "settings" | "about" | "maps";

/** Seconds a fade takes to play, and a turn more. */
export const SETTLE = 0.4;

/** Something that stands on the screen for a while after it was last renewed. */
export interface Lease {
  /** It stands from now. */
  renew(): void;
  /** It is taken off by its owner: nothing more is due. */
  end(): void;
}

/**
 * When something on the screen is due to change with no news from the
 * renderer. A turn of the guest costs milliseconds on a PSP, so the renderer
 * takes one only when it must: `idle` tells it nothing is pending here.
 */
export interface Schedule {
  /** For `seconds` the guest takes every turn it is offered: something fades. */
  hold(seconds: number): void;
  /** Runs `run` in `seconds`, counted by the guest's own turns, every one of
   *  which the renderer then has to take: for a flash of under half a second.
   *  Returns how to call it off. */
  soon(seconds: number, run: () => void): () => void;
  /** Runs `run` in `seconds`, counted by the renderer (`wake`): for what
   *  stands longer, with nothing pending here in between. Returns how to call
   *  it off. */
  wake(seconds: number, run: () => void): () => void;
  /**
   * Runs `over` when `seconds` have passed with no `renew()`, on one wake at
   * a time. Renewing while a wake is pending asks for no other: when that
   * wake is due and something renewed since it was asked for, one more is
   * asked. `over` then comes `seconds` after a lone renewal, and between
   * `seconds` and twice that after the last of several. A hit a turn so asks
   * the renderer for a wake every `seconds`, not for one a turn: a renderer
   * keeps a short table of wakes, and one it has no room for is a banner
   * that never leaves.
   */
  lease(seconds: number, over: () => void): Lease;
}

export function createSchedule(host: Host): Schedule {
  let pending = 0, open = false;
  let said: boolean | undefined;
  const say = () => {
    const idle = pending === 0;
    if (!open || idle === said) return;
    said = idle;
    host.send({ type: "idle", on: idle });
  };
  createEffect(() => {
    if (!host.ready()) return;
    open = true;
    say();
  });
  const hold = (seconds: number) => {
    pending++;
    say();
    after(seconds, () => {
      pending--;
      say();
    });
  };

  // Each wake has an id of its own, which the renderer says back when its seconds are over.
  let ids = 0;
  const waking = new Map<number, () => void>();
  createEffect(on(host.woke, (id) => {
    const run = waking.get(id);
    waking.delete(id);
    run?.();
  }, { defer: true }));
  const wake = (seconds: number, run: () => void) => {
    const id = ++ids;
    waking.set(id, run);
    host.send({ type: "wake", id, seconds });
    return () => void waking.delete(id);
  };

  return {
    hold, wake,
    soon(seconds, run) {
      // The turn the timer fires in, and one after it for what it changed.
      hold(seconds + 0.1);
      return after(seconds, run);
    },
    lease(seconds, over) {
      let standing = false, renewed = false;
      let forget = () => {};
      const due = () => {
        if (renewed) {
          renewed = false;
          forget = wake(seconds, due);
          return;
        }
        standing = false;
        over();
      };
      return {
        renew() {
          if (standing) return void (renewed = true);
          standing = true;
          forget = wake(seconds, due);
        },
        end() {
          forget();
          standing = renewed = false;
        },
      };
    },
  };
}

/**
 * True for `seconds` after each `show()`. The seconds are counted by the
 * renderer, not by a timer here: a timer of the guest's counts its turns, so
 * one that is pending makes the device take every turn it offers, 30 a
 * second, for as long as a banner stands. Between the two fades nothing is
 * scheduled. `hide()` ends a showing early.
 */
export function createPulse(schedule: Schedule, seconds: number): [Accessor<boolean>, () => void, () => void] {
  const [shown, setShown] = createSignal(false);
  const fade = (to: boolean) => {
    if (untrack(shown) === to) return;
    setShown(to);
    schedule.hold(SETTLE);
  };
  const lease = schedule.lease(seconds, () => fade(false));
  onCleanup(lease.end);
  return [shown, () => {
    lease.renew();
    fade(true);
  }, () => {
    lease.end();
    fade(false);
  }];
}

/** "4:07" */
export function clock(seconds: number): string {
  const whole = Math.max(0, Math.floor(seconds));
  const s = whole % 60;
  return `${Math.floor(whole / 60)}:${s < 10 ? "0" : ""}${s}`;
}

/** What the game is, where it comes from and what it is built on. */
/** Two screens: the 3DS. */
const DUAL = modality.screens.length > 1;

export const CREDITS = DUAL
  ? ["Pocket Duty is a Nintendo 3DS port of Claude of Duty", "by mshumer (MIT), built on Pocket3D and PocketJS."]
  : ["Pocket Duty is a PSP port of Claude of Duty by mshumer (MIT),", "built on Pocket3D and PocketJS."];

/** What the soldier's buttons do. The device maps them; these lines say what it maps. */
export function controls(): Row[] {
  const face = `${glyph("triangle")} ${glyph("circle")} ${glyph("cross")} ${glyph("square")}`;
  return [
    { label: DUAL ? "Circle Pad" : "Stick", value: "Move" },
    { label: "Full forward", value: "Sprint" },
    { label: face, value: "Look" },
    { label: glyph("rtrigger"), value: "Fire" },
    { label: glyph("ltrigger"), value: "Aim" },
    { label: glyph("start"), value: "Menu" },
    { label: "D-pad up", value: "Jump, climb" },
    { label: "D-pad down", value: "Crouch" },
    { label: "D-pad left", value: "Reload" },
    { label: "D-pad right", value: "Change weapon" },
    { label: glyph("select"), value: "Fire mode" },
  ];
}

/** Seconds a line of the kill feed, a banner and a mark of where a hit came from stand. */
export const FEED_SECONDS = 5;
/** Wakes a line of the feed stands for: see `createMatch`. */
const FEED_STEPS = 2;
export const BANNER_SECONDS = 2;
export const HURT_SECONDS = 1.5;

export interface Match {
  host: Host;
  schedule: Schedule;
  mode: Accessor<Mode>;
  /** True while a list is up and the pad is the interface's. */
  listing: Accessor<boolean>;
  sheet: Accessor<Sheet>;
  open(sheet: Sheet): void;
  /** The rows of the list that is up. */
  rows: Accessor<Row[]>;
  /** One level up: a list opened from another closes; the menu leaves the match alone. */
  back(): void;
  /** Whether `back` does anything here, and what it is called. */
  backLabel: Accessor<string>;
  /** A list comes up over the match. */
  menu(): void;
  /** A match begins on the map the renderer holds. */
  start(): void;
  /** What the strip under a list says at its left. */
  notice: Accessor<string>;
  /** Each mode's own rows: a presentation may build them before the mode is first shown. */
  lists: { title: Row[]; menu: Row[]; results: Row[] };
  /** One row per map the renderer has a pack for: a press asks for a match on it. */
  maps: Accessor<Row[]>;
  /** The map the renderer holds: its name and its line. */
  map: Accessor<{ title: string; line: string }>;
  /** One row per setting the renderer offers. */
  settings: Accessor<Row[]>;
  controls: Row[];
  /** Rises when this interface asks for a match to begin. What stood on the screen for the last
   *  one (a mark, a banner) leaves then: the renderer owes it nothing more. */
  began: Accessor<number>;
  /** The lines of the kill feed that still stand, newest first. */
  feed: Accessor<Kill[]>;
  /** The banner's line and the line under it, and whether it stands. */
  banner: Accessor<[string, string]>;
  bannered: Accessor<boolean>;
}

/** What is kept between runs: the settings the person changed, by key. */
type Prefs = Record<string, number>;

export function createMatch(host: Host): Match {
  const schedule = createSchedule(host);
  const [sheet, setSheet] = createSignal<Sheet>("menu");
  const [prefs, setPrefs] = createSignal<Prefs>({});
  const [began, setBegan] = createSignal(0);
  const mode = host.mode;
  const listing = createMemo(() => mode() === "title" || mode() === "menu" || mode() === "results");
  const playing = () => mode() === "play" || mode() === "down";

  // Each mode opens on its own list.
  createEffect(on(mode, () => setSheet("menu")));

  // What the renderer stored for us last time: the settings, which it is told again once it has
  // listed them. `said` is the text this interface last asked to be kept: hearing it back is no news.
  let said = "";
  createEffect(() => {
    const text = host.prefs();
    if (!host.ready() || !host.options().length || text === said) return;
    said = text;
    const stored: Prefs = {};
    try {
      const kept = JSON.parse(text || "{}") as unknown;
      if (kept && typeof kept === "object") {
        for (const [key, value] of Object.entries(kept)) if (typeof value === "number") stored[key] = value;
      }
    } catch {
      // A damaged file starts over.
    }
    setPrefs(stored);
    for (const setting of untrack(host.options)) {
      const value = stored[setting.key];
      const most = setting.choices ? setting.choices.length - 1 : 1;
      if (Number.isInteger(value) && value >= 0 && value <= most && value !== setting.value) host.send({ type: "option", key: setting.key, value });
    }
  });
  const set = (setting: Setting, value: number) => {
    host.send({ type: "option", key: setting.key, value });
    const next = { ...prefs(), [setting.key]: value };
    setPrefs(next);
    host.send({ type: "prefs", value: (said = JSON.stringify(next)) });
  };
  // A setting's row stands while the renderer offers the setting: its state is read where it is shown.
  const row = (key: string, choice: boolean): Row => {
    const label = LABELS[key] ?? key;
    const now = () => host.options().find((setting) => setting.key === key);
    if (!choice) {
      return { label, on: () => !!now()?.value, press: () => {
        const setting = now();
        if (setting) set(setting, setting.value ? 0 : 1);
      } };
    }
    return { label, value: () => now()?.choices?.[now()!.value] ?? "", press: () => {
      const setting = now();
      if (setting?.choices) set(setting, (setting.value + 1) % setting.choices.length);
    } };
  };
  // Which settings the renderer offers, each with whether it is a choice: the list made of them is
  // built again only when this changes, not when a setting's value does.
  const offered = createMemo(() => host.options().map((setting) => `${setting.key}${setting.choices ? ":" : ""}`).join(" "));
  const settings = createMemo<Row[]>(() => offered().split(" ").filter((key) => key).map((key) => row(key.replace(":", ""), key.endsWith(":"))));

  // Each mode's own list is built once: showing it again changes no row.
  const begin = (type: "start" | "restart") => {
    setBegan((count) => count + 1);
    host.send({ type });
  };
  // The maps the renderer has a pack for, as this interface knows them, in the list's order.
  const offeredMaps = createMemo(() => host.maps().join(" "));
  const play = (id: string) => {
    setBegan((count) => count + 1);
    host.send({ type: "map", id });
  };
  const maps = createMemo<Row[]>(() =>
    MAPS.filter((map) => offeredMaps().split(" ").includes(map.id)).map((map) => ({
      label: map.title,
      // The map behind the list needs no reading.
      value: () => (host.map() === map.id ? "Here" : ""),
      press: () => play(map.id),
    })));
  const map = createMemo(() => MAPS.find((entry) => entry.id === host.map()) ?? MAPS[0]);
  const title: Row[] = [
    // With one map the match starts on it; with more, the first row opens their list.
    { label: "Start match", press: () => (untrack(maps).length > 1 ? setSheet("maps") : begin("start")) },
    { label: "Settings", press: () => setSheet("settings") },
    { label: "About", press: () => setSheet("about") },
  ];
  const leave = () => host.send({ type: "menu", on: false });
  const menu: Row[] = [
    { label: "Resume", press: leave },
    { label: "Settings", press: () => setSheet("settings") },
    { label: "Restart match", press: () => begin("restart") },
    { label: "Quit to title", press: () => host.send({ type: "title" }) },
  ];
  const results: Row[] = [
    { label: "Play again", press: () => begin("restart") },
    { label: "Title", press: () => host.send({ type: "title" }) },
  ];
  const none: Row[] = [];
  // During play no list is up: the rows stay what they were, so nothing is rebuilt under the HUD.
  const rows = createMemo<Row[]>((before) => {
    if (!listing()) return before;
    if (sheet() === "settings") return settings();
    if (sheet() === "maps") return maps();
    if (sheet() === "about") return none;
    return mode() === "title" ? title : mode() === "menu" ? menu : results;
  }, none);

  // A line of the feed stands for five seconds from when it is first seen. While any stands, one
  // wake at a time counts them all, two to a line's five seconds: a line seen between two wakes
  // leaves at the second after it, between two and a half and five seconds on. Each wake is turns
  // of the guest during play, so the count is coarse.
  /** Wakes each standing line has stood, by its id; and the ids whose time is over. */
  const ages = new Map<number, number>();
  const gone = new Set<number>();
  const [left, setLeft] = createSignal(0);
  let known = new Set<number>();
  let counting = false, forget = () => {};
  const count = () => {
    counting = false;
    let over = false;
    for (const [id, age] of ages) {
      if (age + 1 < FEED_STEPS) ages.set(id, age + 1);
      else {
        ages.delete(id);
        gone.add(id);
        over = true;
      }
    }
    if (over) setLeft((n) => n + 1);
    if (ages.size) keep();
  };
  const keep = () => {
    if (counting) return;
    counting = true;
    forget = schedule.wake(FEED_SECONDS / FEED_STEPS, count);
  };
  /** Every line on the screen leaves now. */
  const sweep = () => {
    forget();
    counting = false;
    if (!ages.size) return;
    for (const id of ages.keys()) gone.add(id);
    ages.clear();
    setLeft((n) => n + 1);
  };
  createEffect(on(host.feed, (lines) => {
    const fresh = lines.filter((line) => !known.has(line.id));
    known = new Set(lines.map((line) => line.id));
    // An id the renderer no longer lists is free: a new match counts its lines from the start again.
    for (const id of gone) if (!known.has(id)) gone.delete(id);
    for (const id of ages.keys()) if (!known.has(id)) ages.delete(id);
    for (const line of fresh) {
      gone.delete(line.id);
      ages.set(line.id, 0);
    }
    if (fresh.length) keep();
  }));
  createEffect(on(began, sweep, { defer: true }));
  onCleanup(() => forget());
  const feed = createMemo(() => (left(), host.feed().filter((line) => !gone.has(line.id)).slice(0, 4)));

  // The banner stands for two seconds from each time the renderer changes it to one with an id.
  // The one the renderer already holds when this guest starts is not news. Its words stay while it
  // fades, whether or not the renderer has cleared it by then.
  const [bannered, showBanner, hideBanner] = createPulse(schedule, BANNER_SECONDS);
  const worded = () => host.banner()[0] !== 0 && host.banner()[1] !== "";
  const banner = createMemo<[string, string]>((before) => (worded() ? [host.banner()[1], host.banner()[2]] : before), ["", ""]);
  let primed = false;
  createEffect(() => {
    const news = worded();
    if (!host.ready()) return;
    if (!primed) return void (primed = true);
    if (news && untrack(playing)) showBanner();
  });
  // A banner belongs to the match it was said in.
  createEffect(on(mode, (now) => (now === "title" || now === "results" || now === "loading") && hideBanner(), { defer: true }));
  createEffect(on(began, hideBanner, { defer: true }));

  return {
    host, schedule, mode, listing, sheet, rows, settings, began, feed, banner, bannered, maps, map,
    lists: { title, menu, results },
    controls: controls(),
    open: setSheet,
    back() {
      if (sheet() !== "menu") setSheet("menu");
      else if (mode() === "menu") leave();
    },
    backLabel: () => (sheet() !== "menu" ? "back" : mode() === "menu" ? "resume" : ""),
    menu: () => host.send({ type: "menu", on: true }),
    start: () => begin("start"),
    // What the strip says: the match's kind and where it is played, or what the list that is up is for.
    notice: createMemo<string>((before) => {
      const where = map().title;
      if (mode() === "title") return sheet() === "maps" ? "Choose a map" : `Team Deathmatch · ${where}`;
      return mode() === "menu" ? `Paused · ${where}` : mode() === "results" ? `Match over · ${where}` : before;
    }, ""),
  };
}
