//! A match.
//!
//! The reference has the street, the weapons and the soldiers, and no match:
//! six soldiers stand guard, nobody comes after them, and a player who is
//! killed stays where he fell (`web/src/ai/index.js:483-538`,
//! `web/src/player/health.js:126-129`). Its interface shows what it was
//! meant to be: two scores, the word TDM and a clock at 10:00
//! (`web/src/ui/index.js:119-122`). This is that match: the player against
//! the soldiers' side, a point a kill, ten minutes or the first to the score
//! that ends it. Soldiers come in threes while fewer than five stand, as the
//! reference's own demonstration brings them (`web/tools/demo-driver.js`),
//! and the player is put back on his feet a few seconds after he falls.

use crate::event::{Event, Events};
use crate::level::Level;
use crate::math::*;
use crate::nav::Pathfinder;
use crate::player::{Input, Player};
use crate::rng::Rng;
use crate::soldier::{Around, Soldier, Squad, State, MAX, SQUADS};
use crate::tune::soldier as rules;
use crate::tune::{WeaponDef, DT, GRAVITY, SURFACES};
use crate::weapon::{Arms, Round, WEAPONS};

/// Seconds a match lasts, and the score that ends it sooner.
pub const MATCH_SECONDS: f32 = 600.0;
pub const SCORE_LIMIT: u32 = 30;
/// Seconds the player lies before he is put back.
pub const DOWN_SECONDS: f32 = 4.0;
/// Soldiers standing below which more come, and the seconds between two arrivals.
const REINFORCE_BELOW: usize = 5;
const REINFORCE_WAIT: f32 = 6.0;
/// Seconds a fallen soldier stays.
const CORPSE_SECONDS: f32 = 14.0;
pub const GRENADES: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    Play,
    Down { wait: f32 },
    Over { won: bool },
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Tally {
    pub kills: u32,
    pub deaths: u32,
    pub headshots: u32,
    pub shots: u32,
    pub hits: u32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Grenade {
    pub live: bool,
    pub pos: V3,
    pub vel: V3,
    pub fuse: f32,
}

pub struct Game<'a> {
    pub level: Level<'a>,
    pub player: Player,
    pub arms: Arms,
    pub soldiers: [Soldier; MAX],
    pub squads: [Squad; SQUADS],
    pub paths: Pathfinder,
    pub grenades: [Grenade; GRENADES],
    pub rng: Rng,
    /// What the last step did.
    pub events: Events,
    pub phase: Phase,
    /// Seconds played, and seconds left.
    pub time: f32,
    pub clock: f32,
    /// The player's side, the soldiers'.
    pub score: [u32; 2],
    pub tally: Tally,
    pub tick: u32,
    reinforce: f32,
    /// How hard the soldiers' rounds hit, of the reference's 1.
    pub skill: f32,
    /// Nodes the way-finding opens in a tick.
    pub path_budget: u32,
    /// The aim's help: the look slows over a soldier, and the sights rise onto the nearest one.
    pub assist: bool,
    /// What is left of a rise of the sights onto a soldier: yaw, pitch, seconds.
    snap: (f32, f32, f32),
    ads_held: bool,
    /// The last hit the player landed (`count × 4 + kind`) and took (`count × 360 + degrees`).
    pub hit_mark: u32,
    pub hurt_mark: u32,
    /// The soldier whose round last hurt the player.
    last_by: Option<u8>,
}

impl<'a> Game<'a> {
    pub fn new(level: Level<'a>, defs: [WeaponDef; WEAPONS], seed: u32) -> Game<'a> {
        let start = level.spawns.first().copied().unwrap_or_default();
        let mut game = Game {
            level,
            player: Player::new(v3(start.pos[0], start.pos[1], start.pos[2]), start.yaw),
            arms: Arms::new(defs),
            soldiers: [Soldier::NONE; MAX],
            squads: [Squad::default(); SQUADS],
            paths: Pathfinder::new(level.flags.len()),
            grenades: [Grenade::default(); GRENADES],
            rng: Rng::new(seed),
            events: Events::default(),
            phase: Phase::Play,
            time: 0.0,
            clock: MATCH_SECONDS,
            score: [0; 2],
            tally: Tally::default(),
            tick: 0,
            reinforce: REINFORCE_WAIT,
            skill: 0.7,
            path_budget: 120,
            assist: true,
            snap: (0.0, 0.0, 0.0),
            ads_held: false,
            hit_mark: 0,
            hurt_mark: 0,
            last_by: None,
        };
        game.garrison();
        game
    }

    /// A new match on the same ground, with the settings kept.
    pub fn restart(&mut self) {
        let (skill, assist, turn, invert, analog) = (
            self.skill,
            self.assist,
            self.player.turn,
            self.player.invert,
            self.player.analog_look,
        );
        let seed = self.rng.u32();
        *self = Game::new(self.level, self.arms.defs, seed);
        self.skill = skill;
        self.assist = assist;
        self.player.turn = turn;
        self.player.invert = invert;
        self.player.analog_look = analog;
    }

    fn spawn_point(&self, i: usize) -> V3 {
        let s = self.level.spawns[i];
        v3(s.pos[0], s.pos[1], s.pos[2])
    }

    /// Puts a squad of three round a spawn point (`web/src/ai/index.js:514-531`).
    fn squad_at(&mut self, squad: usize, anchor: usize) {
        if self.level.spawns.is_empty() {
            return;
        }
        let at = self.spawn_point(anchor);
        // His round: the anchor and the two spawn points nearest to it.
        let mut near: [(f32, usize); 2] = [(f32::MAX, anchor); 2];
        for i in 0..self.level.spawns.len() {
            if i == anchor {
                continue;
            }
            let d = (self.spawn_point(i) - at).len2();
            if d < near[0].0 {
                near[1] = near[0];
                near[0] = (d, i);
            } else if d < near[1].0 {
                near[1] = (d, i);
            }
        }
        let route = [at, self.spawn_point(near[0].1), self.spawn_point(near[1].1)];
        self.squads[squad] = Squad {
            grenade_wait: 6.0,
            ..Default::default()
        };
        for member in 0..3u8 {
            let Some(slot) = self.soldiers.iter().position(|s| !s.used) else {
                break;
            };
            let (turn, reach) = (self.rng.float() * TAU, self.rng.range(0.8, 3.2));
            let want = at + v3(cos(turn) * reach, 0.0, sin(turn) * reach);
            let place = self
                .level
                .nearest_open(want.x, want.z, at.y)
                .map_or(at, |n| self.level.node_pos(n));
            let yaw = self.level.spawns[anchor].yaw + self.rng.signed() * 0.7;
            self.soldiers[slot] = Soldier::spawn(
                (squad as u8 * 3 + member) % 3,
                squad as u8,
                place,
                yaw,
                route,
                &mut self.rng,
            );
            self.squads[squad].members += 1;
        }
    }

    /// The soldiers a match starts with: two squads at the two spawn points farthest from the player.
    fn garrison(&mut self) {
        let from = self.player.body.pos;
        let mut ranked: [(f32, usize); 16] = [(0.0, 0); 16];
        let n = self.level.spawns.len().min(16);
        for (i, slot) in ranked.iter_mut().enumerate().take(n) {
            *slot = ((self.spawn_point(i) - from).len2(), i);
        }
        ranked[..n].sort_unstable_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        for squad in 0..2.min(n) {
            self.squad_at(squad, ranked[squad].1);
        }
    }

    pub fn standing(&self) -> usize {
        self.soldiers.iter().filter(|s| s.alive()).count()
    }

    /// The player's chest: where a soldier's round strikes him.
    fn chest(&self) -> V3 {
        self.player.body.pos
            + v3(
                0.0,
                if self.player.crouched {
                    0.85
                } else {
                    rules::CHEST
                },
                0.0,
            )
    }

    /// Carries the player's round to what it hits, through what it passes
    /// (`web/src/physics/penetration.js:76-166`).
    fn carry(&mut self, round: Round) {
        let def = self.arms.defs[round.weapon];
        let (mut from, dir) = (round.from, round.dir);
        let (mut damage, mut power, mut travelled) = (def.damage, def.penetration, 0.0f32);
        let mut end = from + dir * 120.0;
        self.tally.shots += 1;
        for _ in 0..3 {
            let wall = self.level.ray(from, dir, 200.0);
            let reach = wall.map_or(200.0, |h| h.t);
            // The nearest soldier before the wall.
            let mut struck: Option<(f32, u8, usize)> = None;
            for (i, s) in self.soldiers.iter().enumerate() {
                if let Some((t, zone)) = s.struck_by(from, dir, reach) {
                    if struck.is_none_or(|b| t < b.0) {
                        struck = Some((t, zone, i));
                    }
                }
            }
            if let Some((t, zone, i)) = struck {
                let at = from + dir * t;
                let far = travelled + t;
                // The weapon's own loss with distance, and the soldiers' (`ai/index.js:352-359`).
                let weapon = 1.0
                    - (1.0 - def.dropoff)
                        * min(1.0, far / def.max_range)
                        * min(1.0, far / def.max_range);
                let range = max(
                    rules::FALLOFF_FLOOR,
                    1.0 - max(0.0, far - rules::FALLOFF_FROM) * rules::FALLOFF_PER_METRE,
                );
                let killed = self.soldiers[i].hurt(
                    i as u8,
                    damage * weapon * range * rules::ZONE_SCALE[zone as usize],
                    round.from,
                    &mut self.events,
                );
                self.tally.hits += 1;
                self.hit_mark =
                    (self.hit_mark / 4 + 1) * 4 + if killed { 2 } else { (zone == 0) as u32 };
                if killed {
                    self.tally.kills += 1;
                    self.tally.headshots += (zone == 0) as u32;
                    self.score[0] += 1;
                    let squad = &mut self.squads[self.soldiers[i].squad as usize];
                    squad.members = squad.members.saturating_sub(1);
                    squad.peeking &= !(1 << i);
                    if squad.flanker == Some(i as u8) {
                        squad.flanker = None;
                    }
                    self.paths.forget(i as u8);
                }
                self.events.push(Event::Struck {
                    soldier: i as u8,
                    zone,
                    killed,
                    pos: at,
                });
                end = at;
                break;
            }
            let Some(hit) = wall else { break };
            let at = from + dir * hit.t;
            end = at;
            travelled += hit.t;
            let surface = (hit.tag & 255) as u8;
            self.events.push(Event::Impact {
                pos: at,
                normal: hit.n,
                surface,
            });
            // A round near a soldier's head keeps it down.
            for s in self.soldiers.iter_mut().filter(|s| s.alive()) {
                let d = (s.eye() - at).len();
                if d < 3.2 {
                    s.suppress(0.5 * (1.0 - d / 3.2));
                }
            }
            // Through it, if the round has the force: what it is made of, and how thick it is here.
            let s = SURFACES[(surface as usize).min(SURFACES.len() - 1)];
            let budget = s.pen_depth * power;
            let inside = at + dir * 0.002;
            let thick = match self.level.ray(inside, dir, 1.6) {
                Some(out) if out.back => out.t,
                _ => 0.018 / max(0.2, abs(hit.n.dot(dir))),
            };
            if thick > budget {
                break;
            }
            let part = thick / budget;
            damage *= max(0.05, 1.0 - s.energy_loss * part);
            power *= 1.0 - part;
            if power < 0.02 || damage < 1.0 {
                break;
            }
            from = inside + dir * (thick + 0.004);
            travelled += thick;
        }
        self.events.push(Event::Shot {
            soldier: None,
            weapon: round.weapon as u8,
            from: round.from,
            to: end,
            tracer: round.tracer,
        });
    }

    /// Carries a soldier's round: it strikes the player when it passes his chest closer than the
    /// reference's radius, before anything solid (`web/src/ai/index.js:628-655`).
    fn soldier_round(&mut self, i: usize, from: V3, dir: V3) {
        let wall = self.level.ray(from, dir, 200.0);
        let reach = wall.map_or(200.0, |h| h.t);
        let chest = self.chest();
        let along = (chest - from).dot(dir);
        let miss = (from + dir * along - chest).len();
        let mut end = from + dir * reach;
        if !self.player.dead && along >= 0.5 && along < reach && miss <= rules::HIT_RADIUS {
            let amount =
                rules::DAMAGE * self.skill * if miss < rules::HIT_CLOSE { 1.25 } else { 1.0 };
            end = from + dir * along;
            self.last_by = Some(i as u8);
            self.strike_player(amount, from);
        } else {
            if let Some(hit) = wall {
                self.events.push(Event::Impact {
                    pos: end,
                    normal: hit.n,
                    surface: (hit.tag & 255) as u8,
                });
            }
            if along > 2.0 && along < reach && miss < 2.2 {
                self.events.push(Event::Whizz {
                    pos: from + dir * along,
                });
            }
        }
        // (the soldiers' tracer is one round in three: `ai/index.js:625`)
        self.events.push(Event::Shot {
            soldier: Some(i as u8),
            weapon: self.soldiers[i].variant,
            from,
            to: end,
            tracer: (self.tick + i as u32) % 3 == 0,
        });
    }

    fn strike_player(&mut self, amount: f32, from: V3) {
        let was_dead = self.player.dead;
        self.player.hurt(amount, from, &mut self.events);
        // Where it came from, clockwise from straight ahead.
        let to = (from - self.player.body.pos).flat();
        let degrees = wrap_angle(self.player.yaw - yaw_of(to)) * (180.0 / PI);
        self.hurt_mark = (self.hurt_mark / 360 + 1) * 360 + ((degrees + 360.0) as u32 % 360);
        if self.player.dead && !was_dead {
            self.tally.deaths += 1;
            self.score[1] += 1;
            self.phase = Phase::Down { wait: DOWN_SECONDS };
            self.events.push(Event::Down {
                soldier: self.last_by,
            });
        }
    }

    /// Where to put the player back: the spawn point the standing soldiers are farthest from.
    fn respawn(&mut self) {
        let mut best = (-1.0f32, 0usize);
        for i in 0..self.level.spawns.len() {
            let p = self.spawn_point(i);
            let nearest = self
                .soldiers
                .iter()
                .filter(|s| s.alive())
                .map(|s| (s.pos - p).len2())
                .fold(f32::MAX, min);
            if nearest > best.0 {
                best = (nearest, i);
            }
        }
        if !self.level.spawns.is_empty() {
            let s = self.level.spawns[best.1];
            self.player
                .place(v3(s.pos[0], s.pos[1] + 0.03, s.pos[2]), s.yaw);
        }
        // He comes back with full magazines and the fire modes he had chosen.
        let mode = self.arms.mode;
        self.arms = Arms::new(self.arms.defs);
        self.arms.mode = mode;
        self.last_by = None;
        self.phase = Phase::Play;
        self.events.push(Event::Up);
    }

    /// Three more soldiers, at the spawn point farthest from the player that he cannot see.
    fn reinforce(&mut self) {
        let Some(squad) = (0..SQUADS).find(|q| self.squads[*q].members == 0) else {
            return;
        };
        if self.soldiers.iter().filter(|s| !s.used).count() < 3 {
            return;
        }
        let eye = self.player.head();
        let mut best = (-1.0f32, None);
        for i in 0..self.level.spawns.len() {
            let p = self.spawn_point(i);
            let d = (p - eye).len2();
            let hidden = !self.level.clear(eye, p + v3(0.0, 1.5, 0.0));
            let score = if hidden { d } else { d * 0.1 };
            if d > 18.0 * 18.0 && score > best.0 {
                best = (score, Some(i));
            }
        }
        if let Some(anchor) = best.1 {
            self.squad_at(squad, anchor);
            // They know there is a fight: they come looking.
            for s in self
                .soldiers
                .iter_mut()
                .filter(|s| s.alive() && s.squad == squad as u8)
            {
                s.suppress(0.0);
            }
            self.squads[squad].contact = self.player.body.pos;
            self.squads[squad].contact_at = self.time;
        }
    }

    /// The aim's help: the look is slower while a soldier in sight is under the crosshair, and
    /// raising the sights turns the view onto the nearest one in front of it.
    fn help_aim(&mut self, input: &Input) {
        self.player.look_scale = 1.0;
        let ads = input.buttons & crate::player::btn::ADS != 0;
        let pressed = ads && !self.ads_held;
        self.ads_held = ads;
        if !self.assist || self.player.dead {
            return;
        }
        let (eye, yaw, pitch, _, _) = self.player.eye();
        let ahead = forward(yaw, pitch);
        let mut nearest: Option<(f32, V3)> = None;
        for s in self.soldiers.iter().filter(|s| s.alive() && s.visible) {
            let to = s.pos + v3(0.0, if s.crouched { 0.8 } else { 1.25 }, 0.0) - eye;
            let d = to.len();
            let off = acos(clamp(to.dot(ahead) / max(d, 1e-3), -1.0, 1.0));
            // Within the soldier's own width at his distance, and a little more.
            if off < atan2(0.45, d) + 0.02 {
                self.player.look_scale = 0.5;
            }
            if off < 0.16 && nearest.is_none_or(|b| off < b.0) {
                nearest = Some((off, to));
            }
        }
        if pressed {
            if let Some((_, to)) = nearest {
                self.snap = (
                    wrap_angle(yaw_of(to) - yaw) * 0.8,
                    (atan2(to.y, to.flat().len()) - pitch) * 0.8,
                    0.14,
                );
            }
        }
        if self.snap.2 > 0.0 {
            let part = min(DT, self.snap.2) / 0.14;
            self.player.yaw = wrap_angle(self.player.yaw + self.snap.0 * part);
            self.player.pitch += self.snap.1 * part;
            self.snap.2 -= DT;
        }
    }

    /// One step of the match with the pad as it is.
    pub fn step(&mut self, input: &Input) {
        self.events.clear();
        if let Phase::Over { .. } = self.phase {
            return;
        }
        self.tick += 1;
        self.time += DT;
        self.clock -= DT;
        if let Phase::Down { wait } = &mut self.phase {
            *wait -= DT;
            if *wait <= 0.0 {
                self.respawn();
            }
        }
        // ---- the player
        self.help_aim(input);
        let still = Input::default();
        let pad = if self.player.dead { &still } else { input };
        self.player.step(&self.level, pad, &mut self.events);
        let round = self.arms.step(
            &mut self.player,
            pad.buttons,
            &mut self.rng,
            &mut self.events,
        );
        // How far he was heard: a shot, a running step, a walking one (`ai/index.js:296-349`).
        let mut noise = 0.0f32;
        for e in self.events.iter() {
            if let Event::Footstep { running, .. } = e {
                noise = max(noise, if *running { 24.0 } else { 11.0 });
            }
        }
        if let Some(round) = round {
            noise = 90.0;
            // A round passing a soldier keeps his head down.
            for s in self.soldiers.iter_mut().filter(|s| s.alive()) {
                let to = s.eye() - round.from;
                let along = to.dot(round.dir);
                let d = (to - round.dir * along).len();
                if along > 0.0 && d < 2.6 {
                    s.suppress(0.45 * (1.0 - d / 2.6) + 0.12);
                }
            }
            self.carry(round);
        }

        // ---- the soldiers
        if let Some((owner, path)) = self.paths.step(&self.level, self.path_budget) {
            self.soldiers[owner as usize].take_path(path);
        }
        let mut others = [(V3::ZERO, false, None); MAX];
        for (o, s) in others.iter_mut().zip(&self.soldiers) {
            *o = (s.pos, s.alive(), s.cover);
        }
        let (target, chest, alive) = (self.player.body.pos, self.chest(), !self.player.dead);
        for i in 0..MAX {
            if !self.soldiers[i].used {
                continue;
            }
            let mut s = self.soldiers[i];
            let around = Around {
                level: &self.level,
                target,
                chest,
                alive,
                noise,
                tick: self.tick,
                time: self.time,
                others: &others,
            };
            let act = s.step(
                i as u8,
                &around,
                &mut self.squads[s.squad as usize],
                &mut self.paths,
                &mut self.rng,
                &mut self.events,
            );
            if s.state == State::Dead && s.dead_time > CORPSE_SECONDS {
                s.used = false;
            }
            self.soldiers[i] = s;
            if let Some((from, dir)) = act.round {
                self.soldier_round(i, from, dir);
            }
            if let Some((pos, vel)) = act.grenade {
                if let Some(g) = self.grenades.iter_mut().find(|g| !g.live) {
                    *g = Grenade {
                        live: true,
                        pos,
                        vel,
                        fuse: rules::GRENADE_FUSE,
                    };
                    self.events.push(Event::Thrown { pos });
                }
            }
        }
        for squad in &mut self.squads {
            squad.grenade_wait -= DT;
        }

        // ---- the grenades
        for k in 0..GRENADES {
            let mut g = self.grenades[k];
            if !g.live {
                continue;
            }
            g.vel.y -= GRAVITY * DT;
            let step = g.vel * DT;
            let length = step.len();
            if let Some(hit) = (length > 1e-5)
                .then(|| self.level.ray(g.pos, step * (1.0 / length), length + 0.05))
                .flatten()
            {
                // It strikes and comes off slower (`ai/index.js:672-717`).
                let into = g.vel.dot(hit.n);
                g.vel = (g.vel - hit.n * (1.28 * into)) * 0.7;
                g.pos += step * (max(0.0, hit.t - 0.05) / length);
                if into < -1.5 {
                    self.events.push(Event::Bounce { pos: g.pos });
                }
            } else {
                g.pos += step;
            }
            g.fuse -= DT;
            if g.fuse <= 0.0 {
                g.live = false;
                self.events.push(Event::Blast { pos: g.pos });
                let eye = self.player.head();
                let d = (eye - g.pos).len();
                if d < rules::GRENADE_RADIUS && self.level.clear(g.pos + v3(0.0, 0.2, 0.0), eye) {
                    let amount = rules::GRENADE_DAMAGE
                        * self.skill
                        * libm::powf(1.0 - d / rules::GRENADE_RADIUS, 1.6);
                    self.last_by = None;
                    self.strike_player(amount, g.pos);
                }
            }
            self.grenades[k] = g;
        }

        // ---- more soldiers, and the end
        self.reinforce -= DT;
        if self.standing() < REINFORCE_BELOW && self.reinforce <= 0.0 {
            self.reinforce = REINFORCE_WAIT;
            self.reinforce();
        }
        if self.clock <= 0.0 || self.score[0] >= SCORE_LIMIT || self.score[1] >= SCORE_LIMIT {
            self.clock = max(self.clock, 0.0);
            let won = self.score[0] > self.score[1];
            self.phase = Phase::Over { won };
            self.events.push(Event::Over { won });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bvh::Bvh;
    use crate::level::{Cover, Spawn};
    use crate::player::btn;
    use crate::weapon::reference;
    use alloc::vec::Vec;

    /// An open yard of 40 m with a wall across its middle and a gap in the wall.
    struct Yard {
        boxes: crate::body::tests::Boxes,
        nav: crate::level::NavHeader,
        flags: Vec<u8>,
        floor: Vec<i16>,
        cover: Vec<Cover>,
        spawns: Vec<Spawn>,
    }

    fn yard() -> Yard {
        let boxes = crate::body::tests::Boxes::new(&[
            ([-2.0, -1.0, -2.0], [42.0, 0.0, 42.0]),
            ([0.0, 0.0, 19.6], [16.0, 3.0, 20.4]),
            ([24.0, 0.0, 19.6], [40.0, 3.0, 20.4]),
        ]);
        let rows: Vec<alloc::string::String> = (0..51)
            .map(|j| {
                (0..51)
                    .map(|i| {
                        if (24..=26).contains(&j) && !(21..=29).contains(&i) {
                            '#'
                        } else {
                            '.'
                        }
                    })
                    .collect()
            })
            .collect();
        let refs: Vec<&str> = rows.iter().map(|r| r.as_str()).collect();
        let (nav, flags, floor) = crate::nav::grid(&refs, 0.8);
        let cover = alloc::vec![
            Cover {
                pos: [10.0, 0.0, 21.4],
                dir: [0.0, -1.0],
                high: 1
            },
            Cover {
                pos: [30.0, 0.0, 21.4],
                dir: [0.0, -1.0],
                high: 1
            },
            Cover {
                pos: [14.0, 0.0, 21.4],
                dir: [0.0, -1.0],
                high: 1
            }
        ];
        let spawns = alloc::vec![
            Spawn {
                pos: [20.0, 0.0, 4.0],
                yaw: PI
            },
            Spawn {
                pos: [8.0, 0.0, 36.0],
                yaw: 0.0
            },
            Spawn {
                pos: [32.0, 0.0, 36.0],
                yaw: 0.0
            },
            Spawn {
                pos: [20.0, 0.0, 38.0],
                yaw: 0.0
            }
        ];
        Yard {
            boxes,
            nav,
            flags,
            floor,
            cover,
            spawns,
        }
    }

    impl Yard {
        fn level(&self) -> Level<'_> {
            Level {
                solid: Bvh {
                    nodes: &self.boxes.nodes,
                    tris: &self.boxes.tris,
                    verts: &self.boxes.verts,
                },
                nav: self.nav,
                flags: &self.flags,
                floor: &self.floor,
                cover: &self.cover,
                spawns: &self.spawns,
            }
        }
    }

    #[test]
    fn a_match_starts_with_two_squads_far_from_the_player() {
        let yard = yard();
        let game = Game::new(yard.level(), reference(), 7);
        assert_eq!(game.standing(), 6);
        assert!(game
            .soldiers
            .iter()
            .filter(|s| s.alive())
            .all(|s| (s.pos - game.player.body.pos).len() > 18.0));
    }

    #[test]
    fn soldiers_find_the_player_and_the_match_goes_on_after_he_falls() {
        let yard = yard();
        let mut game = Game::new(yard.level(), reference(), 7);
        game.skill = 1.0;
        // He stands in the open in front of the gap and fires at nothing: they hear him and come.
        game.player.place(v3(20.0, 0.0, 12.0), PI);
        let (mut hurt, mut fell, mut back, mut shots) = (false, false, false, 0);
        for t in 0..60 * 90 {
            game.step(&Input {
                buttons: if t % 240 < 20 { btn::FIRE } else { 0 },
                ..Default::default()
            });
            for e in game.events.iter() {
                hurt |= matches!(e, Event::Hurt { .. });
                fell |= matches!(e, Event::Down { .. });
                back |= matches!(e, Event::Up);
                shots += matches!(
                    e,
                    Event::Shot {
                        soldier: Some(_),
                        ..
                    }
                ) as u32;
            }
            assert!(
                game.player.body.pos.y.is_finite()
                    && game
                        .soldiers
                        .iter()
                        .all(|s| s.pos.x.is_finite() && s.pos.z.is_finite())
            );
        }
        assert!(
            shots > 20 && hurt && fell && back,
            "shots {shots}, hurt {hurt}, fell {fell}, back {back}"
        );
        assert!(game.score[1] >= 1 && game.tally.deaths == game.score[1]);
    }

    #[test]
    fn the_player_fells_soldiers_and_more_come() {
        let yard = yard();
        let mut game = Game::new(yard.level(), reference(), 3);
        game.skill = 0.0;
        // Every round is aimed by the test at the nearest soldier's chest.
        let mut came = 6;
        for _ in 0..60 * 240 {
            let eye = game.player.head();
            let nearest = game
                .soldiers
                .iter()
                .filter(|s| s.alive())
                .min_by(|a, b| {
                    (a.pos - eye)
                        .len2()
                        .partial_cmp(&(b.pos - eye).len2())
                        .unwrap()
                })
                .copied();
            let mut input = Input::default();
            if let Some(s) = nearest {
                let to = s.pos + v3(0.0, if s.crouched { 0.9 } else { 1.3 }, 0.0) - eye;
                game.player.yaw = yaw_of(to);
                game.player.pitch = atan2(to.y, to.flat().len());
                if game.level.clear(eye, eye + to) {
                    input.buttons = btn::FIRE;
                }
            }
            game.step(&input);
            came = came.max(game.standing() + game.tally.kills as usize);
            if matches!(game.phase, Phase::Over { .. }) {
                break;
            }
        }
        assert!(game.tally.kills >= 6, "kills {}", game.tally.kills);
        assert!(came > 6, "more came after the first six");
        assert!(game.tally.hits <= game.tally.shots && game.score[0] == game.tally.kills);
    }
}
