// One 480×272 screen with a pad: the PSP, and the Vita, whose panel also
// takes taps on a list's rows. During play the match has the screen and the
// interface is the readouts at its edges and the marks at its centre; a list
// comes up over it for the title, the menu and the results.
import { createMemo, Show } from "solid-js";
import { Text, View } from "@pocketjs/framework/components";
import { connectHost } from "../host.ts";
import { createMatch, type Match, type Sheet } from "../match.ts";
import { About, Ammo, Banner, Compass, createMenu, Crosshair, Edges, Heading, Hitmarker, Hurt, Keep, KillFeed, Killed, Legend, LevelMap, Loading, MatchBar, type Menu, Outcome, Panel, ReloadPrompt, Rows, Stats, Tally, Vitals, Wordmark } from "../parts.tsx";
import { DIM, HAIRLINE } from "../theme.ts";

const W = 480, H = 272;
const FOOTER = 24;
/** A list's heading and one of its rows. */
const HEAD = 28, ROW = 30;

export default function SingleScreen() {
  const host = connectHost();
  const match = createMatch(host);
  const menu = createMenu(match);
  const mode = host.mode;
  return (
    <View class="relative w-full h-full">
      <Show when={mode() === "loading" || mode() === "error"}><Loading host={host} width={W} height={H} /></Show>
      <Keep when={mode() === "title"} eager width={W} height={H}><Title match={match} menu={menu} /></Keep>
      <Keep when={mode() === "play" || mode() === "down"} eager width={W} height={H}><Hud match={match} /></Keep>
      <Keep when={mode() === "menu"} eager width={W} height={H}><Pause match={match} menu={menu} /></Keep>
      <Keep when={mode() === "results"} eager width={W} height={H}><Results match={match} menu={menu} /></Keep>
      {/* The strip stays built and is shown with a list: a list coming up builds nothing. */}
      <View class="absolute" style={{ insetL: 0, insetB: 0, display: match.listing() ? 0 : 1 }}>
        <Legend width={W} left={match.notice()} legend={menu.actions.legend()} />
      </View>
    </View>
  );
}

/** The settings as a panel over whatever opened them. */
function Settings(props: { match: Match; menu: Menu; active: () => boolean }) {
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

/** The name over the scene the match will be played in, and the title's rows. A plate at the
 *  left keeps them legible; the rest of the scene stays in view. */
function Title(props: { match: Match; menu: Menu }) {
  const match = props.match;
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
        <Text class="absolute text-xs" style={{ insetL: 24, insetT: 88, textColor: DIM }}>A firefight on a market street.</Text>
        <View class="absolute" style={{ insetL: 10, insetT: 114 }}>
          <Rows match={match} menu={props.menu} width={190} rowHeight={26} active={() => up("menu")} warm={[match.lists.title]} />
        </View>
      </View>
      <Keep when={up("settings")} width={W} height={H - FOOTER}><Settings match={match} menu={props.menu} active={() => up("settings")} /></Keep>
      <Keep when={up("about")} width={W} height={H - FOOTER}>
        <Panel width={W} height={H - FOOTER} panelWidth={440} panelHeight={218}>
          <View class="flex-col">
            <Heading text="ABOUT" width={440} height={HEAD} />
            <About match={match} width={440} />
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
function Hud(props: { match: Match }) {
  const host = props.match.host;
  const down = () => host.mode() === "down";
  return (
    <View class="relative w-full h-full">
      <Edges host={host} width={W} height={H} />
      <View class="absolute" style={{ insetL: 0, insetT: 0, width: W, height: H, opacity: down() ? 0.4 : 1, hitPass: 1 }}>
        <View class="absolute" style={{ insetL: 10, insetT: 10 }}><LevelMap host={host} size={MAP} /></View>
        <View class="absolute" style={{ insetL: 10, insetT: 10 + MAP + 10 }}><Stats host={host} /></View>
        <View class="absolute" style={{ insetL: W / 2 - 66, insetT: 8 }}><Compass host={host} width={132} /></View>
        <View class="absolute" style={{ insetL: W / 2 - 66, insetT: 30 }}><MatchBar host={host} width={132} /></View>
        <View class="absolute" style={{ insetR: 10, insetT: 10 }}><KillFeed match={props.match} width={160} /></View>
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
function Pause(props: { match: Match; menu: Menu }) {
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
      <Keep when={up("settings")} width={W} height={H - FOOTER}><Settings match={match} menu={props.menu} active={() => up("settings")} /></Keep>
    </View>
  );
}

/** How the match ended: the word and the score over what to do next, beside the player's own numbers. */
function Results(props: { match: Match; menu: Menu }) {
  const match = props.match;
  const left = 236, right = 180, height = 176;
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
