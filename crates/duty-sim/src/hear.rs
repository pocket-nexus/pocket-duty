//! What a match sounds like: each event of a step as the sounds a device plays
//! for it, how loud in each ear.

use duty_pack::sound::Id;

use crate::event::Event;
use crate::game::Game;
use crate::math::*;

/// One sound to start: gains of the two sides and the pitch out of 256, and the rank a mixer
/// with every voice taken weighs it by.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Play {
    pub id: Id,
    pub left: u16,
    pub right: u16,
    pub pitch: u16,
    pub priority: u8,
}

/// How loud a sound at a place is in each ear, of 256: nearer is louder, and what is to the right
/// is louder on the right (`web/src/audio/spatial.js:194-198`).
pub fn heard(at: V3, eye: V3, yaw: f32, level: f32) -> (u16, u16) {
    let to = at - eye;
    let d = to.len();
    let gain = level * clamp(2.0 / (2.0 + 0.85 * max(0.0, d - 2.0)), 0.04, 1.0);
    let side = to.flat().norm().dot(v3(cos(yaw), 0.0, -sin(yaw)));
    ((gain * (1.0 - 0.45 * max(side, 0.0)) * 256.0) as u16, (gain * (1.0 + 0.45 * min(side, 0.0)) * 256.0) as u16)
}

/// What an event sounds like. `count` is the caller's, one for the match: it picks between a
/// sound's takes.
pub fn sound(game: &Game, event: &Event, count: &mut u32, mut play: impl FnMut(Play)) {
    let (eye, yaw) = (game.player.head(), game.player.yaw);
    *count = count.wrapping_add(1);
    let n = *count;
    let take = |a: Id, b: Id| if n & 1 == 0 { a } else { b };
    let at = |id: Id, pos: V3, level: f32, priority: u8| {
        let (left, right) = heard(pos, eye, yaw, level);
        Play { id, left, right, pitch: 256, priority }
    };
    let near = |id: Id, level: f32, priority: u8| Play { id, left: (level * 256.0) as u16, right: (level * 256.0) as u16, pitch: 256, priority };
    match *event {
        Event::Shot { soldier: None, weapon, .. } => {
            let takes = [[Id::RifleShot0, Id::RifleShot1, Id::RifleShot2], [Id::SmgShot0, Id::SmgShot1, Id::SmgShot2], [Id::PistolShot0, Id::PistolShot1, Id::PistolShot2]];
            play(near(takes[weapon as usize % 3][(n % 3) as usize], 0.9, 9));
        }
        Event::Shot { soldier: Some(_), from, .. } => {
            let far = (from - eye).len() > 26.0;
            play(at(if far { take(Id::EnemyShotFar0, Id::EnemyShotFar1) } else { take(Id::EnemyShotNear0, Id::EnemyShotNear1) }, from, 1.0, 7));
        }
        Event::Impact { pos, surface, .. } => {
            if (pos - eye).len2() < 40.0 * 40.0 {
                let id = match surface {
                    1 => take(Id::ImpactMetal0, Id::ImpactMetal1),
                    2 => take(Id::ImpactWood0, Id::ImpactWood1),
                    3 | 4 => take(Id::ImpactDirt0, Id::ImpactDirt1),
                    _ => take(Id::ImpactConcrete0, Id::ImpactConcrete1),
                };
                play(at(id, pos, 0.8, 3));
            }
        }
        Event::Whizz { pos } => play(at(take(Id::Whizz0, Id::Whizz1), pos, 0.9, 5)),
        Event::Dry => play(near(Id::DryFire, 0.6, 6)),
        Event::Reload { step: 1 } => play(near(Id::ReloadOut, 0.6, 6)),
        Event::Reload { step: 2 } => play(near(Id::ReloadIn, 0.6, 6)),
        Event::Reload { step: 3 } => play(near(Id::ReloadBolt, 0.6, 6)),
        Event::Draw { .. } => play(near(Id::WeaponDraw, 0.5, 4)),
        Event::Footstep { surface, running } => {
            let id = match surface {
                1 => take(Id::StepMetal0, Id::StepMetal1),
                2 => take(Id::StepWood0, Id::StepWood1),
                3 | 4 => take(Id::StepDirt0, Id::StepDirt1),
                _ => take(Id::StepConcrete0, Id::StepConcrete1),
            };
            play(near(id, if running { 0.5 } else { 0.32 }, 1));
        }
        Event::Land { speed, .. } => play(near(Id::Land, clamp(speed / 7.0, 0.35, 1.0) * 0.6, 3)),
        Event::Struck { zone, killed, pos, .. } => {
            play(near(if killed { Id::Kill } else if zone == 0 { Id::Headshot } else { Id::Hitmarker }, 0.7, 8));
            play(at(take(Id::ImpactFlesh0, Id::ImpactFlesh1), pos, 0.7, 4));
        }
        Event::Hurt { .. } => play(near(Id::Hurt, 0.8, 8)),
        Event::Bark { kind, pos, .. } => {
            let id = match kind {
                0 => take(Id::BarkContact0, Id::BarkContact1),
                1 => Id::BarkReload,
                2 => Id::BarkHit,
                _ => take(Id::BarkDeath0, Id::BarkDeath1),
            };
            play(at(id, pos, 1.0, 5));
        }
        Event::Fell { pos, .. } => play(at(Id::BodyFall, pos, 0.8, 3)),
        Event::Bounce { pos } => play(at(Id::GrenadeBounce, pos, 0.9, 5)),
        Event::Blast { pos } => play(at(Id::Explosion, pos, 1.4, 10)),
        _ => {}
    }
}

/// The bench's player: he turns to the nearest soldier in sight, holding the sights against the
/// weapon's climb, and says whether to fire (they are on) and whether to change the magazine.
pub fn bench_aim(game: &mut Game, seconds: f32) -> (bool, bool) {
    let eye = game.player.head();
    let mut best: Option<(f32, V3)> = None;
    for s in game.soldiers.iter().filter(|s| s.alive()) {
        let to = s.pos + v3(0.0, if s.crouched { 0.9 } else { 1.3 }, 0.0) - eye;
        if best.is_none_or(|b| to.len2() < b.0) && game.level.clear(eye, eye + to) {
            best = Some((to.len2(), to));
        }
    }
    let reload = game.arms.magazine[game.arms.active] == 0;
    let mut fire = false;
    if let Some((_, to)) = best {
        let (yaw, pitch) = (yaw_of(to) - game.player.kick[1].value(), atan2(to.y, to.flat().len()) - game.player.kick[0].value());
        game.player.yaw = ease_angle(game.player.yaw, yaw, 14.0, seconds);
        game.player.pitch = ease(game.player.pitch, pitch, 14.0, seconds);
        fire = abs(wrap_angle(game.player.yaw - yaw)) < 0.02 && abs(game.player.pitch - pitch) < 0.02;
    }
    (fire, reload)
}
