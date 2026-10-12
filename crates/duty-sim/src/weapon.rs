//! The player's three weapons: the trigger, the magazine, the sights, the
//! kick. The rules are the reference's (`web/src/weapons/index.js`), with two
//! differences a fixed step makes: the time to the next round carries over
//! from step to step, so a weapon fires at its own rate at any frame rate,
//! and a round arrives at once (the reference flies it, and at this map's
//! distances it lands within two frames).

use crate::event::{Event, Events};
use crate::math::*;
use crate::player::{btn, Player};
use crate::rng::Rng;
use crate::tune::{mode, WeaponDef, DT};

pub const WEAPONS: usize = 3;

/// What plays on the held weapon instead of holding it still.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    None,
    /// A change of magazine, `empty` when the old one was: seconds in, and whether the rounds have moved.
    Reload {
        empty: bool,
        at: f32,
        moved: bool,
    },
    /// Lowering this weapon for another.
    Holster {
        to: usize,
        at: f32,
    },
    Draw {
        at: f32,
    },
}

/// A round that left the muzzle this step, for the match to carry to what it hits.
#[derive(Clone, Copy, Debug)]
pub struct Round {
    pub from: V3,
    pub dir: V3,
    pub weapon: usize,
    pub tracer: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Arms {
    pub defs: [WeaponDef; WEAPONS],
    pub magazine: [u32; WEAPONS],
    pub reserve: [u32; WEAPONS],
    /// Which of its fire modes each weapon is in.
    pub mode: [usize; WEAPONS],
    pub active: usize,
    pub action: Action,
    /// Seconds to the next round; below zero by what the last step left over.
    wait: f32,
    burst_left: u32,
    burst_wait: f32,
    /// Degrees of the cone the next round leaves in.
    pub spread: f32,
    shot: u32,
    since_shot: f32,
    /// How far the sights are up, 0…1, linear in time.
    pub ads: f32,
    /// The held weapon's own motion from a shot: how far back it is, how far up, and its roll;
    /// how far back the bolt is (1 on firing, falling to 0; held at 1 on an empty magazine).
    pub back: f32,
    back_speed: f32,
    pub roll: f32,
    pub bolt: f32,
    /// Steps the muzzle's flash still shows.
    pub flash: u32,
    pub fired: u32,
    held: u32,
}

impl Arms {
    pub fn new(defs: [WeaponDef; WEAPONS]) -> Arms {
        Arms {
            defs,
            magazine: defs.map(|d| d.magazine),
            reserve: defs.map(|d| d.reserve),
            mode: [0; WEAPONS],
            active: 0,
            action: Action::Draw { at: 0.0 },
            wait: 0.0,
            burst_left: 0,
            burst_wait: 0.0,
            spread: defs[0].spread_hip,
            shot: 0,
            since_shot: 10.0,
            ads: 0.0,
            back: 0.0,
            back_speed: 0.0,
            roll: 0.0,
            bolt: 0.0,
            flash: 0,
            fired: 0,
            held: 0,
        }
    }

    pub fn def(&self) -> &WeaponDef {
        &self.defs[self.active]
    }

    /// The fire mode in hand (`tune::mode`).
    pub fn fire_mode(&self) -> u32 {
        self.def().mode[self.mode[self.active]]
    }

    pub fn reloading(&self) -> Option<f32> {
        match self.action {
            Action::Reload { empty, at, .. } => Some(
                at / if empty {
                    self.def().reload_empty
                } else {
                    self.def().reload_tac
                },
            ),
            _ => None,
        }
    }

    fn reload(&mut self, events: &mut Events) {
        let d = *self.def();
        if self.action != Action::None && !matches!(self.action, Action::Draw { .. })
            || self.magazine[self.active] >= d.magazine
            || self.reserve[self.active] == 0
        {
            return;
        }
        self.action = Action::Reload {
            empty: self.magazine[self.active] == 0,
            at: 0.0,
            moved: false,
        };
        self.burst_left = 0;
        events.push(Event::Reload { step: 0 });
    }

    /// One step. Returns the round that left the muzzle, if one did.
    pub fn step(
        &mut self,
        player: &mut Player,
        buttons: u32,
        rng: &mut Rng,
        events: &mut Events,
    ) -> Option<Round> {
        let pressed = buttons & !self.held;
        self.held = buttons;
        let d = *self.def();
        if player.dead {
            self.ads = 0.0;
            player.ads = 0.0;
            return None;
        }
        // ---- the action in hand
        match &mut self.action {
            Action::None => {}
            Action::Reload { empty, at, moved } => {
                let (total, magin, magout) = if *empty {
                    (d.reload_empty, 0.71, 0.16)
                } else {
                    (d.reload_tac, 0.81, 0.20)
                };
                let before = *at;
                *at += DT;
                if before < magout * total && *at >= magout * total {
                    events.push(Event::Reload { step: 1 });
                }
                if !*moved && *at >= magin * total {
                    // The rounds arrive when the magazine seats; the trigger stays locked to the end.
                    *moved = true;
                    let take =
                        (d.magazine - self.magazine[self.active]).min(self.reserve[self.active]);
                    self.magazine[self.active] += take;
                    self.reserve[self.active] -= take;
                    events.push(Event::Reload { step: 2 });
                }
                if *at >= total * 0.995 {
                    self.action = Action::None;
                    self.shot = 0;
                    events.push(Event::Reload { step: 3 });
                }
            }
            Action::Holster { to, at } => {
                *at += DT;
                if *at >= d.holster_time * 0.995 {
                    self.active = *to;
                    self.action = Action::Draw { at: 0.0 };
                    self.shot = 0;
                    self.spread = self.defs[self.active].spread_hip;
                    self.burst_left = 0;
                    events.push(Event::Draw {
                        weapon: self.active as u8,
                    });
                }
            }
            Action::Draw { at } => {
                *at += DT;
                if *at >= d.draw_time {
                    self.action = Action::None;
                }
            }
        }
        let d = *self.def();
        let busy = matches!(self.action, Action::Reload { .. } | Action::Holster { .. });
        // ---- the sights
        let wants = buttons & btn::ADS != 0
            && !busy
            && !player.sprinting
            && player.climb.is_none()
            && player.sliding <= 0.0;
        self.ads = clamp(
            self.ads
                + if wants {
                    DT / d.ads_time
                } else {
                    -1.25 * DT / d.ads_time
                },
            0.0,
            1.0,
        );
        player.ads = self.ads;
        // ---- the cone
        let speed = player.speed();
        let mut rest = lerp(d.spread_hip, d.spread_ads, self.ads);
        if player.crouched {
            rest *= 0.78;
        }
        rest *= if speed < 0.4 {
            0.82
        } else if speed > 3.2 {
            1.15
        } else {
            1.0
        };
        if player.sprinting && self.since_shot > 0.3 {
            rest *= 2.2;
        }
        if !player.body.grounded {
            rest *= 2.0;
        }
        self.spread = max(rest, self.spread - d.spread_decay * DT * (1.0 + self.ads));
        self.since_shot += DT;
        if self.since_shot > 0.6 {
            self.shot = 0;
        }
        // ---- the held weapon's own motion: a spring pulls it forward again
        let w = TAU * 9.0;
        self.back_speed += (-w * w * self.back - 2.0 * 0.45 * w * self.back_speed) * DT;
        self.back += self.back_speed * DT;
        self.roll = ease(self.roll, 0.0, 14.0, DT);
        let empty = self.magazine[self.active] == 0;
        self.bolt = if empty && !busy {
            1.0
        } else {
            max(0.0, self.bolt - DT / max(0.045, 60.0 / d.rpm * 0.62))
        };
        self.flash = self.flash.saturating_sub(1);
        self.wait -= DT;
        self.burst_wait -= DT;

        // ---- the other buttons
        if pressed & btn::NEXT != 0 && !matches!(self.action, Action::Holster { .. }) {
            self.action = Action::Holster {
                to: (self.active + 1) % WEAPONS,
                at: 0.0,
            };
            return None;
        }
        if pressed & btn::MODE != 0 && d.modes > 1 {
            self.mode[self.active] = (self.mode[self.active] + 1) % d.modes as usize;
            self.burst_left = 0;
        }
        if pressed & btn::RELOAD != 0 || (pressed & btn::FIRE != 0 && empty) {
            self.reload(events);
        }
        // ---- the trigger
        if busy || matches!(self.action, Action::Reload { .. }) {
            self.wait = max(self.wait, 0.0);
            return None;
        }
        let fire = match self.fire_mode() {
            mode::AUTO => buttons & btn::FIRE != 0,
            mode::BURST => {
                if pressed & btn::FIRE != 0 && self.burst_left == 0 && self.burst_wait <= 0.0 {
                    self.burst_left = d.burst_count;
                }
                self.burst_left > 0
            }
            _ => pressed & btn::FIRE != 0,
        };
        if !fire || self.wait > 0.0 {
            // A trigger that waited keeps no credit: the next round leaves when it is pulled.
            if !fire {
                self.wait = max(self.wait, 0.0);
            }
            return None;
        }
        if empty {
            if pressed & btn::FIRE != 0 || self.fire_mode() != mode::SEMI {
                events.push(Event::Dry);
            }
            self.wait = 0.25;
            self.burst_left = 0;
            return None;
        }
        self.magazine[self.active] -= 1;
        self.fired += 1;
        let burst = self.fire_mode() == mode::BURST;
        self.wait = max(self.wait, -DT) + 60.0 / if burst { d.burst_rpm } else { d.rpm };
        if burst {
            self.burst_left -= 1;
            if self.burst_left == 0 {
                self.burst_wait = d.burst_delay;
            }
        }
        // The round: along the eye, anywhere in the cone.
        let (eye, yaw, pitch, _, _) = player.eye();
        let ahead = forward(yaw, pitch);
        let right = ahead.cross(V3::UP).norm_or(v3(1.0, 0.0, 0.0));
        let up = right.cross(ahead);
        let (radius, turn) = (
            sqrt(rng.float()) * tan(self.spread * (PI / 180.0)),
            rng.float() * TAU,
        );
        let dir = (ahead + right * (cos(turn) * radius) + up * (sin(turn) * radius)).norm();
        // The kick: the view climbs by the weapon's pattern, the weapon comes back in the hands.
        let p = d.pattern[(self.shot as usize).min(d.pattern_length.max(1) as usize - 1)];
        player.kick[0].add(p[0], 0.34);
        player.kick[1].add(p[1], 0.34);
        player.punch.add(d.punch * 0.012, 0.3);
        let first = self.since_shot > 0.35;
        self.back_speed +=
            d.kick_back * lerp(1.0, 0.54, self.ads) * if first { 1.18 } else { 1.0 } * w;
        self.roll +=
            d.kick_roll * (rng.float() * 0.4 + 0.6) * if rng.float() < 0.5 { -1.0 } else { 1.0 };
        self.spread = min(d.spread_max, self.spread + d.spread_shot);
        self.shot += 1;
        self.since_shot = 0.0;
        self.bolt = 1.0;
        self.flash = 3;
        Some(Round {
            from: eye,
            dir,
            weapon: self.active,
            tracer: self.fired % d.tracer_every.max(1) == 0,
        })
    }
}

/// The three weapons with the reference's numbers, for tests and for a pack that carries none.
pub fn reference() -> [WeaponDef; WEAPONS] {
    let label = |text: &str| {
        let mut out = [0u8; 12];
        out[..text.len()].copy_from_slice(text.as_bytes());
        out
    };
    let pattern = |pitch: f32, yaw: f32, length: usize| {
        let mut p = [[0.0f32; 2]; crate::tune::PATTERN];
        for (i, shot) in p.iter_mut().enumerate().take(length) {
            let climb = [1.45, 1.3, 1.15, 1.05, 1.0][i.min(4)];
            *shot = [pitch * climb, yaw * sin(i as f32 * 0.9) * 1.6];
        }
        p
    };
    [
        WeaponDef {
            label: label("M4A1"),
            mode: [mode::AUTO, mode::BURST, mode::SEMI],
            modes: 3,
            rpm: 800.0,
            burst_rpm: 950.0,
            burst_delay: 0.16,
            burst_count: 3,
            magazine: 30,
            reserve: 210,
            damage: 33.0,
            dropoff: 0.62,
            max_range: 420.0,
            penetration: 1.0,
            tracer_every: 3,
            spread_hip: 2.05,
            spread_ads: 0.24,
            spread_shot: 0.3,
            spread_max: 3.4,
            spread_decay: 3.6,
            kick_back: 0.019,
            kick_up: 0.0072,
            kick_roll: 0.032,
            punch: 0.35,
            ads_time: 0.22,
            reload_tac: 2.1,
            reload_empty: 2.9,
            draw_time: 0.62,
            holster_time: 0.4,
            pattern_length: 30,
            pattern: pattern(0.0085, 0.0022, 30),
        },
        WeaponDef {
            label: label("MPX-9"),
            mode: [mode::AUTO, mode::SEMI, 0],
            modes: 2,
            rpm: 950.0,
            burst_rpm: 1100.0,
            burst_delay: 0.14,
            burst_count: 2,
            magazine: 32,
            reserve: 224,
            damage: 24.0,
            dropoff: 0.48,
            max_range: 240.0,
            penetration: 0.45,
            tracer_every: 4,
            spread_hip: 2.5,
            spread_ads: 0.4,
            spread_shot: 0.26,
            spread_max: 3.9,
            spread_decay: 4.4,
            kick_back: 0.0135,
            kick_up: 0.0052,
            kick_roll: 0.026,
            punch: 0.24,
            ads_time: 0.185,
            reload_tac: 1.85,
            reload_empty: 2.5,
            draw_time: 0.52,
            holster_time: 0.34,
            pattern_length: 32,
            pattern: pattern(0.0058, 0.0026, 32),
        },
        WeaponDef {
            label: label("P-19"),
            mode: [mode::SEMI, 0, 0],
            modes: 1,
            rpm: 460.0,
            burst_rpm: 460.0,
            burst_delay: 0.1,
            burst_count: 1,
            magazine: 17,
            reserve: 68,
            damage: 28.0,
            dropoff: 0.42,
            max_range: 180.0,
            penetration: 0.35,
            tracer_every: 5,
            spread_hip: 3.1,
            spread_ads: 0.5,
            spread_shot: 0.42,
            spread_max: 4.6,
            spread_decay: 5.2,
            kick_back: 0.012,
            kick_up: 0.0105,
            kick_roll: 0.018,
            punch: 0.3,
            ads_time: 0.16,
            reload_tac: 1.6,
            reload_empty: 2.2,
            draw_time: 0.42,
            holster_time: 0.3,
            pattern_length: 17,
            pattern: pattern(0.0125, 0.0032, 17),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fire(arms: &mut Arms, player: &mut Player, buttons: u32, seconds: f32) -> u32 {
        let (mut rng, mut events, mut rounds) = (Rng::new(1), Events::default(), 0);
        for _ in 0..(seconds * 60.0) as u32 {
            rounds += arms.step(player, buttons, &mut rng, &mut events).is_some() as u32;
        }
        rounds
    }

    #[test]
    fn the_rifle_fires_at_its_own_rate_and_empties() {
        let mut player = Player::new(V3::ZERO, 0.0);
        let mut arms = Arms::new(reference());
        fire(&mut arms, &mut player, 0, 0.7);
        // 800 rounds a minute: 13 or 14 in a second, whatever the step.
        let rounds = fire(&mut arms, &mut player, btn::FIRE, 1.0);
        assert!((13..=14).contains(&rounds), "{rounds}");
        assert!(player.kick[0].value() > 0.0, "the view climbs");
        assert_eq!(fire(&mut arms, &mut player, btn::FIRE, 3.0) + rounds, 30);
        assert_eq!(arms.magazine[0], 0);
    }

    #[test]
    fn a_reload_fills_the_magazine_from_the_reserve() {
        let mut player = Player::new(V3::ZERO, 0.0);
        let mut arms = Arms::new(reference());
        fire(&mut arms, &mut player, 0, 0.7);
        fire(&mut arms, &mut player, btn::FIRE, 0.5);
        let left = arms.magazine[0];
        assert!(left < 30);
        fire(&mut arms, &mut player, btn::RELOAD, 1.0);
        assert!(
            arms.reloading().is_some() && arms.magazine[0] == left,
            "the rounds have not moved yet"
        );
        fire(&mut arms, &mut player, 0, 1.2);
        assert!(
            arms.reloading().is_none()
                && arms.magazine[0] == 30
                && arms.reserve[0] == 210 - (30 - left)
        );
    }

    #[test]
    fn the_next_weapon_comes_up_after_this_one_is_lowered() {
        let mut player = Player::new(V3::ZERO, 0.0);
        let mut arms = Arms::new(reference());
        fire(&mut arms, &mut player, 0, 0.7);
        fire(&mut arms, &mut player, btn::NEXT, 0.1);
        assert_eq!(arms.active, 0);
        fire(&mut arms, &mut player, 0, 0.5);
        assert_eq!(arms.active, 1);
        // The pistol fires once a press.
        fire(&mut arms, &mut player, btn::NEXT, 0.1);
        fire(&mut arms, &mut player, 0, 1.0);
        assert_eq!(arms.active, 2);
        assert_eq!(fire(&mut arms, &mut player, btn::FIRE, 1.0), 1);
    }
}
