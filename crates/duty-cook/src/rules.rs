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
    let header = NavHeader { cell: m["cell"].as_f64().unwrap_or(0.8) as f32, min_x: m["minX"].as_f64().unwrap_or(0.0) as f32, min_z: m["minZ"].as_f64().unwrap_or(0.0) as f32, nx, nz, cover: (cover.len() / 8) as u32, spawns: spawns.len() as u32, pad: 0 };
    let mut out = bytes_of(&[header]).to_vec();
    // A node the reference's floor ray never reached is closed.
    out.extend(flags.iter().zip(&floor).map(|(f, y)| if y.is_finite() { *f } else { 0 }));
    pad(&mut out);
    let heights: Vec<i16> = floor.iter().map(|y| if y.is_finite() { (y * 100.0).round().clamp(-32768.0, 32767.0) as i16 } else { 0 }).collect();
    out.extend_from_slice(bytes_of(&heights));
    pad(&mut out);
    let cover: Vec<Cover> = cover.chunks_exact(8).map(|c| Cover { pos: [c[0], c[1], c[2]], dir: [c[3], c[4]], high: (c[5] > 0.5) as u32 }).collect();
    out.extend_from_slice(bytes_of(&cover));
    let spawns: Vec<Spawn> = spawns
        .iter()
        .map(|s| {
            let p = |i: usize| s["position"][i].as_f64().unwrap_or(0.0) as f32;
            let ground = solid.raycast(v3(p(0), p(1) + 1.2, p(2)), v3(0.0, -1.0, 0.0), 4.0, 0).map_or(p(1), |hit| p(1) + 1.2 - hit.t);
            Spawn { pos: [p(0), ground, p(2)], yaw: s["yaw"].as_f64().unwrap_or(0.0) as f32 }
        })
        .collect();
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
