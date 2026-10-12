//! The renderer's side of `ui/app/protocol.ts`.
//!
//! The interface is one PocketJS guest (`ui/`) drawn over the scene. It owns
//! every 2D pixel, and what a button means while a list is up. The renderer
//! keeps a [`State`] and the guest is sent the members that changed, as one
//! JSON line a turn; what the guest asks for comes back as [`Command`]s. The
//! lines travel over the guest's service channel, PocketJS's
//! `pocket.overlay`, answered in the process (`guest`) instead of on a wire.
//!
//! [`session::Session`] is the flow around a match (the title, the match, the
//! menu over it, the results), written once for every device.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;

#[cfg(feature = "guest")]
pub mod guest;
pub mod session;

pub use session::{pad, Pace, Pad, Session};

/// What the screen is for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// The pack is being read; `State::message` names the step.
    #[default]
    Loading,
    /// The street behind the title.
    Title,
    /// The match has the screen, and the pad is the soldier's.
    Play,
    /// The player was killed and waits to be put back.
    Down,
    /// A list is up over a stopped match; the pad is the interface's.
    Menu,
    /// The match is over.
    Results,
    /// `State::message` says what failed.
    Error,
}

impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Mode::Loading => "loading",
            Mode::Title => "title",
            Mode::Play => "play",
            Mode::Down => "down",
            Mode::Menu => "menu",
            Mode::Results => "results",
            Mode::Error => "error",
        }
    }

    pub fn parse(name: &str) -> Option<Mode> {
        [Mode::Loading, Mode::Title, Mode::Play, Mode::Down, Mode::Menu, Mode::Results, Mode::Error].into_iter().find(|m| m.name() == name)
    }
}

/// Something the person can set: a switch (0 or 1), or with `choices` one of several named values.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Setting {
    pub key: &'static str,
    pub value: u32,
    pub choices: Vec<&'static str>,
}

impl Setting {
    pub fn switch(key: &'static str, on: bool) -> Self {
        Self { key, value: on as u32, choices: Vec::new() }
    }

    pub fn choice(key: &'static str, value: usize, choices: &[&'static str]) -> Self {
        Self { key, value: value as u32, choices: choices.to_vec() }
    }
}

/// One line of the kill feed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Kill {
    pub id: u32,
    pub by: &'static str,
    pub whom: &'static str,
    pub head: bool,
    /// The player is `by` (1), is `whom` (2), or neither (0).
    pub mine: u8,
}

/// How a match ended.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Outcome {
    pub won: bool,
    pub us: u32,
    pub them: u32,
    pub kills: u32,
    pub deaths: u32,
    pub headshots: u32,
    pub accuracy: u32,
    pub seconds: u32,
}

/// Indices into [`State::t`], as `ui/app/protocol.ts` names them.
pub mod t {
    pub const HEALTH: usize = 0;
    pub const AMMO: usize = 1;
    pub const RESERVE: usize = 2;
    pub const HEADING: usize = 3;
    pub const X: usize = 4;
    pub const Z: usize = 5;
    pub const SPREAD: usize = 6;
    pub const US: usize = 7;
    pub const THEM: usize = 8;
    pub const CLOCK: usize = 9;
    pub const HIT: usize = 10;
    pub const HURT: usize = 11;
    pub const RELOAD: usize = 12;
    pub const FLAGS: usize = 13;
    pub const RESPAWN: usize = 14;
    pub const BLIPS: usize = 15;
    pub const BLIP: usize = 16;
    /// Blips the map shows at most.
    pub const MAX_BLIPS: usize = 6;
    pub const COUNT: usize = BLIP + MAX_BLIPS * 2;
}

pub mod flag {
    pub const ADS: i32 = 1;
    pub const SPRINT: i32 = 2;
    pub const CROUCH: i32 = 4;
    pub const RELOADING: i32 = 8;
    pub const EMPTY: i32 = 16;
}

/// What the interface is shown.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub mode: Mode,
    /// The loading step, or why `mode` is `Error`.
    pub message: String,
    /// What the device lets the person set, in menu order.
    pub options: Vec<Setting>,
    /// One line of renderer statistics while the `stats` setting is on.
    pub stats: String,
    /// What the interface last stored with `prefs`.
    pub prefs: String,
    /// The `wake` the interface asked for that last came due; 0 before any has.
    pub woke: u32,
    /// The weapon in hand and its fire mode.
    pub weapon: String,
    pub fire: &'static str,
    /// The newest lines of the kill feed, newest first.
    pub feed: Vec<Kill>,
    /// A line across the upper screen: its number (0 none), its title, what stands under it.
    pub banner: (u32, &'static str, &'static str),
    pub result: Option<Outcome>,
    pub t: [i32; t::COUNT],
}

fn escape(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c < ' ' => out.push(' '),
            c => out.push(c),
        }
    }
    out.push('"');
}

impl State {
    /// The line the guest receives: every member, or with `since` the members that differ from it.
    /// `None` when nothing does.
    pub fn line(&self, since: Option<&State>) -> Option<String> {
        let mut s = String::with_capacity(320);
        s.push_str("{\"type\":\"state\",\"value\":{");
        let start = s.len();
        macro_rules! member {
            ($field:ident, $key:literal, $write:expr) => {
                if since.map_or(true, |old| old.$field != self.$field) {
                    if s.len() > start {
                        s.push(',');
                    }
                    s.push_str(concat!("\"", $key, "\":"));
                    #[allow(clippy::redundant_closure_call)]
                    ($write)(&mut s);
                }
            };
        }
        member!(mode, "mode", |s: &mut String| escape(s, self.mode.name()));
        member!(message, "message", |s: &mut String| escape(s, &self.message));
        member!(options, "options", |s: &mut String| {
            s.push('[');
            for (i, o) in self.options.iter().enumerate() {
                let _ = write!(s, "{}{{\"key\":\"{}\",\"value\":{}", if i > 0 { "," } else { "" }, o.key, o.value);
                if !o.choices.is_empty() {
                    s.push_str(",\"choices\":[");
                    for (j, c) in o.choices.iter().enumerate() {
                        if j > 0 {
                            s.push(',');
                        }
                        escape(s, c);
                    }
                    s.push(']');
                }
                s.push('}');
            }
            s.push(']');
        });
        member!(stats, "stats", |s: &mut String| escape(s, &self.stats));
        member!(prefs, "prefs", |s: &mut String| escape(s, &self.prefs));
        member!(woke, "woke", |s: &mut String| {
            let _ = write!(s, "{}", self.woke);
        });
        member!(weapon, "weapon", |s: &mut String| escape(s, &self.weapon));
        member!(fire, "fire", |s: &mut String| escape(s, self.fire));
        member!(feed, "feed", |s: &mut String| {
            s.push('[');
            for (i, k) in self.feed.iter().enumerate() {
                let _ = write!(s, "{}{{\"id\":{},\"by\":\"{}\",\"whom\":\"{}\",\"head\":{},\"mine\":{}}}", if i > 0 { "," } else { "" }, k.id, k.by, k.whom, k.head, k.mine);
            }
            s.push(']');
        });
        member!(banner, "banner", |s: &mut String| {
            let _ = write!(s, "[{},\"{}\",\"{}\"]", self.banner.0, self.banner.1, self.banner.2);
        });
        member!(result, "result", |s: &mut String| match &self.result {
            None => s.push_str("null"),
            Some(r) => {
                let _ = write!(s, "{{\"won\":{},\"us\":{},\"them\":{},\"kills\":{},\"deaths\":{},\"headshots\":{},\"accuracy\":{},\"seconds\":{}}}", r.won, r.us, r.them, r.kills, r.deaths, r.headshots, r.accuracy, r.seconds);
            }
        });
        member!(t, "t", |s: &mut String| {
            s.push('[');
            for (i, v) in self.t.iter().enumerate() {
                let _ = write!(s, "{}{}", if i > 0 { "," } else { "" }, v);
            }
            s.push(']');
        });
        if s.len() == start {
            return None;
        }
        s.push_str("}}\n");
        Some(s)
    }
}

/// What the interface asks of the renderer.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Leave the title: a match starts.
    Start,
    /// A list comes up over the match, or leaves it.
    Menu(bool),
    /// A new match on the same ground.
    Restart,
    /// Back to the title.
    Title,
    Option { key: String, value: u32 },
    /// To store, and to hand back in [`State::prefs`].
    Prefs(String),
    /// The interface has nothing scheduled.
    Idle(bool),
    /// Say `id` back in [`State::woke`] in this many seconds.
    Wake { id: u32, seconds: f32 },
}

/// The value after `"key":` in a flat JSON object. Quoted text is skipped when looking for the
/// key, so a value cannot pose as one.
fn field<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let bytes = json.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'"' {
            i += 1;
            continue;
        }
        let begin = i + 1;
        let mut end = begin;
        while end < bytes.len() && bytes[end] != b'"' {
            end += if bytes[end] == b'\\' { 2 } else { 1 };
        }
        if end >= bytes.len() {
            return None;
        }
        let rest = json[end + 1..].trim_start();
        if rest.starts_with(':') && &json[begin..end] == key {
            return Some(rest[1..].trim_start());
        }
        i = end + 1;
    }
    None
}

/// A decimal number at the start of `text`.
pub fn parse_f32(text: &str) -> Option<f32> {
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let (mut value, mut scale, mut seen, mut point) = (0.0f32, 1.0f32, false, false);
    for c in digits.chars() {
        match c {
            '0'..='9' => {
                seen = true;
                if point {
                    scale *= 0.1;
                    value += (c as u32 - '0' as u32) as f32 * scale;
                } else {
                    value = value * 10.0 + (c as u32 - '0' as u32) as f32;
                }
            }
            '.' if !point => point = true,
            _ => break,
        }
    }
    seen.then_some(if negative { -value } else { value })
}

fn number(json: &str, key: &str) -> f32 {
    field(json, key).and_then(parse_f32).unwrap_or(0.0)
}

fn text(json: &str, key: &str) -> Option<String> {
    let value = field(json, key)?.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = value.chars();
    loop {
        match chars.next()? {
            '"' => return Some(out),
            '\\' => out.push(chars.next()?),
            c => out.push(c),
        }
    }
}

fn flag(json: &str, key: &str) -> bool {
    field(json, key).is_some_and(|v| v.starts_with("true"))
}

impl Command {
    pub fn parse(line: &str) -> Option<Command> {
        let n = |key| number(line, key);
        Some(match text(line, "type")?.as_str() {
            "start" => Command::Start,
            "menu" => Command::Menu(flag(line, "on")),
            "restart" => Command::Restart,
            "title" => Command::Title,
            "option" => Command::Option { key: text(line, "key")?, value: n("value").max(0.0) as u32 },
            "prefs" => Command::Prefs(text(line, "value")?),
            "idle" => Command::Idle(flag(line, "on")),
            "wake" => Command::Wake { id: n("id").max(0.0) as u32, seconds: n("seconds") },
            _ => return None,
        })
    }
}

/// The channel's two ends: the state last sent, and what the guest said.
#[derive(Default)]
pub struct Interface {
    pub state: State,
    sent: Option<State>,
    open: bool,
    inbox: VecDeque<String>,
}

impl Interface {
    /// The guest opened its service: only the overlay is here.
    pub fn open(&mut self, service: &str) -> bool {
        self.open = service == "pocket.overlay";
        self.sent = None; // a fresh guest gets the whole state
        self.open
    }

    /// A guest holds the channel: the interface is on the screen.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The guest is gone (it failed, or is being replaced).
    pub fn close(&mut self) {
        self.open = false;
        self.inbox.clear();
    }

    /// The state line, when the guest has not seen this state.
    pub fn poll(&mut self) -> Option<String> {
        if !self.open {
            return None;
        }
        let line = self.state.line(self.sent.as_ref())?;
        self.sent = Some(self.state.clone());
        Some(line)
    }

    /// The guest has not seen the state as it now is.
    pub fn news(&self) -> bool {
        self.open && self.sent.as_ref() != Some(&self.state)
    }

    /// What the guest has not seen is at most readouts (the numbers of play, the statistics
    /// line): text swapped in place, with nothing set moving.
    pub fn only_readouts(&self) -> bool {
        let Some(sent) = &self.sent else { return false };
        let State { mode, message, options, prefs, woke, weapon, fire, feed, banner, result, stats: _, t: _ } = &self.state;
        *mode == sent.mode && *message == sent.message && *options == sent.options && *prefs == sent.prefs && *woke == sent.woke && *weapon == sent.weapon && *fire == sent.fire && *feed == sent.feed && *banner == sent.banner && *result == sent.result
    }

    /// A line from the guest.
    pub fn receive(&mut self, line: &str) {
        if self.inbox.len() < 32 {
            self.inbox.push_back(String::from(line));
        }
    }

    /// The next command the guest sent, oldest first.
    pub fn next(&mut self) -> Option<Command> {
        while let Some(line) = self.inbox.pop_front() {
            if let Some(command) = Command::parse(&line) {
                return Some(command);
            }
        }
        None
    }
}

static mut INTERFACE: Option<Interface> = None;

/// The one channel, on the thread that runs the guest and the match.
///
/// # Safety
/// Call only on that thread; do not keep the reference across a guest turn.
pub unsafe fn channel() -> &'static mut Interface {
    (*core::ptr::addr_of_mut!(INTERFACE)).get_or_insert_with(Interface::default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn commands_from_the_interface() {
        let parse = |line| Command::parse(line).unwrap();
        assert_eq!(parse(r#"{"type":"start"}"#), Command::Start);
        assert_eq!(parse(r#"{"type":"menu","on":true}"#), Command::Menu(true));
        assert_eq!(parse(r#"{"type":"restart"}"#), Command::Restart);
        assert_eq!(parse(r#"{"type":"title"}"#), Command::Title);
        assert_eq!(parse(r#"{"type":"option","key":"look","value":2}"#), Command::Option { key: "look".into(), value: 2 });
        assert_eq!(parse(r#"{"type":"prefs","value":"{\"look\":1}"}"#), Command::Prefs(r#"{"look":1}"#.into()));
        assert_eq!(parse(r#"{"type":"idle","on":true}"#), Command::Idle(true));
        assert_eq!(parse(r#"{"type":"wake","id":4,"seconds":2.5}"#), Command::Wake { id: 4, seconds: 2.5 });
        assert_eq!(Command::parse(r#"{"type":"pocket.overlay.control","name":"x","node":3}"#), None);
    }

    #[test]
    fn the_guest_is_sent_what_changed() {
        let mut interface = Interface::default();
        assert_eq!(interface.poll(), None);
        assert!(interface.open("pocket.overlay"));
        interface.state.mode = Mode::Title;
        interface.state.options = vec![Setting::switch("invert", false), Setting::choice("look", 1, &["Slow", "Medium", "Fast"])];
        let first = interface.poll().unwrap();
        assert!(first.starts_with("{\"type\":\"state\",\"value\":{\"mode\":\"title\"") && first.contains("\"options\":[{\"key\":\"invert\",\"value\":0},{\"key\":\"look\",\"value\":1,\"choices\":[\"Slow\",\"Medium\",\"Fast\"]}]") && first.contains("\"result\":null") && first.ends_with("}}\n"));
        assert_eq!(interface.poll(), None);
        // A turn of play: only the numbers go out.
        interface.state.t[t::HEALTH] = 64;
        assert!(interface.news() && interface.only_readouts());
        assert!(interface.poll().unwrap().starts_with("{\"type\":\"state\",\"value\":{\"t\":[64,0,"));
        interface.state.feed.push(Kill { id: 1, by: "YOU", whom: "VANGUARD", head: true, mine: 1 });
        interface.state.banner = (1, "ENEMY DOWN", "HEADSHOT +150");
        assert!(!interface.only_readouts());
        assert_eq!(interface.poll().unwrap(), "{\"type\":\"state\",\"value\":{\"feed\":[{\"id\":1,\"by\":\"YOU\",\"whom\":\"VANGUARD\",\"head\":true,\"mine\":1}],\"banner\":[1,\"ENEMY DOWN\",\"HEADSHOT +150\"]}}\n");
        interface.receive(r#"{"type":"menu","on":true}"#);
        interface.receive("not a command");
        assert_eq!(interface.next(), Some(Command::Menu(true)));
        assert_eq!(interface.next(), None);
    }
}
