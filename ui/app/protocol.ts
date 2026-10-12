// What crosses between a device's renderer and the interface. The renderer
// owns the match and the scene; the interface owns every 2D pixel and what a
// button means while a list is up. Both sides speak JSON lines over
// PocketJS's in-process overlay service: the renderer sends the members of
// its state that changed, the interface sends commands.
// `crates/duty-interface` is the renderer's side.
//
// The renderer parses commands with a small scanner, so commands are flat
// objects of numbers, booleans and short strings.

/** What the screen is for.
 *  `play`: the match has the screen, and the pad is the soldier's.
 *  `down`: the player was killed; the match goes on and puts them back in `t[T.respawn]` tenths of a second.
 *  `menu`: a list is up over a paused match. `results`: the match is over. */
export type Mode = "loading" | "title" | "play" | "down" | "menu" | "results" | "error";

/** A switch (0 or 1), or with `choices` one of several named values. */
export interface Setting {
  key: string;
  value: number;
  choices?: string[];
}

/** One line of the kill feed: who, whom, and whether it was a shot to the head. */
export interface Kill {
  /** Rises by one with every line, so a line shown twice is one line. */
  id: number;
  by: string;
  whom: string;
  head: boolean;
  /** The player is `by` (1), is `whom` (2), or neither (0). */
  mine: number;
}

/** How a match ended. */
export interface Result {
  won: boolean;
  us: number;
  them: number;
  kills: number;
  deaths: number;
  headshots: number;
  /** Shots that hit, of a hundred fired. */
  accuracy: number;
  /** Seconds the match ran. */
  seconds: number;
}

export interface HostState {
  mode: Mode;
  /** The loading step, or why `mode` is "error". */
  message: string;
  /** What the device lets the person set, in menu order. */
  options: Setting[];
  /** One line of renderer statistics while the `stats` setting is on. */
  stats: string;
  /** What the interface last stored with `prefs`. */
  prefs: string;
  /** The `wake` the interface asked for that last came due; 0 before any has. */
  woke: number;
  /** The weapon in hand ("M4A1") and its fire mode ("AUTO"). */
  weapon: string;
  fire: string;
  /** The newest lines of the kill feed, newest first, four at most. */
  feed: Kill[];
  /** A line across the upper screen for two seconds: `[id, title, subtitle]`; id 0 is none. */
  banner: [number, string, string];
  /** Set while `mode` is "results". */
  result: Result | null;
  /** The numbers that change during play, at most once a turn (see `T`). */
  t: number[];
}

/** Indices into `HostState.t`. */
export const T = {
  /** 0…100. */
  health: 0,
  /** Rounds in the magazine, and in reserve. */
  ammo: 1,
  reserve: 2,
  /** Degrees clockwise from north (-z), 0…359. */
  heading: 3,
  /** The player on the map: decimetres east and south of the map picture's top left corner. */
  x: 4,
  z: 5,
  /** Half the gap between the crosshair's blades, logical pixels; 0 hides the crosshair (aiming down the sights). */
  spread: 6,
  /** The score: the player's side, the other side. */
  us: 7,
  them: 8,
  /** Seconds the match has left. */
  clock: 9,
  /** The last hit the player landed: `count × 4 + kind` (kind 0 body, 1 head, 2 kill). It changes with every hit. */
  hit: 10,
  /** The last hit the player took: `count × 360 + degrees`, the degrees clockwise from straight ahead to where it came from. */
  hurt: 11,
  /** How far a reload is, 0…100; 0 when none runs. */
  reload: 12,
  /** `FLAG` bits. */
  flags: 13,
  /** Tenths of a second until the player is put back, while `mode` is "down". */
  respawn: 14,
  /** How many of the blips that follow are set. */
  blips: 15,
  /** Enemies the map shows (they fired, or are in view): pairs of decimetres like `x` and `z`. */
  blip: 16,
} as const;

/** Blips the map shows at most. */
export const BLIPS = 6;

export const FLAG = {
  /** Aiming down the sights. */
  ads: 1,
  sprint: 2,
  crouch: 4,
  /** A reload runs. */
  reloading: 8,
  /** The magazine is empty. */
  empty: 16,
} as const;

export type Command =
  /** Leave the title: a match starts. */
  | { type: "start" }
  /** A list comes up over the match (which stops), or leaves it. */
  | { type: "menu"; on: boolean }
  /** A new match on the same ground, from the menu or the results. */
  | { type: "restart" }
  /** Back to the title. */
  | { type: "title" }
  | { type: "option"; key: string; value: number }
  | { type: "prefs"; value: string }
  /** The interface has nothing scheduled (nothing fading). A turn of the
   *  guest costs milliseconds on the slower machines, so while this is on
   *  the renderer turns it only when it has news or a button the interface
   *  listens to changes. */
  | { type: "idle"; on: boolean }
  /** Say `id` back in `HostState.woke` in this many seconds. The interface's
   *  own timers count its turns, and an idle guest takes none: what must
   *  happen later (a banner leaving) is timed by the renderer. */
  | { type: "wake"; id: number; seconds: number };
