//! What a way costs on a pack's walk grid: a way between every two starts, the calls of 300 nodes
//! each took, and the time on this machine.
//!
//!     cargo run --release -p duty-sim --example ways -- <duty.pack>

use std::time::Instant;

use duty_pack as pack;
use duty_sim::bvh::Bvh;
use duty_sim::level::Level;
use duty_sim::nav::Pathfinder;

fn main() {
    let path = std::env::args().nth(1).expect("a pack");
    let bytes = std::fs::read(&path).expect("the pack");
    let section = pack::section(&bytes, pack::NAVG).expect("the pack's walk grid");
    let nav: Vec<u32> = section.chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
    let nav_bytes = unsafe { core::slice::from_raw_parts(nav.as_ptr() as *const u8, nav.len() * 4) };
    let level = unsafe { Level::parse_nav(Bvh { nodes: &[], tris: &[], verts: &[] }, nav_bytes) }.expect("the walk grid");
    let ends: Vec<u32> = level.spawns.iter().filter_map(|s| level.nearest_open(s.pos[0], s.pos[2], s.pos[1])).collect();
    let mut best = f64::MAX;
    let (mut ways, mut found, mut calls, mut points) = (0u32, 0u32, 0u32, 0u32);
    // (the best of seven rounds, or of `DUTY_ROUNDS`)
    for round in 0..std::env::var("DUTY_ROUNDS").ok().and_then(|n| n.parse().ok()).unwrap_or(7) {
        let mut finder = Pathfinder::new(level.flags.len());
        let began = Instant::now();
        for (i, from) in ends.iter().enumerate() {
            for (j, to) in ends.iter().enumerate() {
                if i == j || from == to {
                    continue;
                }
                finder.ask(0, *from, *to);
                loop {
                    if round == 0 {
                        calls += 1;
                    }
                    if let Some((_, way)) = finder.step(&level, 300) {
                        if round == 0 {
                            ways += 1;
                            found += (way.len > 0) as u32;
                            points += way.len as u32;
                        }
                        break;
                    }
                }
            }
        }
        best = best.min(began.elapsed().as_secs_f64());
    }
    println!("walk grid: {} bytes in the pack, {} nodes, {} bytes of the pathfinder's own", section.len(), level.flags.len(), level.flags.len() * 6);
    println!("ways between {} starts: {ways} asked, {found} found, {points} points, {calls} calls of 300 nodes; {:.1} us a way, {:.2} us a call on this machine", ends.len(), best * 1e6 / ways.max(1) as f64, best * 1e6 / calls.max(1) as f64);
}
