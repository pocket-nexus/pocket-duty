//! What a way costs on a pack's walk grid: a way between every two starts and between pairs of
//! open nodes drawn from one seed, the nodes each search opened, the game's steps it took (120
//! nodes a step, `Game::path_budget`), the way's length, and the time on this machine.
//!
//!     cargo run --release -p duty-sim --example ways -- <duty.pack>
//!
//! `DUTY_PAIRS` is how many pairs are drawn (400), `DUTY_ROUNDS` how many times the ways are
//! timed (7, the best counts), `DUTY_LEAN` the sixteenths of the distance left that a search's
//! estimate is (`nav::LEAN`).

use std::time::Instant;

use duty_pack as pack;
use duty_sim::bvh::Bvh;
use duty_sim::level::Level;
use duty_sim::nav::Pathfinder;

/// The game's nodes a step.
const BUDGET: u32 = 120;

fn main() {
    let path = std::env::args().nth(1).expect("a pack");
    let bytes = std::fs::read(&path).expect("the pack");
    let section = pack::section(&bytes, pack::NAVG).expect("the pack's walk grid");
    let nav: Vec<u32> = section.chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
    let nav_bytes = unsafe { core::slice::from_raw_parts(nav.as_ptr() as *const u8, nav.len() * 4) };
    let level = unsafe { Level::parse_nav(Bvh { nodes: &[], tris: &[], verts: &[] }, nav_bytes) }.expect("the walk grid");
    let number = |name: &str| std::env::var(name).ok().and_then(|n| n.parse::<u32>().ok());
    let ends: Vec<u32> = level.spawns.iter().filter_map(|s| level.nearest_open(s.pos[0], s.pos[2], s.pos[1])).collect();
    let mut pairs: Vec<(u32, u32)> = Vec::new();
    for (i, from) in ends.iter().enumerate() {
        for (j, to) in ends.iter().enumerate() {
            if i != j && from != to {
                pairs.push((*from, *to));
            }
        }
    }
    let between = pairs.len();
    // Pairs of open nodes, the same for a pack whatever the pathfinder is.
    let open: Vec<u32> = (0..level.flags.len() as u32).filter(|n| level.open(*n)).collect();
    let mut seed = 0x5eed_1234u32;
    let mut draw = || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        open[(seed >> 8) as usize % open.len()]
    };
    for _ in 0..number("DUTY_PAIRS").unwrap_or(400) {
        let (from, to) = (draw(), draw());
        if from != to {
            pairs.push((from, to));
        }
    }
    let mut best = f64::MAX;
    let (mut found, mut cut, mut metres, mut held) = (0u32, 0u32, 0.0f64, 0usize);
    let (mut opened, mut steps, mut points): (Vec<u32>, Vec<u32>, Vec<u32>) = (Vec::new(), Vec::new(), Vec::new());
    // The longest single step of a search, of every round's least (a step that ends a search
    // also pulls its way straight).
    let mut longest = f64::MAX;
    for round in 0..number("DUTY_ROUNDS").unwrap_or(7) {
        let mut finder = Pathfinder::new(level.flags.len());
        if let Some(lean) = number("DUTY_LEAN") {
            finder.lean = lean;
        }
        let began = Instant::now();
        let mut slowest = 0.0f64;
        for (from, to) in &pairs {
            let before = finder.opened.0;
            finder.ask(0, *from, *to);
            let mut calls = 0;
            let way = loop {
                calls += 1;
                let from = Instant::now();
                let done = finder.step(&level, BUDGET);
                slowest = slowest.max(from.elapsed().as_secs_f64());
                if let Some((_, way)) = done {
                    break way;
                }
            };
            if round == 0 {
                opened.push((finder.opened.0 - before) as u32);
                steps.push(calls);
                found += (way.len > 0) as u32;
                points.push(way.len as u32);
                let mut at = level.node_pos(*from);
                for p in &way.points[..way.len as usize] {
                    metres += (*p - at).len() as f64;
                    at = *p;
                }
            }
        }
        if round == 0 {
            (cut, held) = (finder.cut, finder.bytes());
        }
        best = best.min(began.elapsed().as_secs_f64());
        longest = longest.min(slowest);
    }
    let ways = pairs.len().max(1);
    let shares = |list: &mut Vec<u32>| {
        list.sort_unstable();
        (list.iter().map(|n| *n as u64).sum::<u64>() as f64 / ways as f64, list[ways / 2], list[ways * 95 / 100], list[ways - 1])
    };
    let (o, s, p) = (shares(&mut opened), shares(&mut steps), shares(&mut points));
    println!("walk grid: {} bytes in the pack, {} nodes ({} open), {held} bytes of the pathfinder's own after these ways", section.len(), level.flags.len(), open.len());
    println!("ways: {between} between {} starts and {} between drawn nodes; {found} found, {cut} with more corners than points, {:.1} m a way", ends.len(), ways - between, metres / found.max(1) as f64);
    println!("nodes a search opened: mean {:.0}, half under {}, 95% under {}, most {}", o.0, o.1, o.2, o.3);
    println!("steps of {BUDGET} nodes a way: mean {:.1}, half under {}, 95% under {}, most {}", s.0, s.1, s.2, s.3);
    println!("points a way: mean {:.1}, half under {}, 95% under {}, most {}", p.0, p.1, p.2, p.3);
    println!("{:.1} us a way, {:.2} us a step, {:.1} us the longest step on this machine", best * 1e6 / ways as f64, best * 1e6 / steps.iter().map(|n| *n as u64).sum::<u64>().max(1) as f64, longest * 1e6);
}
