//! Plays matches on a cooked pack with a player who aims at the nearest soldier in sight, and
//! says how long a tick takes: `cargo run --release -p duty-sim --example match -- <duty.pack> [minutes]`.

use std::time::Instant;

use duty_pack as pack;
use duty_sim::bvh::{Bvh, Node, Tri};
use duty_sim::game::{Game, Phase};
use duty_sim::level::Level;
use duty_sim::math::*;
use duty_sim::player::{btn, Input};
use duty_sim::tune::WeaponDef;
use duty_sim::weapon::reference;

fn records<T: Copy>(bytes: &[u8], tag: u32) -> Vec<T> {
    let s = pack::section(bytes, tag).unwrap_or(&[]);
    (0..s.len() / core::mem::size_of::<T>()).map(|i| pack::read::<T>(s, i * core::mem::size_of::<T>()).unwrap()).collect()
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("a pack");
    let minutes: f32 = args.next().and_then(|m| m.parse().ok()).unwrap_or(2.0);
    let bytes = std::fs::read(&path).expect("reading the pack");
    let (verts, tris, nodes): (Vec<[f32; 3]>, Vec<Tri>, Vec<Node>) = (records(&bytes, pack::COLV), records(&bytes, pack::COLT), records(&bytes, pack::COLN));
    // (aligned as the device holds it)
    let nav: Vec<u32> = pack::section(&bytes, pack::NAVG).unwrap().chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
    let nav_bytes = unsafe { core::slice::from_raw_parts(nav.as_ptr() as *const u8, nav.len() * 4) };
    let level = unsafe { Level::parse_nav(Bvh { nodes: &nodes, tris: &tris, verts: &verts }, nav_bytes) }.expect("the walk grid");
    let weapons: Vec<WeaponDef> = records(&bytes, pack::WEAP);
    let defs = if weapons.len() == 3 { [weapons[0], weapons[1], weapons[2]] } else { reference() };
    let mut game = Game::new(level, defs, 0x5eed_1234);
    let ticks = (minutes * 3600.0) as u32;
    if let Some(budget) = std::env::var("DUTY_PATHS").ok().and_then(|b| b.parse().ok()) {
        game.path_budget = budget;
    }
    let still = std::env::var("DUTY_STILL").is_ok();
    let watch = std::env::var("DUTY_WATCH").is_ok();
    let (mut worst, mut total) = (0.0f64, 0.0f64);
    let mut over = None;
    // On a place with floors over floors or ladders: the soldiers' steps by the height of their
    // feet (metres), and their steps and the player's on a ladder.
    let storeyed = !game.level.over.is_empty() || !game.level.links.is_empty();
    let (mut heights, mut ladders) = (std::collections::BTreeMap::<i32, u32>::new(), [0u32; 2]);
    // When the two sides first meet (the player has a soldier in clear sight), and how long the
    // soldiers wait for the pathfinder's answers: ticks a soldier waited, and the longest wait.
    let mut met: Option<u32> = None;
    let (mut waited, mut longest, mut waits) = (0u64, 0u32, [0u32; 16]);
    // The pathfinder's counts over every match of the run (a new match has a new pathfinder): ways
    // asked, found, not there, given up; ways with more corners than points; nodes opened, and the
    // most a search opened.
    let (mut ways, mut cut, mut opened) = ([0u32; 4], 0u32, (0u64, 0u32));
    for t in 0..ticks {
        let eye = game.player.head();
        let mut input = Input::default();
        let mut best: Option<(f32, V3)> = None;
        for s in game.soldiers.iter().filter(|s| s.alive()) {
            let to = s.pos + v3(0.0, if s.crouched { 0.9 } else { 1.3 }, 0.0) - eye;
            if best.is_none_or(|b| to.len2() < b.0) && game.level.clear(eye, eye + to) {
                best = Some((to.len2(), to));
            }
        }
        if best.is_some() && met.is_none() {
            met = Some(t);
        }
        for (k, s) in game.soldiers.iter().enumerate() {
            if s.alive() && s.way().1 {
                waits[k] += 1;
                waited += 1;
                longest = longest.max(waits[k]);
            } else {
                waits[k] = 0;
            }
        }
        if game.arms.magazine[game.arms.active] == 0 {
            input.buttons |= btn::RELOAD;
        }
        if still {
            input = Input::default();
        } else if let Some((_, to)) = best {
            let (yaw, pitch) = (yaw_of(to) - game.player.kick[1].value(), atan2(to.y, to.flat().len()) - game.player.kick[0].value());
            game.player.yaw = ease_angle(game.player.yaw, yaw, 14.0, 1.0 / 60.0);
            game.player.pitch = ease(game.player.pitch, pitch, 14.0, 1.0 / 60.0);
            if abs(wrap_angle(game.player.yaw - yaw)) < 0.02 && abs(game.player.pitch - pitch) < 0.02 {
                input.buttons |= btn::FIRE;
            }
        } else {
            // Nobody in sight: he walks on.
            input.my = 1.0;
            input.lx = if t / 240 % 2 == 0 { 0.25 } else { -0.1 };
        }
        if watch && t % 300 == 0 {
            let p = game.player.body.pos;
            println!(
                "{:5.0}s player ({:.0},{:.0},{:.0}) hp {:.0} | {}",
                t as f32 / 60.0,
                p.x,
                p.y,
                p.z,
                game.player.health,
                game.soldiers.iter().filter(|s| s.alive()).map(|s| format!("{:?} {:.0}m{}", s.state, (s.pos - p).len(), if s.visible { "*" } else { "" })).collect::<Vec<_>>().join(", ")
            );
            if std::env::var("DUTY_WAYS").is_ok() {
                for s in game.soldiers.iter().filter(|s| s.alive()) {
                    let (going, asked, len, at, stuck, goal) = s.way();
                    let node = game.level.node_at(s.pos.x, s.pos.z, s.pos.y);
                    println!("        at ({:.1},{:.1},{:.1}) node open {:?} floor {:?} | going {going} asked {asked} way {at}/{len} stuck {stuck:.1} goal ({:.1},{:.1},{:.1}) next {:?}", s.pos.x, s.pos.y, s.pos.z, node.map(|n| game.level.open(n)), node.map(|n| game.level.floor[n as usize]), goal.x, goal.y, goal.z, (len > 0).then(|| s.path_point()));
                }
            }
        }
        let from = Instant::now();
        game.step(&input);
        let took = from.elapsed().as_secs_f64() * 1e6;
        if storeyed {
            for s in game.soldiers.iter().filter(|s| s.alive()) {
                *heights.entry((s.pos.y + 0.5).floor() as i32).or_insert(0) += 1;
                ladders[0] += s.climbing as u32;
            }
            ladders[1] += game.player.ladder.is_some() as u32;
        }
        total += took;
        worst = worst.max(took);
        if let Phase::Over { won } = game.phase {
            // (the first match's end is the one reported; the rest are for the timing)
            if over.is_none() {
                over = Some((t, won, game.score, game.tally.kills, game.tally.deaths));
            }
            for (sum, count) in ways.iter_mut().zip(game.paths.counts) {
                *sum += count;
            }
            (cut, opened) = (cut + game.paths.cut, (opened.0 + game.paths.opened.0, opened.1.max(game.paths.opened.1)));
            game.restart();
        }
    }
    println!("ticks {ticks}: mean {:.1} us, worst {:.1} us a tick on this machine", total / ticks as f64, worst);
    for (sum, count) in ways.iter_mut().zip(game.paths.counts) {
        *sum += count;
    }
    (cut, opened) = (cut + game.paths.cut, (opened.0 + game.paths.opened.0, opened.1.max(game.paths.opened.1)));
    println!("ways asked, found, not there, given up: {ways:?}; {cut} with more corners than points; nodes a search opened: {:.0} on average, {} at most", opened.0 as f64 / (ways[1] + ways[2] + ways[3]).max(1) as f64, opened.1);
    println!("the sides first meet at {:.1} s; soldiers waited for a way {} ticks in all ({:.1} a way asked), {} at the longest", met.map_or(-1.0, |t| t as f32 / 60.0), waited, waited as f64 / ways[0].max(1) as f64, longest);
    if storeyed {
        println!("soldiers' steps by the height of their feet (m): {heights:?}; on a ladder: {} of theirs, {} of the player's", ladders[0], ladders[1]);
    }
    println!("score {}:{}, kills {}, deaths {}, shots {}, hits {}, standing {}, over {:?}", game.score[0], game.score[1], game.tally.kills, game.tally.deaths, game.tally.shots, game.tally.hits, game.standing(), over);
}
