//! The flow around a match, the same on every device: the street behind the
//! title, the match with the pad in the soldier's hands, the menu over it,
//! the results. A device owns the pad's wiring and the scene; it hands this
//! its [`Pad`] each frame and draws what the match then holds.

use alloc::string::String;
use alloc::vec::Vec;

use duty_sim::event::Event;
use duty_sim::game::{Game, Phase};
use duty_sim::math::*;
use duty_sim::player::{btn, Input};
use duty_sim::tune::{mode, DT};

use crate::maps::{self, MAPS};
use crate::Chart;
use crate::{channel, flag, t, Command, Kill, Mode, Outcome, Setting, State};

/// The pad as a device hands it over: one bit a button, whatever the device calls it.
pub mod pad {
    pub const UP: u32 = 1;
    pub const DOWN: u32 = 1 << 1;
    pub const LEFT: u32 = 1 << 2;
    pub const RIGHT: u32 = 1 << 3;
    /// The four face buttons by where they sit: top, right, bottom, left.
    pub const NORTH: u32 = 1 << 4;
    pub const EAST: u32 = 1 << 5;
    pub const SOUTH: u32 = 1 << 6;
    pub const WEST: u32 = 1 << 7;
    pub const L: u32 = 1 << 8;
    pub const R: u32 = 1 << 9;
    pub const START: u32 = 1 << 10;
    pub const SELECT: u32 = 1 << 11;
}

/// The buttons held and the stick, -1…1, right and up positive.
#[derive(Clone, Copy, Default, Debug)]
pub struct Pad {
    pub held: u32,
    pub sx: f32,
    pub sy: f32,
}

/// How fast the look turns: what the setting calls it, and radians a second.
pub const LOOK: [(&str, f32); 3] = [("Slow", 1.5), ("Medium", 2.2), ("Fast", 3.0)];
/// How hard the soldiers' rounds hit: what the setting calls it, and the share of the reference's damage.
pub const SKILL: [(&str, f32); 3] = [("Recruit", 0.45), ("Regular", 0.7), ("Veteran", 1.0)];
pub const LAYOUT: [&str; 2] = ["Buttons look", "Stick looks"];
/// What the feed calls the soldiers, by variant.
pub const NAMES: [&str; 3] = ["VANGUARD", "IRREGULAR", "BREACHER"];

pub struct Session {
    pub mode: Mode,
    /// The interface has nothing scheduled: see [`Pace`].
    pub idle: bool,
    /// The statistics line is asked for.
    pub stats: bool,
    look: usize,
    invert: bool,
    assist: bool,
    layout: usize,
    skill: usize,
    listed: Option<(usize, bool, bool, usize, usize, bool)>,
    /// Seconds this flow has run, and what the interface asked to be woken for.
    seconds: f32,
    wakes: Vec<(u32, f32)>,
    feed: Vec<Kill>,
    lines: u32,
    banner: (u32, &'static str, &'static str),
    prev_held: u32,
    /// The soldier's buttons (`btn`) a touch key pressed since the last frame of the match: held
    /// through that frame's steps and let go after it, which the match reads as one press.
    tapped: u32,
    frames: u32,
    /// The world's x and z at the map picture's top left corner.
    pub map: (f32, f32),
    /// Pixels a unit of the view's tangent takes at the widest field of view: what turns the
    /// weapon's cone into the crosshair's gap.
    pub focal: f32,
    /// The id of the map the device holds, and the ids of the maps it has a pack for, in the list's
    /// order: the device sets both when it has read a pack. Before that, the first map alone.
    pub map_id: &'static str,
    pub maps: Vec<&'static str>,
    /// A map the interface asked for that the device does not hold: the device takes it, reads that
    /// map's pack, sets `map_id` and calls [`Session::play`].
    pub wanted: Option<&'static str>,
    /// The held map from above, when the device read one beside the map's pack ([`Chart`]): the
    /// device sets it with `map_id`.
    pub chart: Option<Chart>,
}

/// What asking for a match on a map comes to: nothing, a new match on the map that is held, or
/// another map's pack to read first.
#[derive(Debug, PartialEq)]
enum Asked {
    Nothing,
    Again,
    Another(&'static str),
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    pub fn new() -> Session {
        Session { mode: Mode::Title, idle: false, stats: false, look: 1, invert: false, assist: true, layout: 0, skill: 1, listed: None, seconds: 0.0, wakes: Vec::new(), feed: Vec::new(), lines: 0, banner: (0, "", ""), prev_held: u32::MAX, tapped: 0, frames: 0, map: (0.0, 0.0), focal: 213.0, map_id: MAPS[0].id, maps: alloc::vec![MAPS[0].id], wanted: None, chart: None }
    }

    /// What a row of the map list comes to. A list is up (no match runs under a map that goes), and
    /// the device has the map's pack.
    fn asked(&self, id: &str) -> Asked {
        match maps::named(id).filter(|map| self.maps.contains(&map.id)) {
            Some(map) if matches!(self.mode, Mode::Title | Mode::Menu | Mode::Results) => {
                if map.id == self.map_id {
                    Asked::Again
                } else {
                    Asked::Another(map.id)
                }
            }
            _ => Asked::Nothing,
        }
    }

    /// Hands the settings to a match.
    pub fn apply(&self, game: &mut Game) {
        game.player.turn = LOOK[self.look].1;
        game.player.invert = self.invert;
        game.player.analog_look = self.layout == 1;
        game.assist = self.assist;
        game.skill = SKILL[self.skill].1;
    }

    /// A match begins on the map the device now holds: what a device calls when it has read the pack
    /// of the map that was asked for.
    pub fn play(&mut self, game: &mut Game) {
        self.fresh(game);
    }

    fn fresh(&mut self, game: &mut Game) {
        game.restart();
        self.apply(game);
        self.feed.clear();
        self.banner = (self.banner.0, "", "");
        self.mode = Mode::Play;
    }

    /// Notes a touch key's tap for the match's next frame.
    fn tap(&mut self, key: &str) {
        if self.mode == Mode::Play {
            self.tapped |= match key {
                "weapon" => btn::NEXT,
                "mode" => btn::MODE,
                "reload" => btn::RELOAD,
                _ => 0,
            };
        }
    }

    /// Does what the interface asked. A command that is the device's own comes back.
    pub fn command(&mut self, game: &mut Game, command: Command) -> Option<Command> {
        match command {
            Command::Start => {
                if self.mode == Mode::Title {
                    self.fresh(game);
                }
            }
            Command::Menu(on) => match (self.mode, on) {
                (Mode::Play | Mode::Down, true) => self.mode = Mode::Menu,
                (Mode::Menu, false) => self.mode = Mode::Play,
                _ => {}
            },
            Command::Restart => self.fresh(game),
            Command::Title => self.mode = Mode::Title,
            Command::Option { key, value } => {
                match key.as_str() {
                    "look" => self.look = (value as usize).min(LOOK.len() - 1),
                    "invert" => self.invert = value != 0,
                    "assist" => self.assist = value != 0,
                    "layout" => self.layout = (value as usize).min(LAYOUT.len() - 1),
                    "skill" => self.skill = (value as usize).min(SKILL.len() - 1),
                    "stats" => self.stats = value != 0,
                    _ => return Some(Command::Option { key, value }),
                }
                self.apply(game);
            }
            Command::Idle(on) => self.idle = on,
            Command::Wake { id, seconds } => {
                if self.wakes.len() < 16 {
                    self.wakes.push((id, self.seconds + max(seconds, 0.0)));
                }
            }
            // A tap during play is one press of the pad's button for the same thing; anywhere else
            // it is nothing, so a match does not resume with a press waiting.
            Command::Tap(key) => self.tap(&key),
            // A match on a map: at once on the one the device holds, and on another once the device
            // has read it. A map the device has no pack for is not asked of it.
            Command::Map(id) => match self.asked(&id) {
                Asked::Again => self.fresh(game),
                Asked::Another(map) => self.wanted = Some(map),
                Asked::Nothing => {}
            },
            other => return Some(other),
        }
        None
    }

    /// Does everything the interface asked since the last frame. The return is the text the
    /// interface wants kept, when it said so.
    pub fn obey(&mut self, game: &mut Game) -> Option<String> {
        let mut kept = None;
        while let Some(command) = unsafe { channel() }.next() {
            if let Some(Command::Prefs(text)) = self.command(game, command) {
                kept = Some(text);
            }
        }
        kept
    }

    /// The pad as the match reads it, by the layout the person chose.
    fn input(&self, pad: &Pad) -> Input {
        let held = |bit: u32| pad.held & bit != 0;
        let axis = |plus: u32, minus: u32| (held(plus) as i32 - held(minus) as i32) as f32;
        let faces = (axis(pad::EAST, pad::WEST), axis(pad::NORTH, pad::SOUTH));
        let ((mx, my), (lx, ly)) = if self.layout == 0 { ((pad.sx, pad.sy), faces) } else { (faces, (pad.sx, pad.sy)) };
        let mut buttons = 0;
        for (from, to) in [(pad::R, btn::FIRE), (pad::L, btn::ADS), (pad::UP, btn::JUMP), (pad::DOWN, btn::CROUCH), (pad::LEFT, btn::RELOAD), (pad::RIGHT, btn::NEXT), (pad::SELECT, btn::MODE)] {
            if held(from) {
                buttons |= to;
            }
        }
        Input { mx, my, lx, ly, buttons }
    }

    fn note(&mut self, game: &Game, event: &Event) {
        let mut line = |feed: &mut Vec<Kill>, lines: &mut u32, by, whom, head, mine| {
            *lines += 1;
            feed.insert(0, Kill { id: *lines, by, whom, head, mine });
            feed.truncate(4);
        };
        match *event {
            Event::Struck { soldier, zone, killed: true, .. } => {
                let head = zone == 0;
                line(&mut self.feed, &mut self.lines, "YOU", NAMES[game.soldiers[soldier as usize].variant as usize % 3], head, 1);
                self.banner = (self.banner.0 + 1, "ENEMY DOWN", if head { "HEADSHOT +150" } else { "+100" });
            }
            Event::Down { soldier } => line(&mut self.feed, &mut self.lines, soldier.map_or("GRENADE", |s| NAMES[game.soldiers[s as usize].variant as usize % 3]), "YOU", false, 2),
            _ => {}
        }
    }

    /// One frame: `ticks` steps of the match while it has the screen. `each` is told every event
    /// of every step, with the match as it stood after that step.
    pub fn run(&mut self, game: &mut Game, pad: &Pad, ticks: u32, mut each: impl FnMut(&Game, &Event)) {
        let pressed = pad.held & !self.prev_held;
        self.prev_held = pad.held;
        self.frames = self.frames.wrapping_add(1);
        self.seconds += ticks as f32 * DT;
        // No interface on the screen: the match starts by itself, and START stops and starts it.
        if !unsafe { channel() }.is_open() && self.frames > 30 {
            match self.mode {
                Mode::Title | Mode::Loading => self.fresh(game),
                Mode::Play | Mode::Down if pressed & pad::START != 0 => self.mode = Mode::Menu,
                Mode::Menu if pressed & pad::START != 0 => self.mode = Mode::Play,
                Mode::Results if pressed & pad::START != 0 => self.fresh(game),
                _ => {}
            }
        }
        if !matches!(self.mode, Mode::Play | Mode::Down) {
            return;
        }
        let mut input = self.input(pad);
        input.buttons |= core::mem::take(&mut self.tapped);
        for _ in 0..ticks {
            game.step(&input);
            for i in 0..duty_sim::event::Events::ROOM {
                let Some(event) = game.events.iter().nth(i).copied() else { break };
                self.note(game, &event);
                each(game, &event);
            }
        }
        self.mode = match game.phase {
            Phase::Play => Mode::Play,
            Phase::Down { .. } => Mode::Down,
            Phase::Over { .. } => Mode::Results,
        };
    }

    /// Writes what the interface is shown.
    pub fn publish(&mut self, game: &Game, state: &mut State) {
        state.mode = self.mode;
        state.map = self.map_id;
        if state.maps != self.maps {
            state.maps.clone_from(&self.maps);
        }
        if state.chart != self.chart {
            state.chart.clone_from(&self.chart);
        }
        let listed = (self.look, self.invert, self.assist, self.layout, self.skill, self.stats);
        if self.listed != Some(listed) {
            self.listed = Some(listed);
            state.options.clear();
            state.options.push(Setting::choice("look", self.look, &[LOOK[0].0, LOOK[1].0, LOOK[2].0]));
            state.options.push(Setting::switch("invert", self.invert));
            state.options.push(Setting::switch("assist", self.assist));
            state.options.push(Setting::choice("layout", self.layout, &LAYOUT));
            state.options.push(Setting::choice("skill", self.skill, &[SKILL[0].0, SKILL[1].0, SKILL[2].0]));
            state.options.push(Setting::switch("stats", self.stats));
        }
        // One wake a frame: each is a line of its own, so the interface hears every id.
        if let Some(at) = self.wakes.iter().position(|w| w.1 <= self.seconds) {
            state.woke = self.wakes.swap_remove(at).0;
        }
        if state.feed != self.feed {
            state.feed.clone_from(&self.feed);
        }
        state.banner = self.banner;
        state.result = match game.phase {
            Phase::Over { won } if self.mode == Mode::Results => Some(Outcome {
                won,
                us: game.score[0],
                them: game.score[1],
                kills: game.tally.kills,
                deaths: game.tally.deaths,
                headshots: game.tally.headshots,
                accuracy: game.tally.hits * 100 / game.tally.shots.max(1),
                seconds: game.time as u32,
            }),
            _ => None,
        };
        let def = game.arms.def();
        let label = core::str::from_utf8(&def.label).unwrap_or("").trim_end_matches('\0');
        if state.weapon != label {
            state.weapon.clear();
            state.weapon.push_str(label);
        }
        state.fire = mode::NAMES[game.arms.fire_mode() as usize % 3];
        // The numbers every other frame: each change of them is a turn of the guest.
        if self.frames % 2 != 0 && self.mode == Mode::Play {
            return;
        }
        let whole = |x: f32| floor(x + 0.5) as i32;
        let n = &mut state.t;
        let p = &game.player;
        n[t::HEALTH] = whole(p.health);
        n[t::AMMO] = game.arms.magazine[game.arms.active] as i32;
        n[t::RESERVE] = game.arms.reserve[game.arms.active] as i32;
        // Clockwise from north, in steps of two degrees: a still hand changes nothing.
        n[t::HEADING] = (whole(-p.yaw * (90.0 / PI)) * 2).rem_euclid(360);
        n[t::X] = whole((p.body.pos.x - self.map.0) * 2.0) * 5;
        n[t::Z] = whole((p.body.pos.z - self.map.1) * 2.0) * 5;
        n[t::SPREAD] = if game.arms.ads > 0.5 || p.dead { 0 } else { whole(tan(game.arms.spread * (PI / 180.0)) * self.focal).clamp(2, 40) };
        n[t::US] = game.score[0] as i32;
        n[t::THEM] = game.score[1] as i32;
        n[t::CLOCK] = whole(game.clock + 0.49);
        n[t::HIT] = game.hit_mark as i32;
        n[t::HURT] = game.hurt_mark as i32;
        n[t::RELOAD] = game.arms.reloading().map_or(0, |r| whole(r * 20.0).clamp(1, 20) * 5);
        n[t::FLAGS] = (if game.arms.ads > 0.5 { flag::ADS } else { 0 })
            | (if p.sprinting { flag::SPRINT } else { 0 })
            | (if p.crouched { flag::CROUCH } else { 0 })
            | (if game.arms.reloading().is_some() { flag::RELOADING } else { 0 })
            | (if game.arms.magazine[game.arms.active] == 0 { flag::EMPTY } else { 0 });
        n[t::RESPAWN] = match game.phase {
            Phase::Down { wait } => whole(wait * 10.0).max(0),
            _ => 0,
        };
        // The soldiers the map shows: the ones who fired a moment ago.
        let mut blips = 0;
        for s in game.soldiers.iter().filter(|s| s.alive() && s.shown > 0.0).take(t::MAX_BLIPS) {
            n[t::BLIP + blips * 2] = whole((s.pos.x - self.map.0) * 2.0) * 5;
            n[t::BLIP + blips * 2 + 1] = whole((s.pos.z - self.map.1) * 2.0) * 5;
            blips += 1;
        }
        n[t::BLIPS] = blips as i32;
    }
}

/// PocketJS's START: the one button the interface listens to during play (it opens the menu).
const GUEST_MENU: u32 = 0x0008;
/// Turns the guest still takes after the last reason for one: what a press or a line started
/// (a list sliding in, a mark fading) plays out.
const LINGER: u8 = 12;

/// Which of the turns a device offers the guest are worth their cost. A turn runs the whole
/// framework's frame, milliseconds on the slower machines however little changed.
#[derive(Default)]
pub struct Pace {
    buttons: u32,
    linger: u8,
    /// Turns offered since the last one taken for readouts alone.
    waited: u8,
    /// Turns taken so far, by why: the guest is not idle; a button it listens to moved; news
    /// that is more than readouts; the turns after any of those; readouts.
    pub taken: [u32; 5],
}

impl Pace {
    pub const fn new() -> Pace {
        Pace { buttons: 0, linger: 0, waited: 0, taken: [0; 5] }
    }

    /// `buttons`: the guest's pad (PocketJS's bits) as it would be handed over this turn.
    /// `every`: of the turns that would bring the guest nothing but new readouts (the numbers of
    /// play), one in this many is taken. A PSP's turn is a quarter of its frame, and a compass
    /// that follows the view at 15 steps a second reads the same as one at 30.
    pub fn due(&mut self, session: &Session, buttons: u32, every: u8) -> bool {
        let heard = match session.mode {
            Mode::Play => buttons & GUEST_MENU,
            Mode::Down => 0,
            _ => buttons,
        };
        let changed = heard != self.buttons;
        self.buttons = heard;
        let interface = unsafe { channel() };
        let news = interface.news();
        if !session.idle || changed || (news && !interface.only_readouts()) {
            self.taken[if !session.idle { 0 } else if changed { 1 } else { 2 }] += 1;
            self.linger = LINGER;
            self.waited = 0;
            return true;
        }
        if self.linger > 0 {
            // What settles after news (a fade, a slide) is drawn at the readouts' pace too.
            self.linger -= 1;
            self.waited += 1;
            if self.waited >= every.max(1).min(2) || self.linger == 0 {
                self.waited = 0;
                self.taken[3] += 1;
                return true;
            }
            return false;
        }
        if !news {
            return false;
        }
        self.waited += 1;
        if self.waited >= every.max(1) {
            self.waited = 0;
            self.taken[4] += 1;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tapped_key_is_the_soldiers_button_for_one_frame_of_play() {
        let mut session = Session::new();
        // On the title a tap is nothing.
        session.tap("weapon");
        assert_eq!(session.tapped, 0);
        session.mode = Mode::Play;
        session.tap("weapon");
        session.tap("reload");
        session.tap("grenade");
        assert_eq!(session.tapped, btn::NEXT | btn::RELOAD);
        // The frame takes them, as `run` does before its steps, and the next has none.
        assert_eq!(core::mem::take(&mut session.tapped), btn::NEXT | btn::RELOAD);
        session.tap("mode");
        assert_eq!(core::mem::take(&mut session.tapped), btn::MODE);
        assert_eq!(session.tapped, 0);
    }

    #[test]
    fn a_map_is_asked_for_from_a_list_and_only_when_the_device_has_its_pack() {
        let (first, other) = (MAPS[0].id, MAPS[1].id);
        let mut session = Session::new();
        assert_eq!((session.map_id, session.maps.as_slice()), (first, &[first][..]));
        // The device has one pack: the other map is not offered, and asking for it is nothing.
        assert_eq!(session.asked(other), Asked::Nothing);
        assert_eq!(session.asked(first), Asked::Again);
        session.maps.push(other);
        assert_eq!(session.asked(other), Asked::Another(other));
        assert_eq!(session.asked("no-such-map"), Asked::Nothing);
        // The map that is held needs no reading, from the menu and the results as from the title.
        for mode in [Mode::Menu, Mode::Results] {
            session.mode = mode;
            assert_eq!((session.asked(first), session.asked(other)), (Asked::Again, Asked::Another(other)));
        }
        // No list is up: nothing takes the map from under a match, or from under a read.
        for mode in [Mode::Play, Mode::Down, Mode::Loading, Mode::Error] {
            session.mode = mode;
            assert_eq!((session.asked(first), session.asked(other)), (Asked::Nothing, Asked::Nothing));
        }
        // On the other map, the first is the one to read.
        session.mode = Mode::Title;
        session.map_id = other;
        assert_eq!((session.asked(first), session.asked(other)), (Asked::Another(first), Asked::Again));
    }
}
