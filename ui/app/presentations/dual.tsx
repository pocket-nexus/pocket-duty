// Two screens (the 3DS): the match and every list on the upper one, laid
// out as on a console with one screen; on the touch screen below, the level
// from above with the score beside it and four keys a thumb or a stylus
// taps. The map is the lower screen's, so the upper one's readouts leave
// it out.
import { Show } from "solid-js";
import { AuxiliarySurface, Text, View } from "@pocketjs/framework/components";
import { connectHost, type Host } from "../host.ts";
import { createMatch, type Match } from "../match.ts";
import { Board, createMenu, Keep, Legend, LevelMap, Loading, Touchable } from "../parts.tsx";
import { AMBER, DIM, GLASS, HAIRLINE, INK, NIGHT, PLATE } from "../theme.ts";
import { type Frame, Hud, Pause, Results, Title } from "./screens.tsx";

const TOP: Frame = { w: 400, h: 240, map: false };
const LOW = { w: 320, h: 240 };
/** The map's side on the lower screen, and the column beside it. */
const MAP = 240;
const SIDE = LOW.w - MAP;

export default function DualScreen() {
  const host = connectHost();
  const match = createMatch(host);
  const menu = createMenu(match);
  const mode = host.mode;
  return (
    <>
      <View class="relative" style={{ width: TOP.w, height: TOP.h }}>
        <Show when={mode() === "loading" || mode() === "error"}><Loading host={host} width={TOP.w} height={TOP.h} /></Show>
        <Keep when={mode() === "title"} eager width={TOP.w} height={TOP.h}><Title frame={TOP} match={match} menu={menu} /></Keep>
        <Keep when={mode() === "play" || mode() === "down"} eager width={TOP.w} height={TOP.h}><Hud frame={TOP} match={match} /></Keep>
        <Keep when={mode() === "menu"} eager width={TOP.w} height={TOP.h}><Pause frame={TOP} match={match} menu={menu} /></Keep>
        <Keep when={mode() === "results"} eager width={TOP.w} height={TOP.h}><Results frame={TOP} match={match} menu={menu} /></Keep>
        <View class="absolute" style={{ insetL: 0, insetB: 0, display: match.listing() ? 0 : 1 }}>
          <Legend width={TOP.w} left={match.notice()} legend={menu.actions.legend()} />
        </View>
      </View>
      <AuxiliarySurface>{() => <Lower match={match} />}</AuxiliarySurface>
    </>
  );
}

/** A key of the touch screen: a word, and under it what it now stands at. */
function Key(props: { top: number; label: string; value?: () => string; onTap: () => void }) {
  return (
    <Touchable surface="auxiliary" onTap={props.onTap} class="absolute" style={{ insetL: 4, insetT: props.top, width: SIDE - 8, height: 34, bgColor: PLATE, borderWidth: 1, borderColor: HAIRLINE }}>
      <Text class="absolute text-xs font-bold tracking-wide" style={{ insetL: 7, insetT: props.value ? 3 : 9, textColor: INK }}>{props.label}</Text>
      <Show when={props.value}>
        <Text class="absolute text-xs font-bold" style={{ insetL: 7, insetT: 17, textColor: AMBER }}>{props.value!() || " "}</Text>
      </Show>
    </Touchable>
  );
}

/** What the buttons do, for the card over the map while no match has the pad. */
const CONTROLS: [string, string][] = [
  ["Circle Pad", "Walk; all the way, run"],
  ["A B X Y", "Look"],
  ["R / L", "Fire / raise the sights"],
  ["+Pad up, down", "Jump, crouch"],
  ["+Pad left, right", "Reload, next weapon"],
  ["SELECT", "Fire mode"],
  ["START", "Menu"],
  ["C-Stick", "Looks, on a New 3DS"],
];
const CARD = { w: 232, row: 17 };

/**
 * The touch screen. The map, the score and the keys stay built and stay
 * where they are; while a list is up on the upper screen the map steps back
 * under a card that says what the buttons do, and the one key that fits the
 * moment stands in the column.
 */
function Lower(props: { match: Match }) {
  const match = props.match;
  const host: Host = match.host;
  const mode = host.mode;
  const playing = () => mode() === "play" || mode() === "down";
  const waiting = () => mode() === "loading" || mode() === "error";
  const tap = (key: "weapon" | "mode" | "reload") => () => host.send({ type: "tap", key });
  return (
    <View class="relative" style={{ width: LOW.w, height: LOW.h, bgColor: NIGHT }}>
      <View class="absolute" style={{ insetL: 0, insetT: 0, width: MAP, height: MAP, opacity: playing() ? 1 : 0.35 }}>
        <LevelMap host={host} size={MAP} />
      </View>
      <View class="absolute" style={{ insetL: MAP, insetT: 0, width: 1, height: LOW.h, bgColor: HAIRLINE }} />
      <View class="absolute" style={{ insetL: MAP + 1, insetT: 0 }}><Board host={host} width={SIDE - 1} /></View>

      {/* The keys of a match. */}
      <View class="absolute" style={{ insetL: MAP, insetT: 0, width: SIDE, height: LOW.h, display: playing() ? 0 : 1, hitPass: 1 }}>
        <Key top={90} label="WEAPON" value={() => host.weapon().toUpperCase()} onTap={tap("weapon")} />
        <Key top={127} label="MODE" value={() => host.fire().toUpperCase()} onTap={tap("mode")} />
        <Key top={164} label="RELOAD" onTap={tap("reload")} />
        <Key top={201} label="MENU" onTap={match.menu} />
      </View>
      {/* The one key of a list: what leaves it for the match. The results' rows are the upper screen's. */}
      <View class="absolute" style={{ insetL: MAP, insetT: 0, width: SIDE, height: LOW.h, display: mode() === "title" || mode() === "menu" ? 0 : 1, hitPass: 1 }}>
        {/* (with the list of maps up, the key plays the map behind it) */}
        <Key top={201} label={mode() === "menu" ? "RESUME" : match.sheet() === "maps" ? "PLAY" : "START"} onTap={() => (mode() === "menu" ? host.send({ type: "menu", on: false }) : match.sheet() === "maps" ? match.start() : match.lists.title[0].press?.())} />
      </View>

      <View class="absolute" style={{ insetL: (MAP - CARD.w) / 2, insetT: 20, width: CARD.w, height: 24 + CONTROLS.length * CARD.row + 8, display: playing() || waiting() ? 1 : 0, bgColor: GLASS, borderWidth: 1, borderColor: HAIRLINE }}>
        <Text class="absolute text-xs font-bold tracking-wide" style={{ insetL: 10, insetT: 5, textColor: DIM }}>CONTROLS</Text>
        <View class="absolute" style={{ insetL: 0, insetT: 23, width: CARD.w, height: 1, bgColor: HAIRLINE }} />
        {CONTROLS.map(([button, does], index) => (
          <>
            <Text class="absolute text-xs font-bold" style={{ insetL: 10, insetT: 28 + index * CARD.row, textColor: AMBER }}>{button}</Text>
            <Text class="absolute text-xs" style={{ insetL: 104, insetT: 28 + index * CARD.row, textColor: INK }}>{does}</Text>
          </>
        ))}
      </View>
      <Show when={waiting()}>
        <View class="absolute items-center justify-center" style={{ insetL: 0, insetT: 0, width: MAP, height: LOW.h }}>
          <Text class="text-sm" style={{ textColor: DIM }}>{mode() === "error" ? "The level could not be read" : "Loading"}</Text>
        </View>
      </Show>
    </View>
  );
}
