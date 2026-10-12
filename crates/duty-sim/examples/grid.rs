//! Says how the cooked walk grid hangs together: its connected parts, and which part each start
//! and each place of cover is in. `cargo run --release -p duty-sim --example grid -- <duty.pack>`

use duty_pack as pack;
use duty_sim::bvh::Bvh;
use duty_sim::level::Level;

fn main() {
    let path = std::env::args().nth(1).expect("a pack");
    let bytes = std::fs::read(&path).expect("reading the pack");
    let nav: Vec<u32> = pack::section(&bytes, pack::NAVG).unwrap().chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
    let nav_bytes = unsafe { core::slice::from_raw_parts(nav.as_ptr() as *const u8, nav.len() * 4) };
    let level = unsafe { Level::parse_nav(Bvh { nodes: &[], tris: &[], verts: &[] }, nav_bytes) }.expect("the walk grid");
    let (nx, nz) = (level.nav.nx as i32, level.nav.nz as i32);
    let count = (nx * nz) as usize;
    let mut part = vec![u32::MAX; count];
    let mut sizes: Vec<usize> = Vec::new();
    for start in 0..count {
        if !level.open(start as u32) || part[start] != u32::MAX {
            continue;
        }
        let id = sizes.len() as u32;
        let (mut stack, mut size) = (vec![start as u32], 0usize);
        part[start] = id;
        while let Some(node) = stack.pop() {
            size += 1;
            let (i, j) = (node as i32 % nx, node as i32 / nx);
            for (di, dj) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (ni, nj) = (i + di, j + dj);
                if ni < 0 || nj < 0 || ni >= nx || nj >= nz {
                    continue;
                }
                let next = (nj * nx + ni) as u32;
                // (both ways: a step down is a step up coming back)
                if level.passable(node, next) && level.open(node) && part[next as usize] == u32::MAX {
                    part[next as usize] = id;
                    stack.push(next);
                }
            }
        }
        sizes.push(size);
    }
    let open: usize = sizes.iter().sum();
    let mut order: Vec<usize> = (0..sizes.len()).collect();
    order.sort_by_key(|i| std::cmp::Reverse(sizes[*i]));
    println!("{nx} x {nz} nodes of {} m, {open} open in {} parts; largest: {:?}", level.nav.cell, sizes.len(), order.iter().take(8).map(|i| (*i, sizes[*i])).collect::<Vec<_>>());
    let of = |x: f32, z: f32, y: f32| level.nearest_open(x, z, y).map(|n| (part[n as usize], level.floor[n as usize]));
    for (i, s) in level.spawns.iter().enumerate() {
        println!("start {i}: {:?} -> part and floor {:?}", s.pos, of(s.pos[0], s.pos[2], s.pos[1]));
    }
    let mut cover = std::collections::BTreeMap::new();
    for c in level.cover {
        *cover.entry(of(c.pos[0], c.pos[2], c.pos[1]).map(|p| p.0)).or_insert(0usize) += 1;
    }
    println!("cover by part: {cover:?}");
    let mut floors = std::collections::BTreeMap::new();
    for n in 0..count {
        if level.open(n as u32) {
            *floors.entry(level.floor[n] / 50).or_insert(0usize) += 1;
        }
    }
    println!("open nodes by floor height (half metres): {floors:?}");
}
