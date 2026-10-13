// The screens every device has, for a screen of any size: the title and the
// lists it opens, the match's readouts, the menu over a stopped match and the
// results. A presentation says how large its screen is and whether the map
// stands among the readouts; it does not lay a screen out again.
import { createMemo, Show } from "solid-js";
import { Text, View } from "@pocketjs/framework/components";
import type { Match, Sheet } from "../match.ts";
import { About, Ammo, Banner, Compass, Crosshair, Edges, Heading, Hitmarker, Hurt, Keep, KillFeed, Killed, LevelMap, MatchBar, type Menu, Outcome, Panel, ReloadPrompt, Rows, Stats, Tally, Vitals, Wordmark } from "../parts.tsx";
import { DIM, HAIRLINE } from "../theme.ts";

/** A screen's logical size. `map`: the level from above stands among the match's readouts (a
 *  device with a second screen shows it there instead). */
export interface Frame {
  w: number;
  h: number;
  map: boolean;
}

export const FOOTER = 24;
/** A list's heading and one of its rows. */
const HEAD = 28, ROW = 30;
/** A panel is no wider than the screen less a margin. */
const fit = (frame: Frame, width: number) => Math.min(width, frame.w - 16);

/** The settings as a panel over whatever opened them. */
function Settings(props: { frame: Frame; match: Match; menu: Menu; active: () => boolean }) {
  const { w: W, h: H } = props.frame;
  const width = 320;
  return (
    <Panel width={W} height={H - FOOTER} panelWidth={width} panelHeight={HEAD + props.match.settings().length * ROW}>
      <View class="flex-col">
        <Heading text="SETTINGS" width={width} height={HEAD} />
        <Rows match={props.match} menu={props.menu} width={width} rowHeight={ROW} active={props.active} />
      </View>
    </Panel>
  );
}

/** The maps a match can be played on, as a panel over the title. */
function Maps(props: { frame: Frame; match: Match; menu: Menu; active: () => boolean }) {
  const { w: W, h: H } = props.frame;
  const width = fit(props.frame, 300);
  return (
    <Panel width={W} height={H - FOOTER} panelWidth={width} panelHeight={HEAD + Math.max(1, props.match.maps().length) * ROW}>
      <View class="flex-col">
        <Heading text="MAPS" width={width} height={HEAD} />
        <Rows match={props.match} menu={props.menu} width={width} rowHeight={ROW} active={props.active} />
      </View>
    </Panel>
  );
}

/** The name over the scene the match will be played in, and the title's rows. A plate at the
 *  left keeps them legible; the rest of the scene stays in view. */
export function Title(props: { frame: Frame; match: Match; menu: Menu }) {
  const { w: W, h: H } = props.frame;
  const match = props.match;
  // The words about the game and the buttons: as wide as the screen takes, its rows closer on a low one.
  const about = fit(props.frame, 440), tall = H >= 272;
  const up = (sheet: Sheet) => match.mode() === "title" && match.sheet() === sheet;
  // The name and the title's rows leave while a panel is up: type under a panel shows through its glass.
  const home = createMemo(() => match.mode() !== "title" || match.sheet() === "menu");
  return (
    <View class="relative w-full h-full">
      {/* The title's own rows, and the panels a list opened from them comes up in. All stay built:
          the rows from the start, a panel from the first time it is shown. */}
      <View class="absolute" style={{ insetL: 0, insetT: 0, width: W, height: H, display: home() ? 0 : 1, hitPass: 1 }}>
        <View class="absolute bg-gradient-to-r from-[#000000c8] to-[#00000000]" style={{ insetL: 0, insetT: 0, width: 330, height: H }} />
        <View class="absolute" style={{ insetL: 24, insetT: 22 }}><Wordmark large /></View>
        {/* The map behind the title, in its own words. */}
        <Text class="absolute text-xs" style={{ insetL: 24, insetT: 88, textColor: DIM }}>{match.map().line}</Text>
        <View class="absolute" style={{ insetL: 10, insetT: 114 }}>
          <Rows match={match} menu={props.menu} width={190} rowHeight={26} active={() => up("menu")} warm={[match.lists.title]} />
        </View>
      </View>
      <Keep when={up("settings")} width={W} height={H - FOOTER}><Settings frame={props.frame} match={match} menu={props.menu} active={() => up("settings")} /></Keep>
      <Keep when={up("maps")} width={W} height={H - FOOTER}><Maps frame={props.frame} match={match} menu={props.menu} active={() => up("maps")} /></Keep>
      <Keep when={up("about")} width={W} height={H - FOOTER}>
        <Panel width={W} height={H - FOOTER} panelWidth={about} panelHeight={tall ? 218 : 212}>
          <View class="flex-col">
            <Heading text="ABOUT" width={about} height={HEAD} />
            <About match={match} width={about} rowHeight={tall ? 17 : 16} />
          </View>
        </Panel>
      </Keep>
    </View>
  );
}

const MAP = 72;

/**
 * The match's readouts: the map, the heading and the score above, health
 * and the weapon below, the kill feed at the right, and at the centre the
 * crosshair and what a shot did. While the player is down the readouts dim,
 * the centre's marks leave, and the wait is said in their place.
 */
export function Hud(props: { frame: Frame; match: Match }) {
  const { w: W, h: H } = props.frame;
  const host = props.match.host;
  const down = () => host.mode() === "down";
  return (
    <View class="relative w-full h-full">
      <Edges host={host} width={W} height={H} />
      <View class="absolute" style={{ insetL: 0, insetT: 0, width: W, height: H, opacity: down() ? 0.4 : 1, hitPass: 1 }}>
        <Show when={props.frame.map}><View class="absolute" style={{ insetL: 10, insetT: 10 }}><LevelMap host={host} size={MAP} /></View></Show>
        <View class="absolute" style={{ insetL: 10, insetT: 10 + MAP + 10 }}><Stats host={host} /></View>
        <View class="absolute" style={{ insetL: W / 2 - 66, insetT: 8 }}><Compass host={host} width={132} /></View>
        <View class="absolute" style={{ insetL: W / 2 - 66, insetT: 30 }}><MatchBar host={host} width={132} /></View>
        {/* The feed stands at the right; where the map is not among the readouts its corner is free, and a
            narrower screen has no room between the score and the right edge. */}
        <Show when={props.frame.map} fallback={<View class="absolute" style={{ insetL: 4, insetT: 10 }}><KillFeed match={props.match} width={W / 2 - 72} left /></View>}>
          <View class="absolute" style={{ insetR: 10, insetT: 10 }}><KillFeed match={props.match} width={160} /></View>
        </Show>
        <View class="absolute" style={{ insetL: 10, insetB: 10 }}><Vitals host={host} /></View>
        <View class="absolute" style={{ insetR: 10, insetB: 10 }}><Ammo host={host} /></View>
      </View>
      <View class="absolute" style={{ insetL: W / 2, insetT: H / 2, width: 0, height: 0, display: down() ? 1 : 0 }}>
        <Crosshair host={host} />
        <Hitmarker match={props.match} />
        <Hurt match={props.match} />
        <View class="absolute" style={{ insetL: 0, insetT: 52, width: 0, height: 0 }}><ReloadPrompt host={host} /></View>
      </View>
      {/* The banner stands between the score and the ring of marks around the centre: the mark for
          a hit from straight ahead is 42 pixels above it, at y = 92. */}
      <View class="absolute" style={{ insetL: 0, insetT: 51 }}><Banner match={props.match} width={W} /></View>
      <View class="absolute" style={{ insetL: 0, insetT: H / 2 - 30, display: down() ? 0 : 1 }}><Killed host={host} width={W} /></View>
    </View>
  );
}

/** The menu over a match that has stopped. */
export function Pause(props: { frame: Frame; match: Match; menu: Menu }) {
  const { w: W, h: H } = props.frame;
  const match = props.match;
  const up = (sheet: Sheet) => match.mode() === "menu" && match.sheet() === sheet;
  const width = 260;
  return (
    <View class="relative w-full h-full">
      <Keep when={up("menu")} eager width={W} height={H - FOOTER}>
        <Panel width={W} height={H - FOOTER} panelWidth={width} panelHeight={HEAD + match.lists.menu.length * ROW}>
          <View class="flex-col">
            <Heading text="PAUSED" width={width} height={HEAD} />
            <Rows match={match} menu={props.menu} width={width} rowHeight={ROW} active={() => up("menu")} warm={[match.lists.menu]} />
          </View>
        </Panel>
      </Keep>
      <Keep when={up("settings")} width={W} height={H - FOOTER}><Settings frame={props.frame} match={match} menu={props.menu} active={() => up("settings")} /></Keep>
    </View>
  );
}

/** How the match ended: the word and the score over what to do next, beside the player's own numbers. */
export function Results(props: { frame: Frame; match: Match; menu: Menu }) {
  const { w: W, h: H } = props.frame;
  const match = props.match;
  // Two columns, narrowed together where the screen is.
  const all = fit(props.frame, 416), right = Math.round((all * 180) / 416), left = all - right, height = 176;
  return (
    <Panel width={W} height={H - FOOTER} panelWidth={left + right} panelHeight={height}>
      <View class="relative" style={{ width: left, height }}>
        <View class="absolute" style={{ insetL: 0, insetT: 10 }}><Outcome host={match.host} /></View>
        <View class="absolute flex-col" style={{ insetL: 0, insetB: 0, width: left, height: match.lists.results.length * ROW + 1 }}>
          <View style={{ width: left, height: 1, bgColor: HAIRLINE }} />
          <Rows match={match} menu={props.menu} width={left} rowHeight={ROW} active={() => match.mode() === "results"} warm={[match.lists.results]} />
        </View>
      </View>
      <View style={{ width: 1, height, bgColor: HAIRLINE }} />
      <View class="flex-col" style={{ width: right - 1, height }}>
        <Heading text="YOUR MATCH" width={right - 1} height={HEAD - 3} />
        <Tally host={match.host} width={right - 1} rowHeight={ROW} />
      </View>
    </Panel>
  );
}
