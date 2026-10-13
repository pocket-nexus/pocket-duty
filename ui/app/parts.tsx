// The pieces every presentation is built from. A device chooses where they
// go; what the crosshair, the health, a list row or the map looks like is
// decided here once.
//
// The numbers during play (`t`) change up to 30 times a second. Each is
// written straight to its node (`@pocketjs/framework/hot`): a text in a cell
// of fixed size with the text at its left, a transform or an opacity. A new
// value then costs one native call and no layout. Everything else is a signal.
import { createEffect, createMemo, createSignal, For, on, onCleanup, onMount, Show, untrack, type JSX } from "solid-js";
import { useActions, type ActionsHandle } from "@pocketjs/framework/actions";
import { Image, Text, View } from "@pocketjs/framework/components";
import type { SurfaceId } from "@pocketjs/framework/display";
import { createGesture } from "@pocketjs/framework/gesture";
import { prop as hotProp, text as hotText } from "@pocketjs/framework/hot";
import { getOps } from "@pocketjs/framework/host";
import { BTN } from "@pocketjs/framework/input";
import { onFrame } from "@pocketjs/framework/lifecycle";
import { modality } from "@pocketjs/framework/modality";
import type { NodeMirror } from "@pocketjs/framework/renderer";
import type { Host } from "./host.ts";
import { clock, CREDITS, HURT_SECONDS, read, SETTLE, type Match, type Row } from "./match.ts";
import { BLIPS, FLAG, T } from "./protocol.ts";
import { abgr, AMBER, DIM, ENEMY, FAINT, FRIEND, GLASS, HAIRLINE, INK, NIGHT, OK, OUTLINE, PANEL, PLATE, RED, WASH, WELL } from "./theme.ts";

type Node = NodeMirror | undefined;

/**
 * Calls `listener` with the numbers now and in every turn new ones arrive,
 * for the life of the part. A listener runs up to 30 times a second on a
 * 333 MHz interpreter: it compares what it shows with what arrived before
 * it computes or writes anything, and it allocates nothing (no closure, no
 * array, no string) unless a value changed. A collection of the guest's heap
 * stops a PSP for tens of milliseconds.
 */
function watch(host: Host, listener: (t: number[]) => void) {
  onMount(() => onCleanup(host.onNumbers(listener)));
}

const clamp = (value: number, least: number, most: number) => Math.max(least, Math.min(most, value));

/** White, amber and red as a colour prop carries them: a reading that is fine, low, and out. */
const TONES = [abgr(INK), abgr(AMBER), abgr(RED)];

/**
 * A view a finger can tap, on the surface it is drawn on. It stays out of
 * the focus order: a pad has its own button for the same verb. While a
 * finger is on it the view dims, or with `onHeld` its owner is told and
 * draws the press itself.
 */
export function Touchable(props: { surface?: SurfaceId; onTap?: () => void; onHeld?: (held: boolean) => void; class?: string; style?: Record<string, number | string>; children?: JSX.Element }) {
  let node: Node;
  const [down, setDown] = createSignal(false);
  const hold = (held: boolean) => (props.onHeld ? props.onHeld(held) : setDown(held));
  createGesture({
    surface: props.surface,
    region: { node: () => node },
    onDown: () => hold(!!props.onTap),
    onUp: () => hold(false),
    onCancel: () => hold(false),
    onTap: () => props.onTap?.(),
  });
  return <View ref={node} class={props.class} style={{ ...props.style, opacity: down() ? 0.55 : 1 }}>{props.children}</View>;
}

/** The name, as large as the surface has room for. */
export function Wordmark(props: { large?: boolean }) {
  return (
    <View class="flex-col">
      <Text class="text-sm font-bold tracking-wide" style={{ textColor: AMBER }}>POCKET</Text>
      <Show when={props.large} fallback={<Text class="text-2xl font-bold" style={{ textColor: INK }}>DUTY</Text>}>
        <Text class="text-4xl font-bold" style={{ textColor: INK }}>DUTY</Text>
      </Show>
    </View>
  );
}

// ---------------------------------------------------------------------------
// The middle of the screen: where the rifle points, what it hit, where a hit came from
// ---------------------------------------------------------------------------

/**
 * Four blades and a dot around the point of aim. The part is a point: its
 * owner puts it at the centre of the view. The blades stand `t[T.spread]`
 * pixels from the centre; 0 (the sights are up) hides all of it.
 */
export function Crosshair(props: { host: Host }) {
  let root: Node, up: Node, down: Node, left: Node, right: Node;
  let shown = -1;
  watch(props.host, (t) => {
    const spread = Math.round(t[T.spread]) || 0;
    if (spread === shown) return;
    shown = spread;
    hotProp(root, "opacity", spread > 0 ? 1 : 0);
    if (spread <= 0) return;
    hotProp(up, "translateY", -spread);
    hotProp(down, "translateY", spread);
    hotProp(left, "translateX", -spread);
    hotProp(right, "translateX", spread);
  });
  // A blade is 2 × 6 white inside a dark line one pixel wide.
  const blade = { bgColor: "#ffffff", borderWidth: 1, borderColor: OUTLINE };
  return (
    <View ref={root} class="absolute" style={{ insetL: 0, insetT: 0, width: 0, height: 0, opacity: 0 }}>
      <View ref={up} class="absolute" style={{ insetL: -2, insetT: -7, width: 4, height: 8, ...blade }} />
      <View ref={down} class="absolute" style={{ insetL: -2, insetT: -1, width: 4, height: 8, ...blade }} />
      <View ref={left} class="absolute" style={{ insetL: -7, insetT: -2, width: 8, height: 4, ...blade }} />
      <View ref={right} class="absolute" style={{ insetL: -1, insetT: -2, width: 8, height: 4, ...blade }} />
      <View class="absolute" style={{ insetL: -2, insetT: -2, width: 4, height: 4, ...blade }} />
    </View>
  );
}

/** Seconds a hit's mark stands, and its colour: a body, a head, a kill. */
const HITS: [number, string][] = [[0.26, "#ffffff"], [0.32, AMBER], [0.42, RED]];
/** Where a stroke of the mark begins and ends, pixels from the centre along each axis. */
const STROKE = [5, 6, 7, 8, 9, 10];

/**
 * A diagonal cross of four strokes that flashes when a shot lands. No node
 * turns here: a stroke is squares stepped along its diagonal. Each kind has
 * its own strokes, built once, so a flash writes one opacity.
 */
export function Hitmarker(props: { match: Match }) {
  const host = props.match.host;
  const marks = HITS.map(() => ({ node: undefined as Node }));
  let last = 0, lit = -1, forget = () => {};
  const clear = () => {
    forget();
    if (lit >= 0) hotProp(marks[lit].node, "opacity", 0);
    lit = -1;
  };
  watch(host, (t) => {
    const hit = t[T.hit] || 0, before = last;
    last = hit;
    // The number changes with every hit. The first number to arrive says where the match stands,
    // and 0 says a match began: neither is a hit.
    if (!host.live || hit === before || hit === 0) return;
    const kind = Math.min(HITS.length - 1, hit % 4);
    forget();
    if (kind !== lit) {
      if (lit >= 0) hotProp(marks[lit].node, "opacity", 0);
      hotProp(marks[(lit = kind)].node, "opacity", 1);
    }
    forget = props.match.schedule.soon(HITS[kind][0], clear);
  });
  // The match is over, or left, or begins again: what flashed in it is gone.
  createEffect(on(host.mode, (mode) => mode !== "play" && clear(), { defer: true }));
  createEffect(on(props.match.began, clear, { defer: true }));
  return (
    <View class="absolute" style={{ insetL: 0, insetT: 0, width: 0, height: 0 }}>
      <For each={marks}>
        {(mark, kind) => (
          <View ref={mark.node} class="absolute" style={{ insetL: 0, insetT: 0, width: 0, height: 0, opacity: 0 }}>
            <For each={[[-1, -1], [1, -1], [-1, 1], [1, 1]]}>
              {([x, y]) => (
                <For each={STROKE}>
                  {(d) => <View class="absolute" style={{ insetL: x * d - 1, insetT: y * d - 1, width: 2, height: 2, bgColor: HITS[kind()][1] }} />}
                </For>
              )}
            </For>
          </View>
        )}
      </For>
    </View>
  );
}

/** How far from the centre a mark of where a hit came from stands, and how long and thick it is. */
const HURT_RADIUS = 42;
const HURT_LENGTH = 24;

/**
 * Where the last hit came from: one of eight marks around the centre, the
 * one nearest to the hit's bearing, for a second and a half. Straight ahead
 * is at the top. A mark on a diagonal is squares stepped along it. Under
 * fire the mark is renewed with every hit, on one wake at a time.
 */
export function Hurt(props: { match: Match }) {
  const host = props.match.host, schedule = props.match.schedule;
  const marks = Array.from({ length: 8 }, (_, place) => ({ place, node: undefined as Node }));
  let last = 0, lit = -1;
  const hide = () => {
    if (lit < 0) return;
    hotProp(marks[lit].node, "opacity", 0);
    lit = -1;
    schedule.hold(SETTLE);
  };
  const lease = schedule.lease(HURT_SECONDS, hide);
  const clear = () => {
    lease.end();
    hide();
  };
  onCleanup(lease.end);
  watch(host, (t) => {
    const hurt = t[T.hurt] || 0, before = last;
    last = hurt;
    if (!host.live || hurt === before || hurt === 0) return;
    const place = Math.round((hurt % 360) / 45) % 8;
    if (place !== lit) {
      if (lit >= 0) hotProp(marks[lit].node, "opacity", 0);
      hotProp(marks[(lit = place)].node, "opacity", 1);
      schedule.hold(SETTLE);
    }
    lease.renew();
  });
  createEffect(on(host.mode, (mode) => (mode === "title" || mode === "results" || mode === "loading") && clear(), { defer: true }));
  createEffect(on(props.match.began, clear, { defer: true }));
  const half = HURT_LENGTH / 2, slant = Math.round(HURT_RADIUS * Math.SQRT1_2);
  /** The left or top edge of a mark three pixels thick whose middle is `distance` from the centre on the side `sign`. */
  const edge = (sign: number, distance: number) => sign * (distance + 0.5) - 1.5;
  /** The squares of a mark on a diagonal: at the corner `(x, y)` of the ring, across the line from the centre. */
  const steps = Array.from({ length: 9 }, (_, i) => (i - 4) * 2);
  return (
    <View class="absolute" style={{ insetL: 0, insetT: 0, width: 0, height: 0 }}>
      <For each={marks}>
        {(mark) => {
          const place = mark.place;
          const x = [0, 1, 1, 1, 0, -1, -1, -1][place], y = [-1, -1, 0, 1, 1, 1, 0, -1][place];
          return (
            <View ref={mark.node} class="absolute transition-opacity duration-150" style={{ insetL: 0, insetT: 0, width: 0, height: 0, opacity: 0 }}>
              <Show
                when={x !== 0 && y !== 0}
                fallback={
                  x === 0
                    ? <View class="absolute" style={{ insetL: -half, insetT: edge(y, HURT_RADIUS), width: HURT_LENGTH, height: 3, bgColor: RED }} />
                    : <View class="absolute" style={{ insetL: edge(x, HURT_RADIUS), insetT: -half, width: 3, height: HURT_LENGTH, bgColor: RED }} />
                }
              >
                <For each={steps}>
                  {(step) => <View class="absolute" style={{ insetL: edge(x, slant) + step, insetT: edge(y, slant) - step * x * y, width: 3, height: 3, bgColor: RED }} />}
                </For>
              </Show>
            </View>
          );
        }}
      </For>
    </View>
  );
}

/**
 * Low health: red along the four edges of the view, stronger as health
 * falls. Strips, not a layer over the scene: a GE blends every pixel a
 * layer covers. Their red lies between the theme's red and its blood; a
 * class is a literal the compiler reads, so the colour is written out.
 */
export function Edges(props: { host: Host; width: number; height: number; depth?: number }) {
  let root: Node;
  let seen = -1, shown = -1;
  watch(props.host, (t) => {
    const health = t[T.health];
    if (health === seen) return;
    seen = health;
    // Twenty steps: a point of health lost is not always a write.
    const level = health > 0 && health < 36 ? Math.round((0.3 + (0.7 * (36 - health)) / 36) * 20) / 20 : 0;
    if (level !== shown) hotProp(root, "opacity", (shown = level));
  });
  const depth = () => props.depth ?? 18;
  return (
    <View ref={root} class="absolute" style={{ insetL: 0, insetT: 0, width: props.width, height: props.height, opacity: 0, hitPass: 1 }}>
      <View class="absolute bg-gradient-to-b from-[#c4140ed0] to-[#8d0f0a00]" style={{ insetL: 0, insetT: 0, width: props.width, height: depth() }} />
      <View class="absolute bg-gradient-to-t from-[#c4140ed0] to-[#8d0f0a00]" style={{ insetL: 0, insetB: 0, width: props.width, height: depth() }} />
      <View class="absolute bg-gradient-to-r from-[#c4140ed0] to-[#8d0f0a00]" style={{ insetL: 0, insetT: 0, width: depth(), height: props.height }} />
      <View class="absolute bg-gradient-to-l from-[#c4140ed0] to-[#8d0f0a00]" style={{ insetR: 0, insetT: 0, width: depth(), height: props.height }} />
    </View>
  );
}

/** Pressed when the magazine is empty: the d-pad with its left arm lit, and the word. A point, like the crosshair. */
export function ReloadPrompt(props: { host: Host }) {
  let root: Node;
  let seen = -1;
  watch(props.host, (t) => {
    const flags = t[T.flags] || 0;
    if (flags === seen) return;
    seen = flags;
    hotProp(root, "opacity", flags & FLAG.empty && !(flags & FLAG.reloading) ? 1 : 0);
  });
  const width = 86;
  return (
    <View ref={root} class="absolute flex-row items-center" style={{ insetL: -width / 2, insetT: 0, width, height: 19, opacity: 0, bgColor: PLATE, borderWidth: 1, borderColor: HAIRLINE }}>
      <View class="relative" style={{ width: 13, height: 13, marginL: 7 }}>
        <View class="absolute" style={{ insetL: 4, insetT: 0, width: 5, height: 13, bgColor: FAINT }} />
        <View class="absolute" style={{ insetL: 0, insetT: 4, width: 13, height: 5, bgColor: FAINT }} />
        <View class="absolute" style={{ insetL: 0, insetT: 4, width: 5, height: 5, bgColor: AMBER }} />
      </View>
      <Text class="text-xs font-bold tracking-wide" style={{ marginL: 7, textColor: AMBER }}>RELOAD</Text>
    </View>
  );
}

// ---------------------------------------------------------------------------
// The edges of the screen
// ---------------------------------------------------------------------------

/**
 * Health: the number and five segments of twenty points each. Amber from 55
 * down, red from 28 down. The digits stand at the left of a cell of fixed
 * size: a number written to its node keeps the place layout gave the last
 * one, so text that is centred or set to the right would drift.
 */
export function Vitals(props: { host: Host }) {
  let digits: Node;
  const segments = Array.from({ length: 5 }, (_, index) => ({ index, fill: undefined as Node }));
  let seen = NaN, shown = -1, tone = -1;
  watch(props.host, (t) => {
    if (t[T.health] === seen) return;
    seen = t[T.health];
    const health = clamp(Math.round(seen) || 0, 0, 100);
    if (health === shown) return;
    shown = health;
    hotText(digits, health);
    for (const segment of segments) hotProp(segment.fill, "scaleX", clamp((health - segment.index * 20) / 20, 0, 1));
    const now = health <= 28 ? 2 : health <= 55 ? 1 : 0;
    if (now === tone) return;
    tone = now;
    hotProp(digits, "textColor", TONES[now]);
    for (const segment of segments) hotProp(segment.fill, "bgColor", TONES[now]);
  });
  return (
    <View class="relative" style={{ width: 118, height: 44, bgColor: PLATE }}>
      <Text class="absolute text-xs font-bold tracking-wide" style={{ insetL: 8, insetT: 3, textColor: DIM }}>HEALTH</Text>
      <Text ref={digits} class="absolute text-2xl font-bold" style={{ insetL: 8, insetT: 14, width: 48, height: 29, textColor: INK }}>100</Text>
      <For each={segments}>
        {(segment) => (
          <View class="absolute" style={{ insetL: 58 + segment.index * 11, insetT: 29, width: 9, height: 8, bgColor: WELL }}>
            <View ref={segment.fill} class="absolute origin-left" style={{ insetL: 0, insetT: 0, width: 9, height: 8, bgColor: INK }} />
          </View>
        )}
      </For>
    </View>
  );
}

/** Pixels a digit of the magazine's count takes. */
const DIGIT = 16;

/**
 * The weapon: its fire mode and name, the rounds in the magazine large and
 * those in reserve small. The count is amber from 8 down and red at none.
 * While a reload runs, the word and a bar of how far it is stand above.
 */
export function Ammo(props: { host: Host }) {
  const host = props.host;
  let count: Node, reserve: Node, reloading: Node, bar: Node;
  let ammo = NaN, kept = NaN, tone = -1, running = false, done = NaN;
  watch(host, (t) => {
    if (t[T.ammo] !== ammo) {
      ammo = t[T.ammo];
      const now = Math.max(0, Math.round(ammo) || 0);
      hotText(count, now);
      // Set to the right by hand: a number written to its node keeps the left edge layout gave it.
      hotProp(count, "translateX", (2 - (now < 10 ? 1 : now < 100 ? 2 : 3)) * DIGIT);
      const next = now === 0 ? 2 : now <= 8 ? 1 : 0;
      if (next !== tone) hotProp(count, "textColor", TONES[(tone = next)]);
    }
    if (t[T.reserve] !== kept) {
      kept = t[T.reserve];
      hotText(reserve, `/ ${Math.max(0, Math.round(kept) || 0)}`);
    }
    const now = ((t[T.flags] || 0) & FLAG.reloading) !== 0;
    if (now !== running) hotProp(reloading, "opacity", (running = now) ? 1 : 0);
    if (running && t[T.reload] !== done) hotProp(bar, "scaleX", clamp(((done = t[T.reload]) || 0) / 100, 0, 1));
  });
  return (
    <View class="relative" style={{ width: 118, height: 44 }}>
      <View ref={reloading} class="absolute" style={{ insetL: 0, insetT: -20, width: 118, height: 18, opacity: 0, bgColor: PLATE }}>
        <Text class="absolute text-xs font-bold tracking-wide" style={{ insetL: 8, insetT: 0, textColor: AMBER }}>RELOADING</Text>
        <View class="absolute" style={{ insetL: 0, insetB: 0, width: 118, height: 2, bgColor: WELL }} />
        <View ref={bar} class="absolute origin-left" style={{ insetL: 0, insetB: 0, width: 118, height: 2, scaleX: 0, bgColor: AMBER }} />
      </View>
      <View class="absolute" style={{ insetL: 0, insetT: 0, width: 118, height: 44, bgColor: PLATE }} />
      <View class="absolute flex-row items-center" style={{ insetR: 8, insetT: 3, height: 15, gap: 6 }}>
        <View class="flex-row items-center" style={{ height: 15, paddingL: 4, paddingR: 4, borderWidth: 1, borderColor: HAIRLINE, display: host.fire() ? 0 : 1 }}>
          <Text class="text-xs font-bold" style={{ textColor: DIM }}>{host.fire().toUpperCase()}</Text>
        </View>
        <Text class="text-xs font-bold tracking-wide" style={{ textColor: INK }}>{host.weapon().toUpperCase()}</Text>
      </View>
      <Text ref={count} class="absolute text-2xl font-bold" style={{ insetL: 40, insetT: 14, width: 34, height: 29, textColor: INK }}>0</Text>
      <Text ref={reserve} class="absolute text-sm" style={{ insetL: 76, insetT: 23, width: 40, height: 18, textColor: DIM }}>/ 0</Text>
    </View>
  );
}

/** Pixels of tape per degree of heading. */
const TAPE = 1;
const POINTS = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
/** Steps a mark of the tape fades through at an end of the window. */
const FADE = 4;

/**
 * The heading as a tape: the points of the compass slide under a caret at
 * the middle. The tape is one node that moves; a new heading lays nothing
 * out. No clip cuts the tape at the window's ends (a clip is a scissor on a
 * GE and a pass over the whole screen's stencil on the Vita): a mark that
 * reaches an end fades out, which is one more write to its node.
 */
export function Compass(props: { host: Host; width: number }) {
  let tape: Node;
  let shown = -1;
  // The points from a quarter turn before north to a quarter past the next north: the window never
  // looks past either end. Between two points stands a tick.
  const marks = Array.from({ length: 13 }, (_, i) => ({ at: (i - 2) * 45, label: undefined as Node, tick: undefined as Node, steps: [-1, -1] }));
  /** How much of a mark `offset` degrees from the heading shows, in steps of the fade. */
  const step = (offset: number) => clamp(Math.round((props.width / 2 - 6 - Math.abs(offset) * TAPE) / 4), 0, FADE);
  /** Degrees to either side of the heading within which a mark can show. */
  const reach = () => props.width / 2 / TAPE + 24;
  watch(props.host, (t) => {
    const heading = Math.round(t[T.heading]) || 0;
    if (heading === shown) return;
    // Before the first heading every mark is unknown.
    const low = shown < 0 ? -360 : Math.min(shown, heading), high = shown < 0 ? 720 : Math.max(shown, heading);
    shown = heading;
    hotProp(tape, "translateX", Math.round(props.width / 2 - heading * TAPE));
    // The marks that showed at the last heading or show at this one: four or five of the thirteen,
    // unless the heading passed north and came round the tape.
    const from = Math.max(0, Math.floor((low - reach()) / 45) + 2);
    const to = Math.min(marks.length - 1, Math.ceil((high + reach()) / 45) + 2);
    for (let i = from; i <= to; i++) {
      const mark = marks[i];
      const label = step(mark.at - heading), tick = step(mark.at + 22 - heading);
      if (label !== mark.steps[0]) hotProp(mark.label, "opacity", (mark.steps[0] = label) / FADE);
      if (tick !== mark.steps[1]) hotProp(mark.tick, "opacity", (mark.steps[1] = tick) / FADE);
    }
  });
  const middle = () => Math.round(props.width / 2);
  return (
    <View class="relative" style={{ width: props.width, height: 20, bgColor: PLATE }}>
      <View ref={tape} class="absolute" style={{ insetL: 0, insetT: 0, width: 1, height: 20, translateX: middle() }}>
        <For each={marks}>
          {(mark) => (
            <>
              <Text ref={mark.label} class="absolute text-xs font-bold text-center" style={{ insetL: mark.at * TAPE - 12, insetT: 5, width: 24, opacity: step(mark.at) / FADE, textColor: mark.at % 90 === 0 ? INK : DIM }}>{POINTS[(((mark.at / 45) % 8) + 8) % 8]}</Text>
              <View ref={mark.tick} class="absolute" style={{ insetL: mark.at * TAPE + 22, insetT: 10, width: 1, height: 5, opacity: step(mark.at + 22) / FADE, bgColor: FAINT }} />
            </>
          )}
        </For>
      </View>
      {/* The caret: a wedge of four lines, its point down on the tape. */}
      <For each={[0, 1, 2, 3]}>
        {(row) => <View class="absolute" style={{ insetL: middle() - 3 + row, insetT: row, width: 7 - row * 2, height: 1, bgColor: AMBER }} />}
      </For>
    </View>
  );
}

/** The score and the clock: the player's side, the game and what the match has left, the other side. */
export function MatchBar(props: { host: Host; width: number }) {
  let us: Node, them: Node, time: Node;
  let ours = NaN, theirs = NaN, left = NaN;
  watch(props.host, (t) => {
    if (t[T.us] !== ours) hotText(us, Math.round((ours = t[T.us])) || 0);
    if (t[T.them] !== theirs) {
      const score = Math.round((theirs = t[T.them])) || 0;
      hotText(them, score);
      // The other side's score ends at the bar's right edge, whatever its digits.
      hotProp(them, "translateX", (2 - (score < 10 ? 1 : score < 100 ? 2 : 3)) * 9);
    }
    if (t[T.clock] !== left) hotText(time, clock((left = t[T.clock]) || 0));
  });
  const middle = () => Math.round((props.width - 72) / 2);
  return (
    <View class="relative" style={{ width: props.width, height: 18, bgColor: PLATE }}>
      <View class="absolute" style={{ insetL: 0, insetT: 0, width: 2, height: 18, bgColor: FRIEND }} />
      <View class="absolute" style={{ insetR: 0, insetT: 0, width: 2, height: 18, bgColor: ENEMY }} />
      <Text ref={us} class="absolute text-sm font-bold" style={{ insetL: 7, insetT: 0, width: 22, height: 18, textColor: FRIEND }}>0</Text>
      <Text class="absolute text-xs font-bold tracking-wide" style={{ insetL: middle(), insetT: 2, textColor: DIM }}>TDM</Text>
      <Text ref={time} class="absolute text-sm font-bold" style={{ insetL: middle() + 34, insetT: 0, width: 40, height: 18, textColor: INK }}>0:00</Text>
      <Text ref={them} class="absolute text-sm font-bold" style={{ insetL: props.width - 25, insetT: 0, width: 20, height: 18, translateX: 9, textColor: ENEMY }}>0</Text>
    </View>
  );
}

/**
 * The match in a column, for a screen of its own beside the map: what the
 * clock has left, and the player's kills and deaths (the player is one side
 * and the soldiers the other, so the two scores are those).
 */
export function Board(props: { host: Host; width: number }) {
  let time: Node, us: Node, them: Node;
  let left = NaN, ours = NaN, theirs = NaN;
  watch(props.host, (t) => {
    if (t[T.clock] !== left) hotText(time, clock((left = t[T.clock]) || 0));
    if (t[T.us] !== ours) hotText(us, Math.round((ours = t[T.us])) || 0);
    if (t[T.them] !== theirs) hotText(them, Math.round((theirs = t[T.them])) || 0);
  });
  return (
    <View class="relative" style={{ width: props.width, height: 84, bgColor: PLATE }}>
      <Text class="absolute text-xs font-bold tracking-wide" style={{ insetL: 8, insetT: 4, textColor: DIM }}>TDM</Text>
      <Text ref={time} class="absolute text-xl font-bold" style={{ insetL: 8, insetT: 16, width: props.width - 10, height: 24, textColor: INK }}>0:00</Text>
      <View class="absolute" style={{ insetL: 0, insetT: 42, width: props.width, height: 1, bgColor: HAIRLINE }} />
      <Text class="absolute text-xs font-bold" style={{ insetL: 8, insetT: 47, textColor: DIM }}>KILLS</Text>
      <Text ref={us} class="absolute text-sm font-bold" style={{ insetL: props.width - 20, insetT: 45, width: 19, height: 18, textColor: FRIEND }}>0</Text>
      <Text class="absolute text-xs font-bold" style={{ insetL: 8, insetT: 66, textColor: DIM }}>DEATHS</Text>
      <Text ref={them} class="absolute text-sm font-bold" style={{ insetL: props.width - 20, insetT: 64, width: 19, height: 18, textColor: ENEMY }}>0</Text>
    </View>
  );
}

/**
 * The level from above, north up: one picture that does not move, the
 * player as a dot with a tick toward where they face, and the enemies the
 * renderer lists. Positions arrive in decimetres from the picture's top left
 * corner. Nothing turns: the tick has sixteen places around the dot.
 *
 * The picture is the held map's own: the renderer reads it beside the map's
 * pack and says where it put it (`chart` of the state). The interface holds no
 * map's picture itself, so a map that arrives later brings its own.
 */
export function LevelMap(props: { host: Host; size: number }) {
  let me: Node, tick: Node, picture: Node;
  const blips = Array.from({ length: BLIPS }, (_, index) => ({ index, node: undefined as Node }));
  // What arrived last, as it arrived: a turn in which the player and the enemies stood still writes nothing.
  let x = NaN, z = NaN, heading = NaN, faced = -1, count = -1;
  const seen = new Array<number>(BLIPS * 2).fill(NaN);
  // The held map's picture: its side in pixels, the map's side in it, the metres a pixel covers. Before
  // the renderer has said, and for a map without a picture, the dots keep the scale every map so far has.
  const [chart, setChart] = createSignal({ texture: 256, side: 256, metres: 0.625, shown: false });
  let held = chart();
  // A texture this interface uploaded itself (in a browser tab), to free when another map's comes.
  let own = -1;
  createEffect(on(props.host.chart, (told) => {
    const ops = getOps();
    if (own >= 0) ops.freeTexture?.(own);
    own = -1;
    let texture = told ? told[0] : -1;
    if (told && texture < 0 && told[4] && ops.uploadImgEntry) texture = own = ops.uploadImgEntry(Uint8Array.from(atob(told[4]), (c) => c.charCodeAt(0)));
    if (texture >= 0 && picture) ops.setImage(picture.id, texture);
    held = told ? { texture: told[1], side: told[2], metres: told[3], shown: texture >= 0 } : { ...held, shown: false };
    setChart(held);
    // What was drawn was drawn at the other map's scale.
    x = z = NaN;
    seen.fill(NaN);
  }));
  /** A pixel of the map as drawn, from decimetres. */
  const at = (decimetres: number) => Math.round(clamp(((decimetres || 0) * props.size) / (held.side * held.metres * 10), 0, props.size));
  watch(props.host, (t) => {
    if (t[T.x] !== x) hotProp(me, "translateX", at((x = t[T.x])));
    if (t[T.z] !== z) hotProp(me, "translateY", at((z = t[T.z])));
    if (t[T.heading] !== heading) {
      heading = t[T.heading];
      const sector = Math.round((heading || 0) / 22.5) % 16;
      if (sector !== faced) {
        faced = sector;
        const angle = (sector * Math.PI) / 8;
        hotProp(tick, "translateX", Math.round(Math.sin(angle) * 5));
        hotProp(tick, "translateY", -Math.round(Math.cos(angle) * 5));
      }
    }
    const now = clamp(t[T.blips] || 0, 0, BLIPS);
    for (let i = 0; i < BLIPS; i++) {
      if (i < now) {
        const bx = t[T.blip + i * 2], bz = t[T.blip + i * 2 + 1];
        if (bx !== seen[i * 2]) hotProp(blips[i].node, "translateX", at((seen[i * 2] = bx)));
        if (bz !== seen[i * 2 + 1]) hotProp(blips[i].node, "translateY", at((seen[i * 2 + 1] = bz)));
      }
      // A blip joins or leaves the list.
      if (i < now !== i < count) hotProp(blips[i].node, "opacity", i < now ? 1 : 0);
    }
    count = now;
  });
  return (
    <View class="relative" style={{ width: props.size, height: props.size, bgColor: "#10161c" }}>
      <Image ref={picture} class="absolute" style={{ insetL: 0, insetT: 0, width: (props.size * chart().texture) / chart().side, height: (props.size * chart().texture) / chart().side, display: chart().shown ? 0 : 1 }} />
      <View class="absolute" style={{ insetL: 0, insetT: 0, width: props.size, height: props.size, borderWidth: 1, borderColor: HAIRLINE }} />
      <For each={blips}>
        {(blip) => <View ref={blip.node} class="absolute rounded-full w-[5] h-[5]" style={{ insetL: -2, insetT: -2, opacity: 0, bgColor: RED, borderWidth: 1, borderColor: OUTLINE }} />}
      </For>
      <View ref={me} class="absolute" style={{ insetL: 0, insetT: 0, width: 0, height: 0 }}>
        <View ref={tick} class="absolute" style={{ insetL: -1, insetT: -1, width: 2, height: 2, bgColor: "#ffffff" }} />
        <View class="absolute rounded-full w-[6] h-[6]" style={{ insetL: -3, insetT: -3, bgColor: "#ffffff", borderWidth: 1, borderColor: OUTLINE }} />
      </View>
    </View>
  );
}

/**
 * The kill feed: who, an arrow, whom, and a ring when it was a shot to the
 * head. The player's own name is amber. Four lines stay built; a line that
 * arrives or leaves writes their texts, a few times a minute.
 */
export function KillFeed(props: { match: Match; width: number; left?: boolean }) {
  return (
    <View class={props.left ? "flex-col items-start" : "flex-col items-end"} style={{ width: props.width }}>
      <For each={[0, 1, 2, 3]}>
        {(index) => {
          const line = () => props.match.feed()[index];
          return (
            <View class="flex-row items-center" style={{ height: 15, marginB: 2, paddingL: 5, gap: 4, bgColor: PLATE, display: line() ? 0 : 1 }}>
              <Text class="text-xs font-bold" style={{ textColor: line()?.mine === 1 ? AMBER : FRIEND }}>{(line()?.by ?? "").toUpperCase()}</Text>
              {/* The arrow: a wedge of four lines. */}
              <View class="flex-row items-center" style={{ height: 7 }}>
                <For each={[7, 5, 3, 1]}>{(height) => <View style={{ width: 1, height, bgColor: DIM }} />}</For>
              </View>
              <Text class="text-xs font-bold" style={{ textColor: line()?.mine === 2 ? AMBER : ENEMY }}>{(line()?.whom ?? "").toUpperCase()}</Text>
              <View class="relative rounded-full w-[9] h-[9]" style={{ borderWidth: 1, borderColor: AMBER, display: line()?.head ? 0 : 1 }}>
                <View class="absolute" style={{ insetL: 3, insetT: 3, width: 3, height: 3, bgColor: AMBER }} />
              </View>
              <View style={{ width: 2, height: 15, bgColor: line()?.mine ? AMBER : HAIRLINE }} />
            </View>
          );
        }}
      </For>
    </View>
  );
}

/** A line across the upper screen and a smaller amber one under it, while the match's banner stands. */
export function Banner(props: { match: Match; width: number }) {
  const banner = props.match.banner;
  return (
    <View class={props.match.bannered() ? "flex-col items-center opacity-100 transition-opacity duration-200" : "flex-col items-center opacity-0 transition-opacity duration-200"} style={{ width: props.width }}>
      <View class="flex-col items-center" style={{ paddingL: 10, paddingR: 10, bgColor: PLATE }}>
        <Text class="text-xl font-bold tracking-wide" style={{ height: 22, textColor: INK }}>{banner()[0].toUpperCase() || " "}</Text>
        <Text class="text-xs font-bold tracking-wide" style={{ height: 15, textColor: AMBER, display: banner()[1] ? 0 : 1 }}>{banner()[1].toUpperCase() || " "}</Text>
      </View>
    </View>
  );
}

/** The renderer's statistics line, while its setting is on. */
export function Stats(props: { host: Host }) {
  return (
    <View class="flex-row items-center" style={{ height: 17, paddingL: 6, paddingR: 6, bgColor: PLATE, display: props.host.stats() ? 0 : 1 }}>
      <Text class="text-xs" style={{ textColor: DIM }}>{props.host.stats() || " "}</Text>
    </View>
  );
}

/** What the screen says while the player waits to be put back. */
export function Killed(props: { host: Host; width: number }) {
  let seconds: Node;
  let seen = NaN, shown = -1;
  watch(props.host, (t) => {
    if (t[T.respawn] === seen) return;
    seen = t[T.respawn];
    const left = Math.max(0, Math.ceil((seen || 0) / 10));
    if (left !== shown) hotText(seconds, (shown = left));
  });
  return (
    <View class="flex-col items-center" style={{ width: props.width }}>
      <View class="flex-col items-center" style={{ paddingL: 22, paddingR: 22, paddingT: 6, paddingB: 6, bgColor: GLASS }}>
        <Text class="text-xl font-bold tracking-wide" style={{ height: 24, textColor: RED }}>YOU WERE KILLED</Text>
        <View class="flex-row items-center" style={{ height: 18, gap: 5 }}>
          <Text class="text-sm" style={{ textColor: DIM }}>Back in</Text>
          <Text ref={seconds} class="text-sm font-bold" style={{ width: 10, height: 18, textColor: INK }}>0</Text>
        </View>
      </View>
    </View>
  );
}

// ---------------------------------------------------------------------------
// Lists
// ---------------------------------------------------------------------------

/** The strip along the bottom of a screen with buttons: a notice at the left,
 *  what the buttons do at the right. */
export function Legend(props: { width: number; left?: string; legend: string }) {
  return (
    <View class="relative" style={{ width: props.width, height: 24, bgColor: GLASS }}>
      <View class="absolute" style={{ insetL: 0, insetT: 0, width: props.width, height: 1, bgColor: HAIRLINE }} />
      <Text class="absolute text-xs" style={{ insetL: 10, insetT: 5, textColor: DIM }}>{props.left ?? ""}</Text>
      <Text class="absolute text-xs" style={{ insetR: 10, insetT: 5, textColor: INK }}>{props.legend}</Text>
    </View>
  );
}

/** A list's focus, and the buttons that drive it. One for the life of a
 *  presentation: it watches the pad through the match too, so a button held
 *  when a list opens is not taken for a press. */
export interface Menu {
  focus: () => number;
  press(index: number): void;
  actions: ActionsHandle;
}

export function createMenu(match: Match): Menu {
  const [focus, setFocus] = createSignal(0);
  const pressable = (row: Row | undefined) => !!row?.press;
  // A list starts on its first row; one that opened another comes back to the row that opened it.
  let kept = 0;
  createEffect(on(() => [match.mode(), match.sheet()] as const, ([mode, sheet], before) => {
    if (before && before[0] === mode && before[1] === "menu" && sheet !== "menu") {
      kept = untrack(focus);
      setFocus(0);
    } else if (before && before[0] === mode && sheet === "menu") setFocus(kept);
    else setFocus((kept = 0));
  }));
  // The mark slides to its row, and a switch's knob to its side, over the turns after the one
  // that moved them: the renderer keeps turning the guest until both have arrived.
  const settle = () => match.listing() && match.schedule.hold(0.25);
  createEffect(on(focus, settle, { defer: true }));
  createEffect(on(match.host.options, settle, { defer: true }));
  const press = (index: number) => {
    const row = match.rows()[index];
    if (!pressable(row)) return;
    setFocus(index);
    row!.press!();
  };
  const step = (by: number) => {
    const rows = match.rows();
    for (let at = focus() + by; at >= 0 && at < rows.length; at += by) {
      if (pressable(rows[at])) return setFocus(at);
    }
  };
  let previous = ~0;
  onFrame((buttons) => {
    const pressed = buttons & ~previous;
    previous = buttons;
    if (!match.listing()) return;
    if (pressed & BTN.UP) step(-1);
    if (pressed & BTN.DOWN) step(1);
  });
  const focused = () => match.rows()[focus()];
  // The bindings are read every turn: built again only when the mode, the list or the focus changes.
  const actions = useActions(createMemo(() => ({
    confirm: { label: match.sheet() === "maps" ? "play" : focused()?.on || focused()?.value !== undefined ? "change" : "select", run: () => press(focus()), when: () => match.listing() && pressable(focused()) },
    back: { label: match.backLabel(), run: match.back, when: () => match.listing() && !!match.backLabel() },
    // START opens the menu over a match, and closes it. A player who is down has no button here.
    media: { label: match.mode() === "play" ? "menu" : undefined, run: () => (match.mode() === "menu" ? match.back() : match.menu()), when: () => match.mode() === "play" || (match.mode() === "menu" && match.sheet() === "menu") },
  })));
  return { focus, press, actions };
}

/** A switch: its knob at the right when on. */
function Switch(props: { on: boolean }) {
  return (
    <View class="relative rounded-full w-[34] h-[18]" style={{ bgColor: props.on ? AMBER : "#ffffff30" }}>
      <View class="absolute rounded-full w-[14] h-[14] transition-transform duration-150" style={{ insetL: 2, insetT: 2, translateX: props.on ? 16 : 0, bgColor: props.on ? NIGHT : INK }} />
    </View>
  );
}

/** What a row shows: its label, at the right its switch or its named value, and a rule under it. */
export function Face(props: { row: Row; width: number; height: number }) {
  return (
    <>
      <Text class="absolute text-sm font-bold" style={{ insetL: 14, insetT: props.height / 2 - 9, textColor: INK }}>{read(props.row.label)}</Text>
      <Show when={props.row.on}>
        <View class="absolute" style={{ insetR: 14, insetT: props.height / 2 - 9 }}><Switch on={props.row.on!()} /></View>
      </Show>
      <Show when={props.row.value !== undefined}>
        <Text class="absolute text-sm" style={{ insetR: 14, insetT: props.height / 2 - 9, textColor: AMBER }}>{read(props.row.value!)}</Text>
      </Show>
      <View class="absolute" style={{ insetL: 0, insetB: 0, width: props.width, height: 1, bgColor: HAIRLINE }} />
    </>
  );
}

/** The bar at the left of a list's mark; the mark itself is a wash a row tall. */
export function Mark(props: { height: number }) {
  return <View class="absolute" style={{ insetL: 0, insetT: 0, width: 3, height: props.height, bgColor: AMBER }} />;
}

/**
 * The rows of the list that is up. A row takes a tap and the list's mark:
 * one bar, so moving it changes one node. Where the device has buttons the
 * mark is the focus and slides to its row; where it has none the mark stands
 * under the finger while a row is pressed.
 *
 * Every list this view has shown stays built, hidden: showing one again
 * builds nothing, and a switch that turns writes its own row. `warm` lists
 * are built with the view, before they are first shown.
 */
export function Rows(props: { match: Match; menu: Menu; width: number; rowHeight: number; surface?: SurfaceId; active?: () => boolean; warm?: Row[][] }) {
  // A list that is kept but hidden (`active` false) holds its rows and its mark as they were.
  const live = () => props.active?.() ?? true;
  const rows = createMemo<Row[]>((before) => (live() ? props.match.rows() : before), props.warm?.[0] ?? []);
  const focus = createMemo<number>((before) => (live() ? props.menu.focus() : before), 0);
  const built = createMemo<Row[][]>((before) => (!rows().length || before.includes(rows()) ? before : [...before, rows()]), props.warm ?? []);
  // The row a finger is on, where no button moves a focus.
  const [held, setHeld] = createSignal(-1);
  const marked = () => (modality.buttons ? focus() : held());
  return (
    <View class="relative flex-col" style={{ width: props.width }}>
      <View class={modality.buttons ? "absolute transition-transform duration-100 ease-out" : "absolute"} style={{ insetL: 0, insetT: 0, width: props.width, height: props.rowHeight, translateY: Math.max(0, marked()) * props.rowHeight, display: marked() < 0 || !rows().length ? 1 : 0, bgColor: WASH }}>
        <Mark height={props.rowHeight} />
      </View>
      <For each={built()}>
        {(list) => {
          // A list keeps the row height it was built with: another list's height does not reach it.
          const height = untrack(() => props.rowHeight);
          return (
            <View class="flex-col" style={{ width: props.width, display: list === rows() ? 0 : 1 }}>
              <For each={list}>
                {(row, index) => (
                  <Touchable surface={props.surface} class="relative" style={{ width: props.width, height }} onTap={() => props.menu.press(index())} onHeld={modality.buttons ? undefined : (on) => setHeld(on ? index() : -1)}>
                    <Face row={row} width={props.width} height={height} />
                  </Touchable>
                )}
              </For>
            </View>
          );
        }}
      </For>
    </View>
  );
}

/**
 * A screen that is built the first time `when` holds (at once with `eager`)
 * and then stays, shown or hidden. Showing it again builds nothing: on the
 * PSP a screen takes tenths of a second to build, which the loading screen
 * can spend and the moment a list closes cannot.
 */
export function Keep(props: { when: boolean; eager?: boolean; width: number; height: number; children: JSX.Element }) {
  const [built, setBuilt] = createSignal(!!props.eager);
  createEffect(() => {
    if (props.when) setBuilt(true);
  });
  return (
    <View class="absolute" style={{ insetL: 0, insetT: 0, width: props.width, height: props.height, display: props.when ? 0 : 1, hitPass: 1 }}>
      <Show when={built()}>{props.children}</Show>
    </View>
  );
}

/** A list's heading over a rule. */
export function Heading(props: { text: string; width: number; height: number }) {
  return (
    <View class="relative" style={{ width: props.width, height: props.height }}>
      <Text class="absolute text-xs font-bold tracking-wide" style={{ insetL: 14, insetT: props.height / 2 - 8, textColor: DIM }}>{props.text}</Text>
      <View class="absolute" style={{ insetL: 0, insetB: 0, width: props.width, height: 1, bgColor: HAIRLINE }} />
    </View>
  );
}

/**
 * A panel in the middle of a surface. Nothing dims the scene around it: on a
 * PSP the GE blends every pixel a layer covers, and a layer over the whole
 * screen costs more than the panel.
 */
export function Panel(props: { width: number; height: number; panelWidth: number; panelHeight: number; children: JSX.Element }) {
  return (
    <View class="absolute flex-row" style={{ insetL: Math.round((props.width - props.panelWidth) / 2), insetT: Math.max(4, Math.round((props.height - props.panelHeight) / 2)), width: props.panelWidth, height: props.panelHeight, bgColor: PANEL, borderWidth: 1, borderColor: HAIRLINE }}>
      {props.children}
    </View>
  );
}

/** Rows that only inform, in columns: a label and what it means. */
export function Facts(props: { rows: Row[]; width: number; columns: number; rowHeight: number; labelWidth: number }) {
  const each = () => Math.ceil(props.rows.length / props.columns);
  const column = () => Math.floor(props.width / props.columns);
  return (
    <View class="relative" style={{ width: props.width, height: each() * props.rowHeight }}>
      <For each={props.rows}>
        {(row, index) => (
          <>
            <Text class="absolute text-xs font-bold" style={{ insetL: 14 + Math.floor(index() / each()) * column(), insetT: (index() % each()) * props.rowHeight, textColor: AMBER }}>{read(row.label)}</Text>
            <Text class="absolute text-xs" style={{ insetL: 14 + Math.floor(index() / each()) * column() + props.labelWidth, insetT: (index() % each()) * props.rowHeight, textColor: DIM }}>{read(row.value ?? "")}</Text>
          </>
        )}
      </For>
    </View>
  );
}

/** What the game is and where it comes from, over what the buttons do. */
export function About(props: { match: Match; width: number; rowHeight?: number }) {
  return (
    <View class="flex-col" style={{ width: props.width }}>
      <View class="flex-col" style={{ paddingL: 14, paddingT: 8, paddingB: 8 }}>
        <For each={CREDITS}>{(line) => <Text class="text-xs" style={{ height: 16, textColor: INK }}>{line}</Text>}</For>
      </View>
      <Heading text="CONTROLS" width={props.width} height={24} />
      <View style={{ height: 8 }} />
      <Facts rows={props.match.controls} width={props.width} columns={2} rowHeight={props.rowHeight ?? 17} labelWidth={props.width < 440 ? 82 : 92} />
      <View style={{ height: 8 }} />
    </View>
  );
}

/** How the match ended: the word, and the two scores. */
export function Outcome(props: { host: Host }) {
  const result = props.host.result;
  // Level scores when the clock ran out: nobody won.
  const level = () => result()?.us === result()?.them;
  return (
    <View class="flex-col" style={{ paddingL: 14 }}>
      <Text class="text-xs font-bold tracking-wide" style={{ height: 16, textColor: DIM }}>TEAM DEATHMATCH</Text>
      <Text class="text-4xl font-bold" style={{ height: 42, textColor: level() ? INK : result()?.won ? OK : RED }}>{level() ? "DRAW" : result()?.won ? "VICTORY" : "DEFEAT"}</Text>
      <View class="flex-row items-center" style={{ height: 24, gap: 8 }}>
        <Text class="text-xl font-bold" style={{ textColor: FRIEND }}>{`${result()?.us ?? 0}`}</Text>
        <Text class="text-sm" style={{ textColor: FAINT }}>–</Text>
        <Text class="text-xl font-bold" style={{ textColor: ENEMY }}>{`${result()?.them ?? 0}`}</Text>
      </View>
    </View>
  );
}

/** The player's own match in five lines. */
export function Tally(props: { host: Host; width: number; rowHeight: number }) {
  const result = props.host.result;
  const lines: [string, () => string][] = [
    ["KILLS", () => `${result()?.kills ?? 0}`],
    ["DEATHS", () => `${result()?.deaths ?? 0}`],
    ["HEAD SHOTS", () => `${result()?.headshots ?? 0}`],
    ["ACCURACY", () => `${result()?.accuracy ?? 0}%`],
    ["TIME", () => clock(result()?.seconds ?? 0)],
  ];
  return (
    <View class="flex-col" style={{ width: props.width }}>
      <For each={lines}>
        {([label, value]) => (
          <View class="relative" style={{ width: props.width, height: props.rowHeight }}>
            <Text class="absolute text-xs font-bold tracking-wide" style={{ insetL: 14, insetT: props.rowHeight / 2 - 8, textColor: DIM }}>{label}</Text>
            <Text class="absolute text-sm font-bold" style={{ insetR: 14, insetT: props.rowHeight / 2 - 9, textColor: INK }}>{value()}</Text>
            <View class="absolute" style={{ insetL: 0, insetB: 0, width: props.width, height: 1, bgColor: HAIRLINE }} />
          </View>
        )}
      </For>
    </View>
  );
}

/** The screen while the level loads, and when it could not. */
export function Loading(props: { host: Host; width: number; height: number }) {
  return (
    <View class="items-center justify-center flex-col gap-3" style={{ width: props.width, height: props.height, bgColor: NIGHT }}>
      <Wordmark />
      <Text class="text-xs" style={{ textColor: props.host.mode() === "error" ? RED : DIM }}>{props.host.message() || "Loading"}</Text>
    </View>
  );
}
