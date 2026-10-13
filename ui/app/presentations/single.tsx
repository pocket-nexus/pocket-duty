// One 480×272 screen with a pad: the PSP, and the Vita, whose panel also
// takes taps on a list's rows. During play the match has the screen and the
// interface is the readouts at its edges and the marks at its centre; a list
// comes up over it for the title, the menu and the results.
import { Show } from "solid-js";
import { View } from "@pocketjs/framework/components";
import { connectHost } from "../host.ts";
import { createMatch } from "../match.ts";
import { createMenu, Keep, Legend, Loading } from "../parts.tsx";
import { type Frame, Hud, Pause, Results, Title } from "./screens.tsx";

const W = 480, H = 272;
const FRAME: Frame = { w: W, h: H, map: true };

export default function SingleScreen() {
  const host = connectHost();
  const match = createMatch(host);
  const menu = createMenu(match);
  const mode = host.mode;
  return (
    <View class="relative w-full h-full">
      <Show when={mode() === "loading" || mode() === "error"}><Loading host={host} width={W} height={H} /></Show>
      <Keep when={mode() === "title"} eager width={W} height={H}><Title frame={FRAME} match={match} menu={menu} /></Keep>
      <Keep when={mode() === "play" || mode() === "down"} eager width={W} height={H}><Hud frame={FRAME} match={match} /></Keep>
      <Keep when={mode() === "menu"} eager width={W} height={H}><Pause frame={FRAME} match={match} menu={menu} /></Keep>
      <Keep when={mode() === "results"} eager width={W} height={H}><Results frame={FRAME} match={match} menu={menu} /></Keep>
      {/* The strip stays built and is shown with a list: a list coming up builds nothing. */}
      <View class="absolute" style={{ insetL: 0, insetB: 0, display: match.listing() ? 0 : 1 }}>
        <Legend width={W} left={match.notice()} legend={menu.actions.legend()} />
      </View>
    </View>
  );
}
