//! What the simulation reads from the pack beside the collision world: the walk grid with its
//! cover and spawn points (`NAVG`), and the weapons' numbers (`WEAP`).

use std::path::Path;

use duty_pack::bytes_of;
use duty_sim::bvh::Bvh;
use duty_sim::level::{Cover, NavHeader, Spawn};
use duty_sim::math::v3;
use duty_sim::tune::{mode, WeaponDef, PATTERN};
use serde_json::Value;

use crate::ir::Dir;

fn pad(bytes: &mut Vec<u8>) {
    bytes.resize((bytes.len() + 3) & !3, 0);
}

/// The `NAVG` section. A spawn point stands on the solid ground under it.
pub fn nav(ir: &Path, scene: &Value, solid: &Bvh) -> Result<Vec<u8>, String> {
    let dir = Dir::open(&ir.join("nav.dir"))?;
    let m = &dir.meta;
    let (nx, nz) = (m["nx"].as_u64().ok_or("nav.dir: no size")? as u32, m["nz"].as_u64().ok_or("nav.dir: no size")? as u32);
    let (flags, floor, cover) = (dir.u8s("flags"), dir.f32s("floor"), dir.f32s("cover"));
    let spawns = scene["spawns"].as_array().ok_or("scene.json: no spawns")?;
    let mut header = NavHeader { cell: m["cell"].as_f64().unwrap_or(0.8) as f32, min_x: m["minX"].as_f64().unwrap_or(0.0) as f32, min_z: m["minZ"].as_f64().unwrap_or(0.0) as f32, nx, nz, cover: (cover.len() / 8) as u32, spawns: spawns.len() as u32, pad: 0 };
    // A node the reference's floor ray never reached is closed.
    let mut flags: Vec<u8> = flags.iter().zip(&floor).map(|(f, y)| if y.is_finite() { *f } else { 0 }).collect();
    let heights: Vec<i16> = floor.iter().map(|y| if y.is_finite() { (y * 100.0).round().clamp(-32768.0, 32767.0) as i16 } else { 0 }).collect();
    // The grid holds one height a node, the top surface's: a roof and an upper floor are islands no
    // step of 45 cm reaches. A soldier sent to one, or standing on one, finds no way at all, and
    // asks again. So only the largest part that hangs together stays open: every way asked for
    // on it exists.
    // (a place marks a stair's nodes with 4: a step to or from one may be twice a knee)
    let stair: Vec<bool> = flags.iter().map(|f| f & 4 != 0).collect();
    let step = |a: usize, b: usize| (heights[a] as i32 - heights[b] as i32).abs() <= if stair[a] || stair[b] { 90 } else { 45 };
    let mut part = vec![u32::MAX; flags.len()];
    let mut sizes: Vec<usize> = Vec::new();
    for start in 0..flags.len() {
        if flags[start] == 0 || part[start] != u32::MAX {
            continue;
        }
        let (mut stack, mut size) = (vec![start], 0usize);
        part[start] = sizes.len() as u32;
        while let Some(node) = stack.pop() {
            size += 1;
            let (i, j) = ((node % nx as usize) as i64, (node / nx as usize) as i64);
            for (di, dj) in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
                let (ni, nj) = (i + di, j + dj);
                if ni < 0 || nj < 0 || ni >= nx as i64 || nj >= nz as i64 {
                    continue;
                }
                let next = (nj * nx as i64 + ni) as usize;
                if flags[next] != 0 && part[next] == u32::MAX && step(node, next) {
                    part[next] = sizes.len() as u32;
                    stack.push(next);
                }
            }
        }
        sizes.push(size);
    }
    let main = (0..sizes.len()).max_by_key(|i| sizes[*i]).ok_or("nav.dir: no open node")? as u32;
    let open = flags.iter().filter(|f| **f != 0).count();
    for (f, p) in flags.iter_mut().zip(&part) {
        if *p != main {
            *f = 0;
        }
    }
    // The open node at a place, or the nearest one within `reach` nodes of it.
    let near = |x: f32, z: f32, reach: i64| -> Option<usize> {
        let (ci, cj) = (((x - header.min_x) / header.cell + 0.5).floor() as i64, ((z - header.min_z) / header.cell + 0.5).floor() as i64);
        let mut best: Option<(i64, usize)> = None;
        for dj in -reach..=reach {
            for di in -reach..=reach {
                let (i, j) = (ci + di, cj + dj);
                if i < 0 || j < 0 || i >= nx as i64 || j >= nz as i64 {
                    continue;
                }
                let node = (j * nx as i64 + i) as usize;
                if flags[node] != 0 && best.is_none_or(|b| di * di + dj * dj < b.0) {
                    best = Some((di * di + dj * dj, node));
                }
            }
        }
        best.map(|b| b.1)
    };
    let place = |node: usize| [header.min_x + (node % nx as usize) as f32 * header.cell, heights[node] as f32 * 0.01, header.min_z + (node / nx as usize) as f32 * header.cell];
    // Cover a soldier cannot walk to is no cover.
    let all = cover.len() / 8;
    let cover: Vec<Cover> = cover.chunks_exact(8).filter(|c| near(c[0], c[2], 0).is_some_and(|n| (heights[n] as f32 * 0.01 - c[1]).abs() < 1.4)).map(|c| Cover { pos: [c[0], c[1], c[2]], dir: [c[3], c[4]], high: (c[5] > 0.5) as u32 }).collect();
    header.cover = cover.len() as u32;
    let mut moved = 0;
    let spawns: Vec<Spawn> = spawns
        .iter()
        .map(|s| {
            let p = |i: usize| s["position"][i].as_f64().unwrap_or(0.0) as f32;
            let ground = solid.raycast(v3(p(0), p(1) + 1.2, p(2)), v3(0.0, -1.0, 0.0), 4.0, 0).map_or(p(1), |hit| p(1) + 1.2 - hit.t);
            // A start off the part that stays open moves to the nearest node of it.
            let pos = match near(p(0), p(2), 0) {
                Some(n) if (heights[n] as f32 * 0.01 - ground).abs() < 0.6 => [p(0), ground, p(2)],
                _ => {
                    moved += 1;
                    near(p(0), p(2), 12).map_or([p(0), ground, p(2)], place)
                }
            };
            Spawn { pos, yaw: s["yaw"].as_f64().unwrap_or(0.0) as f32 }
        })
        .collect();
    println!("walk grid: {} of {open} open nodes hang together and stay open ({} parts in all); {} of {all} places of cover are on them; {moved} of {} starts moved onto them", sizes[main as usize], sizes.len(), cover.len(), spawns.len());
    let mut out = bytes_of(&[header]).to_vec();
    out.extend(&flags);
    pad(&mut out);
    out.extend_from_slice(bytes_of(&heights));
    pad(&mut out);
    out.extend_from_slice(bytes_of(&cover));
    out.extend_from_slice(bytes_of(&spawns));
    Ok(out)
}

/// The `WEAP` section: the rifle, the submachine gun, the pistol.
pub fn weapons(scene: &Value) -> Result<Vec<WeaponDef>, String> {
    let all = scene["weapons"].as_array().ok_or("scene.json lists no weapons: export again")?;
    ["rifle", "smg", "pistol"]
        .iter()
        .map(|id| {
            let w = all.iter().find(|w| w["id"] == *id).ok_or(format!("scene.json: no weapon {id}"))?;
            let d = &w["def"];
            let n = |v: &Value| v.as_f64().unwrap_or(0.0) as f32;
            let mut label = [0u8; 12];
            let name = d["label"].as_str().unwrap_or(id).as_bytes();
            label[..name.len().min(11)].copy_from_slice(&name[..name.len().min(11)]);
            let mut modes = [0u32; 3];
            let list = d["modes"].as_array().ok_or("a weapon without fire modes")?;
            for (slot, m) in modes.iter_mut().zip(list) {
                *slot = match m.as_str() {
                    Some("auto") => mode::AUTO,
                    Some("burst") => mode::BURST,
                    _ => mode::SEMI,
                };
            }
            let length = (d["recoil"]["patternLength"].as_u64().unwrap_or(1) as usize).min(PATTERN);
            let mut pattern = [[0.0f32; 2]; PATTERN];
            for (i, shot) in pattern.iter_mut().enumerate().take(length) {
                *shot = [n(&w["pattern"][i * 2]), n(&w["pattern"][i * 2 + 1])];
            }
            Ok(WeaponDef {
                label,
                mode: modes,
                modes: list.len().min(3) as u32,
                rpm: n(&d["rpm"]),
                burst_rpm: n(&d["burstRpm"]),
                burst_delay: n(&d["burstDelay"]),
                burst_count: d["burstCount"].as_u64().unwrap_or(1) as u32,
                magazine: d["magSize"].as_u64().unwrap_or(1) as u32,
                reserve: d["reserve"].as_u64().unwrap_or(0) as u32,
                damage: n(&d["damage"]),
                dropoff: n(&d["dropoff"]),
                max_range: n(&d["maxRange"]),
                penetration: n(&d["penetration"]),
                tracer_every: d["tracerEvery"].as_u64().unwrap_or(3) as u32,
                spread_hip: n(&d["spreadHip"]),
                spread_ads: n(&d["spreadAds"]),
                spread_shot: n(&d["spreadPerShot"]),
                spread_max: n(&d["spreadMax"]),
                spread_decay: n(&d["spreadDecay"]),
                kick_back: n(&d["recoil"]["kickBack"]),
                kick_up: n(&d["recoil"]["kickUp"]),
                kick_roll: n(&d["recoil"]["roll"]),
                punch: n(&d["recoil"]["punch"]),
                ads_time: n(&d["adsTime"]),
                reload_tac: n(&d["reloadTac"]),
                reload_empty: n(&d["reloadEmpty"]),
                draw_time: n(&d["drawTime"]),
                holster_time: n(&d["holsterTime"]),
                pattern_length: length as u32,
                pattern,
            })
        })
        .collect()
}
