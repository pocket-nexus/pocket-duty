//! The soldiers.
//!
//! What a soldier does is the reference's (`web/src/ai/agent.js`,
//! `squad.js`, `nav.js`): he stands or walks his round until he sees or hears
//! the player; then he takes the cover that stands best between him and the
//! player, leans out of it when his squad lets him, fires in bursts, changes
//! his magazine, flanks, falls back when he is hurt, and throws his one
//! grenade. The numbers are in `tune::soldier`.
//!
//! Where the reference asks the physics system every frame (a sight ray, the
//! foot rays, three rays to lean out), a soldier here looks every fourth step
//! and walks the grid's own floor: ten soldiers cost a few hundred
//! microseconds of a step on the handheld.

use crate::bvh::nearest_on_segments;
use crate::event::{Event, Events};
use crate::level::Level;
use crate::math::*;
use crate::nav::{Path, Pathfinder};
use crate::rng::Rng;
use crate::tune::soldier::*;
use crate::tune::{DT, GRAVITY};
use duty_pack::figure::clip;

/// Soldiers on the ground at once at most.
pub const MAX: usize = 10;
pub const SQUADS: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Idle,
    Patrol,
    Alert,
    Combat,
    Suppressed,
    Flank,
    Retreat,
    Dead,
}

/// Seconds without sight or sound of the player before a patrolling squad goes to look for him,
/// and how far from where he is each of them heads.
const HUNT_AFTER: f32 = 14.0;
const HUNT_SPREAD: f32 = 7.0;

/// What the members of a squad share.
#[derive(Clone, Copy, Debug, Default)]
pub struct Squad {
    pub members: u8,
    /// Where one of them last saw the player, and when.
    pub contact: V3,
    pub contact_at: f32,
    /// Who leans out now, as bits by soldier.
    pub peeking: u16,
    pub flanker: Option<u8>,
    pub grenade_wait: f32,
    /// When one of them last called out.
    pub called_at: f32,
}

/// What a soldier's step may ask of the match.
#[derive(Clone, Copy, Debug, Default)]
pub struct Act {
    /// A round: from where and along what.
    pub round: Option<(V3, V3)>,
    /// A grenade: from where and how fast.
    pub grenade: Option<(V3, V3)>,
}

/// What is around a soldier this step.
pub struct Around<'a> {
    pub level: &'a Level<'a>,
    /// The player's feet, where a round would strike him, and whether he is on them.
    pub target: V3,
    pub chest: V3,
    pub alive: bool,
    /// How far the player was heard this step, metres: 0 not at all.
    pub noise: f32,
    /// The step's number and the match's seconds.
    pub tick: u32,
    pub time: f32,
    /// Every soldier's place, whether he stands, and the cover he holds.
    pub others: &'a [(V3, bool, Option<u16>); MAX],
}

#[derive(Clone, Copy, Debug)]
pub struct Soldier {
    /// The slot holds a soldier, living or fallen.
    pub used: bool,
    /// 0 vanguard, 1 irregular, 2 breacher.
    pub variant: u8,
    pub squad: u8,
    pub pos: V3,
    pub yaw: f32,
    pub speed: f32,
    pub state: State,
    state_time: f32,
    pub health: f32,
    pub crouched: bool,
    pub peeking: bool,
    // ---- what he knows
    awareness: f32,
    alertness: f32,
    pub has_target: bool,
    pub visible: bool,
    last_known: V3,
    last_known_age: f32,
    unseen: f32,
    suppression: f32,
    aim: V3,
    // ---- where he goes
    path: Path,
    path_at: u8,
    goal: V3,
    going: bool,
    asked: bool,
    stuck: f32,
    /// Seconds before he may ask for a way again.
    ask_wait: f32,
    pub cover: Option<u16>,
    cover_wait: f32,
    peek_wait: f32,
    route: [V3; 3],
    route_at: u8,
    // ---- his weapon
    ammo: u32,
    burst: u32,
    burst_wait: f32,
    fire_wait: f32,
    pub reload: f32,
    grenade: bool,
    grenade_wait: f32,
    // ---- what the renderer shows
    /// The clip he is in and where in it (turns of a looped clip, seconds of one that plays once),
    /// the one he leaves, and how far the change is, 0…1.
    pub clip: u32,
    pub phase: f32,
    pub clip_before: u32,
    pub phase_before: f32,
    pub blend: f32,
    /// How far up he aims, radians.
    pub aim_pitch: f32,
    /// Seconds since he fired, was hit, and (once dead) fell; the way he falls.
    pub fired_ago: f32,
    pub hit_ago: f32,
    pub dead_time: f32,
    pub fall_yaw: f32,
    /// Seconds the map still shows him.
    pub shown: f32,
}

impl Soldier {
    pub const NONE: Soldier = Soldier {
        used: false,
        variant: 0,
        squad: 0,
        pos: V3::ZERO,
        yaw: 0.0,
        speed: 0.0,
        state: State::Dead,
        state_time: 0.0,
        health: 0.0,
        crouched: false,
        peeking: false,
        awareness: 0.0,
        alertness: 0.0,
        has_target: false,
        visible: false,
        last_known: V3::ZERO,
        last_known_age: 100.0,
        unseen: 100.0,
        suppression: 0.0,
        aim: V3::ZERO,
        path: Path {
            points: [V3::ZERO; crate::nav::POINTS],
            len: 0,
        },
        path_at: 0,
        goal: V3::ZERO,
        going: false,
        asked: false,
        stuck: 0.0,
        ask_wait: 0.0,
        cover: None,
        cover_wait: 0.0,
        peek_wait: 0.0,
        route: [V3::ZERO; 3],
        route_at: 0,
        ammo: 0,
        burst: 0,
        burst_wait: 0.0,
        fire_wait: 0.0,
        reload: 0.0,
        grenade: false,
        grenade_wait: 0.0,
        clip: clip::IDLE,
        phase: 0.0,
        clip_before: clip::IDLE,
        phase_before: 0.0,
        blend: 1.0,
        aim_pitch: 0.0,
        fired_ago: 10.0,
        hit_ago: 10.0,
        dead_time: 0.0,
        fall_yaw: 0.0,
        shown: 0.0,
    };

    pub fn spawn(
        variant: u8,
        squad: u8,
        pos: V3,
        yaw: f32,
        route: [V3; 3],
        rng: &mut Rng,
    ) -> Soldier {
        Soldier {
            used: true,
            variant,
            squad,
            pos,
            yaw,
            state: State::Idle,
            health: HEALTH,
            aim: pos + heading(yaw) * 10.0,
            route,
            ammo: MAGAZINE,
            burst_wait: rng.range(0.4, 1.4),
            grenade: true,
            grenade_wait: rng.range(9.0, 22.0),
            cover_wait: rng.range(0.0, 0.6),
            ..Soldier::NONE
        }
    }

    pub fn alive(&self) -> bool {
        self.used && self.state != State::Dead
    }

    /// The carbine, or the irregular's slower rifle.
    fn weapon(&self) -> usize {
        (self.variant == 1) as usize
    }

    pub fn eye(&self) -> V3 {
        self.pos + v3(0.0, if self.crouched { 1.1 } else { EYE }, 0.0)
    }

    pub fn muzzle(&self) -> V3 {
        self.pos + v3(0.0, if self.crouched { 0.95 } else { 1.42 }, 0.0) + heading(self.yaw) * 0.5
    }

    fn enter(&mut self, state: State) {
        self.state = state;
        self.state_time = 0.0;
    }

    /// The zones a round can strike, as capsules: each an axis, a radius and the zone's number
    /// (`tune::soldier::ZONE_*`). They stand where the reference's stand between its bones, for a
    /// soldier on his feet; a crouched one is two thirds as tall.
    fn zones(&self) -> [(V3, V3, f32, u8); 7] {
        let k = if self.crouched { 0.66 } else { 1.0 };
        let ahead = heading(self.yaw);
        let side = v3(cos(self.yaw), 0.0, -sin(self.yaw));
        let at = |x: f32, y: f32, z: f32| {
            self.pos
                + side * x
                + v3(0.0, y * k, 0.0)
                + ahead * (z + if self.crouched { 0.12 } else { 0.0 })
        };
        [
            (
                at(0.0, 1.57, 0.02),
                at(0.0, 1.72, 0.03),
                ZONE_RADIUS[0] * 1.15,
                0,
            ),
            (at(0.0, 1.215, 0.0), at(0.0, 1.475, 0.0), ZONE_RADIUS[1], 1),
            (at(0.0, 0.98, 0.0), at(0.0, 1.215, 0.0), ZONE_RADIUS[2], 2),
            (
                at(-0.19, 1.42, 0.0),
                at(-0.12, 1.28, 0.26),
                ZONE_RADIUS[3],
                3,
            ),
            (at(0.19, 1.42, 0.0), at(0.1, 1.3, 0.36), ZONE_RADIUS[3], 3),
            (
                at(-0.095, 0.945, 0.0),
                at(-0.1, 0.09, 0.0),
                ZONE_RADIUS[4],
                4,
            ),
            (at(0.095, 0.945, 0.0), at(0.1, 0.09, 0.0), ZONE_RADIUS[4], 4),
        ]
    }

    /// Where a ray strikes him first within `far`: how far along it, and the zone.
    pub fn struck_by(&self, from: V3, dir: V3, far: f32) -> Option<(f32, u8)> {
        if !self.alive() {
            return None;
        }
        // Nowhere near: no zone is farther than a metre from his middle.
        let middle = self.pos + v3(0.0, 0.9, 0.0);
        let along = (middle - from).dot(dir);
        if along < -1.2 || along > far + 1.2 || (from + dir * along - middle).len2() > 1.3 * 1.3 {
            return None;
        }
        let mut best: Option<(f32, u8)> = None;
        for (a, b, r, zone) in self.zones() {
            let (on_ray, on_axis) = nearest_on_segments(from, from + dir * far, a, b);
            let d2 = (on_ray - on_axis).len2();
            if d2 > r * r {
                continue;
            }
            // In from the nearest point by the half chord.
            let t = (on_ray - from).dot(dir) - sqrt(r * r - d2);
            if t >= 0.0 && best.is_none_or(|b| t < b.0) {
                best = Some((t, zone));
            }
        }
        best
    }

    /// Takes a hit. Returns whether it was the last.
    pub fn hurt(&mut self, me: u8, amount: f32, from: V3, events: &mut Events) -> bool {
        if !self.alive() {
            return false;
        }
        self.health -= amount;
        self.hit_ago = 0.0;
        self.suppression = min(1.6, self.suppression + 0.35);
        self.alertness = 1.0;
        self.last_known = from;
        self.last_known_age = min(self.last_known_age, 0.6);
        if matches!(self.state, State::Idle | State::Patrol) {
            self.enter(State::Alert);
        }
        if self.health <= 0.0 {
            self.health = 0.0;
            self.enter(State::Dead);
            self.peeking = false;
            self.speed = 0.0;
            self.dead_time = 0.0;
            // He falls away from where the round came from.
            self.fall_yaw = yaw_of(self.pos - from);
            events.push(Event::Bark {
                soldier: me,
                kind: 3,
                pos: self.pos,
            });
            events.push(Event::Fell {
                soldier: me,
                pos: self.pos,
            });
            return true;
        }
        events.push(Event::Bark {
            soldier: me,
            kind: 2,
            pos: self.pos,
        });
        false
    }

    /// A round or a blast passed near: he keeps his head down for a while.
    pub fn suppress(&mut self, amount: f32) {
        self.suppression = min(1.6, self.suppression + amount);
    }

    /// The cover that stands best between him and the threat (`nav.js:447-470`).
    fn pick_cover(&self, me: u8, a: &Around) -> Option<u16> {
        let threat = self.last_known;
        let reach = if self.cover.is_some() { 12.0 } else { 26.0 };
        let mut best: Option<(f32, u16)> = None;
        for (i, c) in a.level.cover.iter().enumerate() {
            let p = v3(c.pos[0], c.pos[1], c.pos[2]);
            let travel2 = (p - self.pos).flat().len2();
            if travel2 > reach * reach {
                continue;
            }
            let to = (threat - p).flat();
            let far = to.len();
            if !(2.5..=40.0).contains(&far) {
                continue;
            }
            let shelter = (to.x * c.dir[0] + to.z * c.dir[1]) / far;
            if shelter < 0.25 {
                continue;
            }
            let mut score =
                shelter * 5.0 + if c.high != 0 { 2.2 } else { 1.0 } - sqrt(travel2) * 0.16;
            if far < 7.0 {
                score -= (7.0 - far) * 0.55;
            }
            if far > 30.0 {
                score -= (far - 30.0) * 0.28;
            }
            let mut taken = false;
            for (k, (other, alive, held)) in a.others.iter().enumerate() {
                if k == me as usize || !alive {
                    continue;
                }
                taken |= *held == Some(i as u16);
                let d = (*other - p).flat().len();
                if d < 3.2 {
                    score -= (3.2 - d) * 1.4;
                }
            }
            if !taken && best.is_none_or(|b| score > b.0) {
                best = Some((score, i as u16));
            }
        }
        best.map(|b| b.1)
    }

    /// Sets him on his way to a place.
    fn go(&mut self, me: u8, to: V3, level: &Level, paths: &mut Pathfinder) {
        if self.going && (self.goal - to).flat().len2() < 1.0 {
            return;
        }
        // A way that was not found is not asked for again at once.
        if self.ask_wait > 0.0 {
            return;
        }
        self.goal = to;
        self.going = true;
        self.path.len = 0;
        self.path_at = 0;
        self.stuck = 0.0;
        match (
            level
                .nearest_open(self.pos.x, self.pos.z, self.pos.y)
                .or_else(|| level.nearest_below(self.pos.x, self.pos.z)),
            level
                .nearest_open(to.x, to.z, to.y)
                .or_else(|| level.nearest_below(to.x, to.z)),
        ) {
            (Some(from), Some(goal)) if from != goal => {
                paths.ask(me, from, goal);
                self.asked = true;
            }
            _ => {
                self.going = false;
                self.asked = false;
                self.ask_wait = 0.75;
            }
        }
    }

    fn halt(&mut self, me: u8, paths: &mut Pathfinder) {
        if self.asked {
            paths.forget(me);
        }
        self.going = false;
        self.asked = false;
    }

    /// The pathfinder's answer to his last question.
    /// For a bench's printout: whether he is on his way, waits for one, the way's points and the
    /// one he heads for, the seconds he has not moved, and his goal.
    pub fn way(&self) -> (bool, bool, u8, u8, f32, V3) {
        (self.going, self.asked, self.path.len, self.path_at, self.stuck, self.goal)
    }

    pub fn path_point(&self) -> V3 {
        self.path.points[(self.path_at as usize).min(crate::nav::POINTS - 1)]
    }

    pub fn take_path(&mut self, path: Path) {
        self.asked = false;
        self.path = path;
        self.path_at = 0;
        if path.len == 0 {
            self.going = false;
            self.ask_wait = 0.75;
        }
    }

    /// Walks his way at `top` metres a second. Returns whether he has arrived.
    fn walk(&mut self, me: u8, top: f32, a: &Around, paths: &mut Pathfinder) -> bool {
        if !self.going {
            self.speed = ease(self.speed, 0.0, 7.0, DT);
            return true;
        }
        if self.asked || self.path.len == 0 {
            self.speed = ease(self.speed, 0.0, 7.0, DT);
            return false;
        }
        let last = self.path_at as usize + 1 >= self.path.len as usize;
        let point = self.path.points[self.path_at as usize];
        let to = (point - self.pos).flat();
        let distance = to.len();
        if distance < if last { 0.45 } else { 0.75 } {
            if last {
                self.going = false;
                return true;
            }
            self.path_at += 1;
            return false;
        }
        let top = top
            * if self.crouched { 0.42 } else { 1.0 }
            * (1.0 - 0.25 * min(self.suppression, 1.0));
        self.speed = ease(self.speed, top, 7.0, DT);
        let mut step = to * (self.speed * DT / distance);
        // Round the others, a little to one side by the soldier's number.
        for (k, (other, alive, _)) in a.others.iter().enumerate() {
            if k == me as usize || !alive {
                continue;
            }
            let away = (self.pos - *other).flat();
            let d = away.len();
            if d < RADIUS * 2.0 + 0.42 && d > 1e-3 {
                let push = away * ((RADIUS * 2.0 + 0.42 - d) / d * 1.5 * DT);
                step += push + v3(-push.z, 0.0, push.x) * if me & 1 == 0 { 0.35 } else { -0.35 };
            }
        }
        // He stays on the grid's open nodes. Where the straight step meets a closed one he goes
        // along it: the step's part across the grid's lines first, the other if that is closed too.
        let open = |x: f32, z: f32| a.level.node(x, z).is_some_and(|n| a.level.open(n));
        let ways = if abs(step.x) > abs(step.z) {
            [(step.x, step.z), (step.x, 0.0), (0.0, step.z)]
        } else {
            [(step.x, step.z), (0.0, step.z), (step.x, 0.0)]
        };
        let moved = ways
            .into_iter()
            .find(|(x, z)| (*x != 0.0 || *z != 0.0) && open(self.pos.x + x, self.pos.z + z));
        match moved {
            Some((x, z)) => {
                self.pos.x += x;
                self.pos.z += z;
                self.stuck = if x * x + z * z < (self.speed * DT * 0.3) * (self.speed * DT * 0.3) {
                    self.stuck + DT
                } else {
                    0.0
                };
            }
            None => self.stuck += DT,
        }
        if self.stuck > 1.1 {
            // Something is in the way: ask again from where he stands.
            let goal = self.goal;
            self.going = false;
            self.go(me, goal, a.level, paths);
        }
        self.yaw = ease_angle(self.yaw, yaw_of(to), 6.5, DT);
        false
    }

    /// One step of him. `squad` is his own.
    pub fn step(
        &mut self,
        me: u8,
        a: &Around,
        squad: &mut Squad,
        paths: &mut Pathfinder,
        rng: &mut Rng,
        events: &mut Events,
    ) -> Act {
        let mut act = Act::default();
        self.fired_ago += DT;
        self.hit_ago += DT;
        self.ask_wait -= DT;
        self.shown = max(0.0, self.shown - DT);
        if self.state == State::Dead {
            self.dead_time += DT;
            return act;
        }
        self.state_time += DT;
        let bit = 1u16 << me;
        let eye = self.eye();
        let to_player = a.chest - eye;
        let distance = to_player.len();

        // ---- what he sees: a look every fourth step
        if (a.tick + me as u32) % 4 == 0 {
            let facing = heading(self.yaw).dot(to_player.flat().norm());
            let edge = if self.has_target {
                -0.2
            } else {
                0.643 - 0.25 * self.alertness
            };
            self.visible = a.alive
                && distance < VIEW_RANGE
                && (facing > edge || distance < 4.5)
                && a.level.clear(eye, a.chest);
        }
        if self.visible {
            self.awareness = min(
                1.0,
                self.awareness
                    + DT / max(
                        0.12,
                        0.16 + 0.0075 * distance + 0.28 * (1.0 - self.alertness),
                    ),
            );
            if self.awareness >= 1.0 {
                if !self.has_target && a.time - squad.called_at > 4.5 {
                    squad.called_at = a.time;
                    events.push(Event::Bark {
                        soldier: me,
                        kind: 0,
                        pos: self.pos,
                    });
                }
                self.has_target = true;
            }
        } else {
            self.awareness = max(0.0, self.awareness - 0.35 * DT);
        }
        if self.visible && self.has_target {
            self.last_known = a.target;
            self.last_known_age = 0.0;
            self.unseen = 0.0;
            self.alertness = 1.0;
            squad.contact = a.target;
            squad.contact_at = a.time;
        } else {
            self.last_known_age += DT;
            self.unseen += DT;
            if self.unseen > 6.5 {
                self.has_target = false;
            }
        }
        // ---- what he hears, and what his squad tells him
        if a.noise > 0.0 && (a.target - self.pos).len2() < a.noise * a.noise {
            self.alertness = 1.0;
            self.last_known = a.target;
            self.last_known_age = min(self.last_known_age, 0.5);
            self.awareness = max(self.awareness, min(0.85, self.awareness + 0.3));
            if matches!(self.state, State::Idle | State::Patrol) {
                self.enter(State::Alert);
            }
        }
        if !self.has_target && self.last_known_age > 1.5 && a.time - squad.contact_at < 4.0 {
            self.last_known = squad.contact;
            self.last_known_age = 1.2;
            self.alertness = 1.0;
            if matches!(self.state, State::Idle | State::Patrol) {
                self.enter(State::Alert);
            }
        }
        self.suppression = max(0.0, self.suppression - 0.55 * DT);
        self.cover_wait -= DT;
        self.peek_wait -= DT;
        self.grenade_wait -= DT;

        // ---- what he does
        let mut fire = false;
        match self.state {
            State::Idle => {
                self.crouched = false;
                self.walk(me, 0.0, a, paths);
                if self.has_target {
                    self.enter(State::Combat);
                } else if self.state_time > 2.5 {
                    self.enter(State::Patrol);
                }
            }
            State::Patrol => {
                self.crouched = false;
                let point = self.route[self.route_at as usize % 3];
                if (point - self.pos).flat().len2() < 1.1 * 1.1 {
                    self.route_at = (self.route_at + 1) % 3;
                    self.going = false;
                } else {
                    self.go(me, point, a.level, paths);
                }
                self.walk(me, PATROL_SPEED, a, paths);
                if self.has_target {
                    self.enter(State::Combat);
                } else if a.alive && self.state_time > 5.0 && a.time - squad.contact_at > HUNT_AFTER
                {
                    // A match is two sides that meet. A squad that has seen and heard nothing for
                    // a while goes to look where the other side is, each of them to a place of
                    // his own some metres from it.
                    let turn = me as f32 * 2.4 + a.time * 0.37;
                    self.last_known = a.target + v3(cos(turn), 0.0, sin(turn)) * HUNT_SPREAD;
                    self.last_known_age = 0.0;
                    self.going = false;
                    self.enter(State::Alert);
                }
            }
            State::Alert => {
                self.crouched = false;
                if self.last_known_age < 8.0 {
                    let to = self.last_known;
                    self.go(me, to, a.level, paths);
                }
                self.walk(me, ALERT_SPEED, a, paths);
                if self.has_target {
                    self.enter(State::Combat);
                } else if self.state_time > 12.0 {
                    self.halt(me, paths);
                    self.enter(State::Patrol);
                }
            }
            State::Combat => {
                if !self.has_target && self.last_known_age >= 5.0 {
                    squad.peeking &= !bit;
                    self.peeking = false;
                    self.cover = None;
                    self.enter(State::Alert);
                } else if self.suppression > 1.15 && self.cover.is_some() {
                    squad.peeking &= !bit;
                    self.peeking = false;
                    self.enter(State::Suppressed);
                } else {
                    if self.cover_wait <= 0.0 {
                        self.cover_wait = rng.range(2.2, 4.5);
                        if let Some(found) = self.pick_cover(me, a) {
                            self.cover = Some(found);
                        }
                    }
                    let place = self.cover.map(|c| a.level.cover[c as usize]);
                    let at_cover = place.is_none_or(|c| {
                        (v3(c.pos[0], c.pos[1], c.pos[2]) - self.pos).flat().len2() < 0.85 * 0.85
                    });
                    if let (Some(c), false) = (place, at_cover) {
                        // On his way to cover he runs and does not fire.
                        squad.peeking &= !bit;
                        self.peeking = false;
                        self.crouched = false;
                        self.go(me, v3(c.pos[0], c.pos[1], c.pos[2]), a.level, paths);
                        self.walk(me, COVER_SPEED, a, paths);
                        if !self.going && !self.asked {
                            // No way there: another cover, soon.
                            self.cover = None;
                            self.cover_wait = min(self.cover_wait, 0.6);
                        }
                    } else {
                        self.halt(me, paths);
                        self.walk(me, 0.0, a, paths);
                        if self.peek_wait <= 0.0 {
                            // Half the squad leans out at a time.
                            let room = (squad.peeking & !bit).count_ones()
                                < (squad.members as u32).div_ceil(2).max(1);
                            self.peeking = room && self.visible;
                            squad.peeking = if self.peeking {
                                squad.peeking | bit
                            } else {
                                squad.peeking & !bit
                            };
                            self.peek_wait = if self.peeking {
                                rng.range(1.1, 2.4)
                            } else {
                                rng.range(0.7, 1.8)
                            };
                        }
                        self.crouched = match place {
                            Some(c) if c.high != 0 => !self.peeking,
                            Some(_) => true,
                            None => false,
                        };
                        fire = self.peeking
                            && self.visible
                            && self.has_target
                            && distance < WEAPON_RANGE;
                        self.yaw =
                            ease_angle(self.yaw, yaw_of(self.last_known - self.pos), 3.4, DT);
                        // Hurt, he may fall back; with the squad holding, he may go round; and his one grenade.
                        if self.health < 34.0 && self.state_time > 1.5 && rng.float() < DT * 0.5 {
                            squad.peeking &= !bit;
                            self.peeking = false;
                            let away = (self.pos - self.last_known)
                                .flat()
                                .norm_or(heading(self.yaw));
                            let to = self.pos + away * 9.0;
                            self.cover = None;
                            self.go(me, to, a.level, paths);
                            self.enter(State::Retreat);
                        } else if self.state_time > 4.0
                            && squad.flanker.is_none()
                            && squad.members > 1
                            && rng.float() < DT * 0.25
                        {
                            squad.peeking &= !bit;
                            self.peeking = false;
                            squad.flanker = Some(me);
                            let toward = (self.last_known - self.pos)
                                .flat()
                                .norm_or(heading(self.yaw));
                            let side = v3(-toward.z, 0.0, toward.x)
                                * if rng.float() < 0.5 { 1.0 } else { -1.0 };
                            let to = self.pos + side * rng.range(8.0, 15.0) + toward * 4.0;
                            self.cover = None;
                            self.go(me, to, a.level, paths);
                            self.enter(State::Flank);
                        } else if self.grenade
                            && self.grenade_wait <= 0.0
                            && squad.grenade_wait <= 0.0
                            && (8.0..26.0).contains(&distance)
                            && self.last_known_age < 1.5
                        {
                            self.grenade = false;
                            squad.grenade_wait = rng.range(14.0, 26.0);
                            let to = (self.last_known - self.pos).flat();
                            let reach = to.len();
                            let speed = min(18.0, sqrt(max(4.0, reach * GRAVITY / 0.95)));
                            act.grenade = Some((
                                self.eye(),
                                to.norm() * (speed * 0.78) + v3(0.0, speed * 0.62, 0.0),
                            ));
                        }
                    }
                }
            }
            State::Suppressed => {
                self.crouched = true;
                self.walk(me, 0.0, a, paths);
                if self.suppression < 0.45 {
                    self.enter(State::Combat);
                }
            }
            State::Flank => {
                self.crouched = false;
                let arrived = self.walk(me, FLANK_SPEED, a, paths);
                if arrived || self.state_time > 7.0 || self.suppression > 1.0 {
                    squad.flanker = None;
                    self.halt(me, paths);
                    self.cover_wait = 0.0;
                    self.enter(State::Combat);
                }
            }
            State::Retreat => {
                self.crouched = false;
                if self.walk(me, RETREAT_SPEED, a, paths) || self.state_time > 6.0 {
                    self.halt(me, paths);
                    self.cover_wait = 0.0;
                    self.enter(State::Combat);
                }
            }
            State::Dead => {}
        }
        // His feet follow the grid's floor.
        if let Some(node) = a.level.node(self.pos.x, self.pos.z) {
            if a.level.open(node) {
                self.pos.y = ease(
                    self.pos.y,
                    a.level.floor[node as usize] as f32 * 0.01,
                    14.0,
                    DT,
                );
            }
        }

        // ---- his weapon
        self.aim = self.aim.ease(
            self.last_known + v3(0.0, a.chest.y - a.target.y, 0.0),
            6.0,
            DT,
        );
        let muzzle = self.muzzle();
        let line = self.aim - muzzle;
        self.aim_pitch = ease(
            self.aim_pitch,
            if self.has_target || self.state == State::Alert {
                clamp(atan2(line.y, max(line.flat().len(), 0.5)), -0.7, 0.7)
            } else {
                0.0
            },
            8.0,
            DT,
        );
        self.fire_wait -= DT;
        self.burst_wait -= DT;
        if self.reload > 0.0 {
            self.reload -= DT;
            if self.reload <= 0.0 {
                self.ammo = MAGAZINE;
            }
        } else if fire {
            if self.burst == 0 && self.burst_wait <= 0.0 {
                self.burst = rng.int(3, 7);
                self.burst_wait = rng.range(0.45, 1.35) + 0.5 * self.suppression;
            }
            if self.burst > 0 && self.fire_wait <= 0.0 {
                self.burst -= 1;
                self.ammo -= 1;
                self.fire_wait = 1.0 / FIRE_RATE[self.weapon()];
                self.fired_ago = 0.0;
                self.shown = 2.5;
                // Where he aims, and as far off it as his hands are steady.
                let sigma = SPREAD * (1.0 + 1.5 * min(self.suppression, 1.0));
                let dir = (line.norm()
                    + v3(
                        rng.gauss() * sigma,
                        rng.gauss() * sigma * 0.8,
                        rng.gauss() * sigma,
                    ))
                .norm();
                act.round = Some((muzzle, dir));
                if self.ammo == 0 {
                    self.reload = RELOAD[self.weapon()];
                    self.burst = 0;
                    events.push(Event::Bark {
                        soldier: me,
                        kind: 1,
                        pos: self.pos,
                    });
                }
            }
        } else {
            self.burst = 0;
        }

        // ---- what the renderer plays
        let (next, rate) = if self.crouched {
            if self.speed > 0.25 {
                (clip::CROUCH_WALK, self.speed / 0.95)
            } else {
                (clip::CROUCH_AIM, 1.0 / 1.5)
            }
        } else if self.speed > 2.6 {
            (clip::RUN, self.speed / 2.05)
        } else if self.speed > 0.25 {
            if self.alertness > 0.5 {
                (clip::WALK_AIM, max(0.55, self.speed / 1.42))
            } else {
                (clip::PATROL, max(0.55, self.speed / 1.42))
            }
        } else if self.alertness > 0.5 {
            (clip::AIM, 1.0 / 1.5)
        } else {
            (clip::IDLE, 1.0 / 1.5)
        };
        if next != self.clip {
            self.clip_before = self.clip;
            self.phase_before = self.phase;
            self.clip = next;
            self.blend = 0.0;
            // A stride goes on from where the last gait's was.
            if !matches!(
                next,
                clip::WALK_AIM | clip::RUN | clip::PATROL | clip::CROUCH_WALK
            ) {
                self.phase = 0.0;
            }
        }
        self.blend = min(1.0, self.blend + DT / 0.18);
        self.phase += rate * DT;
        self.phase -= floor(self.phase);
        act
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_round_through_the_head_strikes_the_head() {
        let mut rng = Rng::new(1);
        // Standing at the origin, facing -Z.
        let s = Soldier::spawn(0, 0, V3::ZERO, 0.0, [V3::ZERO; 3], &mut rng);
        let head = s
            .struck_by(v3(0.0, 1.65, -10.0), v3(0.0, 0.0, 1.0), 50.0)
            .unwrap();
        assert!(head.1 == 0 && abs(head.0 - 9.9) < 0.2, "{head:?}");
        let chest = s
            .struck_by(v3(0.0, 1.35, -10.0), v3(0.0, 0.0, 1.0), 50.0)
            .unwrap();
        assert_eq!(chest.1, 1);
        let leg = s
            .struck_by(v3(0.1, 0.4, -10.0), v3(0.0, 0.0, 1.0), 50.0)
            .unwrap();
        assert_eq!(leg.1, 4);
        assert!(s
            .struck_by(v3(0.8, 1.35, -10.0), v3(0.0, 0.0, 1.0), 50.0)
            .is_none());
        assert!(
            s.struck_by(v3(0.0, 1.35, -10.0), v3(0.0, 0.0, 1.0), 5.0)
                .is_none(),
            "out of reach"
        );
    }

    #[test]
    fn four_rounds_of_the_rifle_to_the_chest_fell_him() {
        let mut rng = Rng::new(1);
        let mut events = Events::default();
        let mut s = Soldier::spawn(0, 0, V3::ZERO, 0.0, [V3::ZERO; 3], &mut rng);
        for _ in 0..3 {
            assert!(!s.hurt(0, 33.0, v3(0.0, 0.0, -10.0), &mut events));
        }
        assert!(s.hurt(0, 33.0, v3(0.0, 0.0, -10.0), &mut events) && !s.alive());
        assert!(events.iter().any(|e| matches!(e, Event::Fell { .. })));
    }
}
