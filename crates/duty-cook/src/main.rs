//! Pocket Duty's compiler.
//!
//!   duty-cook cook --in IR --out DIR --profile profiles/psp30.json
//!   duty-cook shot --in IR --out DIR --profile P [--eye x,y,z] [--yaw deg] [--pitch deg] [--fov deg]
//!                  [--size 960x544] [--png FILE] [--all]
//!
//! `cook` lowers the DutyIR to `DIR/duty.pack` and writes `DIR/receipt.json`. `shot` draws a frame
//! of that pack on this computer.

mod audio;
mod chunks;
mod collision;
mod figure_shot;
mod figures;
mod fx;
mod ground;
mod ir;
mod light;
mod lod;
mod preview;
mod pvs;
mod rules;
mod sky;
mod tex;
mod world;

use std::path::{Path, PathBuf};
use std::time::Instant;

use duty_pack::{self as pack, texture, Scene};
use duty_sim::math::v3;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

fn number(v: &Value, fallback: f64) -> f32 {
    v.as_f64().unwrap_or(fallback) as f32
}

fn abgr(c: [f32; 3]) -> u32 {
    let b = |x: f32| (x.clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
    0xff00_0000 | b(c[2]) << 16 | b(c[1]) << 8 | b(c[0])
}

fn cook(ir: &Path, out: &Path, profile: &Value) -> Result<(), String> {
    let t0 = Instant::now();
    let scene_json: Value = serde_json::from_slice(&std::fs::read(ir.join("scene.json")).map_err(|e| format!("scene.json: {e} (run `bun tools/duty.ts export` first)"))?).map_err(|e| e.to_string())?;
    let drop: Vec<String> = profile["drop"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
    let world = world::load(ir, &drop)?;
    let kept: usize = world.soups.iter().map(|s| s.idx.len() / 3).sum();
    println!("world: {} triangles in the export, {} kept in {} materials ({:.1} s)", world.source_triangles, kept, world.materials.len(), t0.elapsed().as_secs_f32());

    // ---- the grid
    let bounds = |key: &str, axis: usize| number(&scene_json["bounds"][key][axis], 0.0);
    let cell = number(&profile["cell"], 8.0);
    let min = [bounds("min", 0).floor(), bounds("min", 1), bounds("min", 2).floor()];
    let (nx, nz) = (((bounds("max", 0) - min[0]) / cell).ceil() as usize, ((bounds("max", 2) - min[2]) / cell).ceil() as usize);
    let grid = lod::Grid { min: [min[0], min[2]], cell, nx, nz };
    let list = |v: &Value, fallback: [f32; 3]| -> [f32; 3] { core::array::from_fn(|i| number(&v[i], fallback[i] as f64)) };
    let settings = lod::Settings {
        error: list(&profile["lod"]["error"], [0.035, 0.16, 0.45]),
        // (a place's floors lie at any height and have surfaces of the map's own: its scene names them)
        ground_families: [&profile["ground"]["families"], &scene_json["ground"]["families"]].iter().filter_map(|list| list.as_array()).flatten().filter_map(|v| v.as_str().map(String::from)).collect(),
        ground_below: number(&scene_json["ground"]["below"], number(&profile["ground"]["below"], 0.75) as f64),
        min_area: list(&profile["lod"]["minArea"], [0.0, 0.0, 0.0]),
    };
    let t1 = Instant::now();
    let pieces = lod::pieces(&world, &grid, &settings);
    let per_level: [usize; 3] = core::array::from_fn(|l| pieces.iter().map(|p| p.lods[l].len()).sum());
    println!("levels: {} pieces of {} × {} cells, triangles {:?} ({:.1} s)", pieces.len(), nx, nz, per_level, t1.elapsed().as_secs_f32());

    // ---- the pictures
    let mut texels = tex::Texels { pica: profile["textures"]["format"].as_str() == Some("pica"), rgba: profile["textures"]["format"].as_str() == Some("rgba"), ..Default::default() };
    let side_default = profile["textures"]["side"].as_u64().unwrap_or(128) as usize;
    let levels = profile["textures"]["levels"].as_u64().unwrap_or(4) as usize;
    let mut family_texture: Vec<(String, u16)> = Vec::new();
    let mut texture_of = Vec::new();
    let mut texture_flags = Vec::new();
    for m in &world.materials {
        let at = match family_texture.iter().find(|f| f.0 == m.family) {
            Some(f) => f.1,
            None => {
                let path = ir.join(format!("tex_{}.png", m.family));
                let side = profile["textures"]["sides"][&m.family].as_u64().unwrap_or(side_default as u64) as usize;
                let image = if path.exists() {
                    // (never larger than the reference bakes it: a profile's side is the most a picture keeps)
                    let source = tex::Image::open(&path)?;
                    let mut image = source.reduced(side.min(source.width), side.min(source.height));
                    // A texel of the tile's mean colour becomes one half: the vertex carries the mean, lit.
                    let mean = m.mean.map(|c| c.powf(1.0 / 2.2).max(1e-3));
                    for p in &mut image.data {
                        for k in 0..3 {
                            p[k] = (0.5 * p[k] / mean[k]).min(1.0);
                        }
                        if !m.cutout {
                            p[3] = 1.0;
                        }
                    }
                    image
                } else {
                    let mut image = tex::Image::new(16, 16);
                    image.data.fill([0.5, 0.5, 0.5, 1.0]);
                    image
                };
                let at = texels.add(&image, &tex::Options { levels, flags: if m.cutout { texture::CUTOUT } else { 0 }, dither: false }) as u16;
                family_texture.push((m.family.clone(), at));
                at
            }
        };
        texture_of.push(at);
        texture_flags.push(if m.cutout { pack::chunk::CUTOUT } else { 0 });
    }

    // ---- the light
    let t2 = Instant::now();
    let tone = light::Tone::open(ir)?;
    let sky = light::Sky::open(ir)?;
    let mut lighting = light::Lighting::new(&world, sky, profile["light"]["rays"].as_u64().unwrap_or(64) as usize);
    if scene_json["ground"].is_object() {
        lighting.floors(world.materials.iter().map(|m| settings.ground_families.contains(&m.family)).collect(), settings.ground_below);
    }
    // A place states its own light beside the sun and the sky: `light` in its scene.
    if scene_json["light"].is_object() {
        let stated = &scene_json["light"];
        let lamps = stated["lamps"].as_array().map(|all| all.iter().map(|l| light::Lamp { at: { let p = list(&l["at"], [0.0; 3]); v3(p[0], p[1], p[2]) }, colour: list(&l["colour"], [1.0, 0.7, 0.4]), reach: number(&l["reach"], 10.0) }).collect()).unwrap_or_default();
        lighting.place(light::PlaceLight { sky: list(&stated["sky"], [1.0; 3]), bounce: list(&stated["bounce"], [0.0; 3]), lamps });
        println!("light: the place's own: sky x {:?}, bounce {:?}, {} lamps", list(&stated["sky"], [1.0; 3]), list(&stated["bounce"], [0.0; 3]), stated["lamps"].as_array().map_or(0, |a| a.len()));
    }
    println!("light: {} triangles stand in its way, sun {:.3} {:.3} {:.3} at {:.1} h ({:.1} s)", lighting.triangles(), lighting.sky.sun.x, lighting.sky.sun.y, lighting.sky.sun.z, lighting.sky.hour, t2.elapsed().as_secs_f32());

    let mut scene = Scene {
        min,
        cell,
        max: [min[0] + nx as f32 * cell, bounds("max", 1), min[2] + nz as f32 * cell],
        quantum: number(&profile["quantum"], 1.0 / 1024.0),
        cells_x: nx as u32,
        cells_z: nz as u32,
        origin_y: ((bounds("min", 1) + bounds("max", 1)) * 0.5).round(),
        lod: [number(&profile["lod"]["distance"][0], 16.0), number(&profile["lod"]["distance"][1], 44.0)],
        sun: [lighting.sky.sun.x, lighting.sky.sun.y, lighting.sky.sun.z],
        ..Default::default()
    };
    let spawn = &scene_json["spawns"][0];
    scene.start = [number(&spawn["position"][0], 0.0), number(&spawn["position"][1], 0.0), number(&spawn["position"][2], 0.0), number(&spawn["yaw"], 0.0)];

    let level = ground::Level { origin: list(&scene_json["level"]["origin"], [0.0; 3]), axis_x: list(&scene_json["level"]["axisX"], [1.0, 0.0, 0.0]), axis_z: list(&scene_json["level"]["axisZ"], [0.0, 0.0, 1.0]) };
    let t3 = Instant::now();
    let built = chunks::build(
        &pieces,
        &world,
        &scene,
        &lighting,
        &tone,
        &chunks::Settings {
            open_floor: number(&profile["light"]["openFloor"], 0.3),
            leaves_both: profile["light"]["leaves"].as_str() == Some("both"),
            texture_of,
            texture_flags,
            largest: list(&profile["lod"]["largest"], [4.0, 8.0, 0.0]),
            shadow_edge: list(&profile["lod"]["shadowEdge"], [0.6, 2.5, 0.0]),
            large_edge: number(&profile["clip"]["largeEdge"], 0.9),
            cluster_triangles: profile["cluster"]["triangles"].as_u64().unwrap_or(32) as usize,
            cone_weight: number(&profile["cluster"]["cone"], 0.25),
            square: number(&profile["cluster"]["square"], 0.0),
            level_axes: [level.axis_x, level.axis_z],
        },
    );
    println!("chunks: {} chunks in {} clusters, {} vertices, triangles {:?} (large {:?}), light at {} points ({:.1} s)", built.chunks.len(), built.clusters.len(), built.vertices.len(), built.triangles, built.large, built.lit_points, t3.elapsed().as_secs_f32());

    if let Ok(text) = std::env::var("DUTY_PROBE") {
        // "x,y,z,dx,dy,dz": what a ray from the point meets, and the light on that surface.
        for point in text.split(';') {
            let p: Vec<f32> = point.split(',').filter_map(|c| c.parse().ok()).collect();
            let (from, d) = (v3(p[0], p[1], p[2]), v3(p[3], p[4], p[5]).norm());
            match lighting.first(from, d) {
                None => println!("probe {point}: nothing"),
                Some((t, m)) => {
                    let at = from + d * t;
                    let n = -d;
                    println!("probe {point}: {} at {:.2} ({:.2} {:.2} {:.2}); sun seen 3 cm off {:.2}, 10 cm off {:.2}; blocker {:?}", world.materials[m].name.split('|').next().unwrap_or(""), t, at.x, at.y, at.z, lighting.sun_seen(at + n * 0.03, 0.012, 4), lighting.sun_seen(at + n * 0.1, 0.012, 4), lighting.first(at + n * 0.03, lighting.sky.sun).map(|h| (h.0, world.materials[h.1].name.split('|').next().unwrap_or("").to_string())));
                }
            }
        }
    }

    // ---- what can be seen from where
    let t5 = Instant::now();
    let collision = collision::Collision::open(ir)?;
    let sight_settings = pvs::Settings { rays: std::env::var("DUTY_RAYS").ok().and_then(|n| n.parse().ok()).unwrap_or(profile["sight"]["rays"].as_u64().unwrap_or(400_000) as usize), near: number(&profile["sight"]["near"], 3.0) };
    // The sight pass is nine tenths of a cook and follows from the geometry alone: two profiles
    // with the same chunks (the PSP's and the 3DS's) share one, kept beside the packs under the
    // hash of everything it reads.
    let sight_key = {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(pack::bytes_of(&[scene.min, scene.max]));
        h.update(scene.cell.to_le_bytes());
        h.update(pack::bytes_of(&built.chunks));
        h.update(pack::bytes_of(&built.clusters));
        h.update(pack::bytes_of(&built.vertices));
        h.update(pack::bytes_of(&built.indices));
        h.update(pack::bytes_of(&collision.verts));
        h.update(pack::bytes_of(&collision.tris));
        h.update((sight_settings.rays as u64).to_le_bytes());
        h.update(sight_settings.near.to_le_bytes());
        h.finalize().iter().take(8).map(|b| format!("{b:02x}")).collect::<String>()
    };
    let sight_file = out.parent().unwrap_or(out).join(format!("sight-{sight_key}.bin"));
    let sight = match pvs::Sight::read(&sight_file) {
        Some(sight) => {
            println!("sight: taken from {}", sight_file.display());
            sight
        }
        None => {
            let sight = pvs::compute(&scene, &built.chunks, &built.clusters, &built.vertices, &built.indices, &collision, &sight_settings);
            sight.write(&sight_file).map_err(|e| format!("{}: {e}", sight_file.display()))?;
            sight
        }
    };
    println!("sight: {} cells an eye can be in; a cell sees {} of {} chunks and {} of {} clusters on average ({:.1} s); collision {} triangles, {} nodes", sight.open, sight.chunks, built.chunks.len(), sight.clusters, built.clusters.len(), t5.elapsed().as_secs_f32(), collision.tris.len(), collision.nodes.len());

    // ---- the ground's shadow and the sky
    let t4 = Instant::now();
    // What open ground is: the mean of the flat, low faces of the ground's materials.
    let (mut albedo, mut ambient, mut count) = ([0.0f64; 3], [0.0f64; 3], 0u64);
    for soup in world.soups.iter().filter(|s| settings.ground_families.contains(&world.materials[s.material].family)) {
        for v in soup.verts.iter().filter(|v| v.nrm[1] > 0.94 && v.pos[1] < settings.ground_below && v.amb[0] >= 0.0) {
            for k in 0..3 {
                albedo[k] += (v.col[k] as f64).powf(2.2);
                ambient[k] += v.amb[k] as f64;
            }
            count += 1;
        }
    }
    let mean = |sum: [f64; 3]| sum.map(|s| (s / count.max(1) as f64) as f32);
    let gain = lighting.sky_gain();
    let shadow = ground::shadow(&lighting, &tone, &level, number(&profile["ground"]["shadow"]["extent"], 128.0), profile["ground"]["shadow"]["side"].as_u64().unwrap_or(512) as usize, mean(albedo), core::array::from_fn(|k| mean(ambient)[k] * gain[k]));
    scene.shadow_texture = texels.add(&shadow.image, &tex::Options { levels: 1, flags: texture::CLAMP, dither: false });
    scene.shadow_rows = shadow.rows;
    let s = &profile["sky"];
    let band = sky::band(&lighting.sky, &tone, s["columns"].as_u64().unwrap_or(4) as usize, s["width"].as_u64().unwrap_or(256) as usize, s["height"].as_u64().unwrap_or(128) as usize, number(&s["floor"], -0.12), number(&s["top"], 0.95));
    scene.sky_columns = band.columns.len() as u32;
    scene.sky_floor = band.floor;
    scene.sky_top = band.top;
    scene.zenith = abgr(band.zenith);
    // The haze: the reference's fog takes 0.00145 of the light in a metre at its base and thins with
    // height (`heightScale` 18 m above -2 m), which at the eye's height leaves about 0.0012 a metre:
    // 11 % at 100 m. The GE's fog is a line between two distances; one from the eye to 880 m is within
    // a hundredth of that curve out to the level's far side. Its colour is the sky's just above the
    // horizon, all the way round.
    {
        let mut sum = [0.0f32; 3];
        for i in 0..32 {
            let a = i as f32 / 32.0 * core::f32::consts::TAU;
            let c = tone.display(lighting.sky.soft(v3(a.cos() * 0.998, 0.06, a.sin() * 0.998)));
            for k in 0..3 {
                sum[k] += c[k] / 32.0;
            }
        }
        scene.haze = [0.0, 880.0];
        scene.haze_colour = abgr(sum);
    }
    for (i, image) in band.columns.iter().enumerate() {
        let at = texels.add(image, &tex::Options { levels: 1, flags: texture::CLAMP, dither: true });
        if i == 0 {
            scene.sky_texture = at;
        }
    }
    scene.fx_texture = texels.add_ramp(&fx::picture(32), texture::CLAMP);
    scene.level_origin = level.origin;
    scene.level_x = level.axis_x;
    scene.level_z = level.axis_z;
    // What a soldier or a held weapon is lit by: the device shades a figure's own colour by
    // `sky_colour + sun_colour × facing`, in the display's terms. The two are what the tone table
    // makes of a mid grey in the open ground's fill, and under the sun on top of it.
    let grey = 0.18f32;
    // The open ground's fill is the whole sky. A standing figure's sides see half of it, and the
    // ground for the rest: the ground's own colour under that sky and, where the sun reaches (about
    // half of what lies round a figure in these streets), under the sun.
    let open: [f32; 3] = core::array::from_fn(|k| mean(ambient)[k] / mean(albedo)[k].max(1e-3));
    let fill: [f32; 3] = core::array::from_fn(|k| 0.5 * open[k] + 0.5 * mean(albedo)[k] * (open[k] + 0.5 * lighting.sky.sun_colour[k] * core::f32::consts::FRAC_1_PI * lighting.sky.sun.y.max(0.0)));
    let shade = tone.display(core::array::from_fn(|k| grey * fill[k]));
    let lit = tone.display(core::array::from_fn(|k| grey * (fill[k] + lighting.sky.sun_colour[k] * core::f32::consts::FRAC_1_PI)));
    let base = grey.powf(1.0 / 2.2);
    scene.sky_colour = shade.map(|c| c / base);
    scene.sun_colour = core::array::from_fn(|k| (lit[k] - shade[k]) / base);
    println!("shadow and sky: {} pictures, {} KiB of texels ({:.1} s)", texels.records.len(), texels.bytes.len() / 1024, t4.elapsed().as_secs_f32());

    // ---- what the match runs on
    let nav = rules::nav(ir, &scene_json, &collision.bvh())?;
    let weapons = rules::weapons(&scene_json)?;

    // ---- the figures and the sounds
    let figure_bytes = figures::cook(ir, profile)?;
    let (sounds, samples) = audio::cook(ir, profile)?;
    println!("figures: {} KiB; sounds: {} in {} KiB", figure_bytes.len() / 1024, sounds.len(), samples.len() / 1024);

    // ---- the pack
    let mut writer = pack::Writer::default();
    writer.add(pack::SCEN, &[scene]);
    writer.add(pack::TEXT, &texels.records);
    writer.add(pack::TEXD, &texels.bytes);
    writer.add(pack::CHNK, &built.chunks);
    writer.add(pack::WVTX, &built.vertices);
    writer.add(pack::WIDX, &built.indices);
    writer.add(pack::CLUS, &built.clusters);
    // A device whose rasterizer cuts a triangle at every side of the view asks nothing of the large ones.
    if profile["clip"]["records"].as_bool() != Some(false) {
        writer.add(pack::LTRI, &built.larges);
        writer.add(pack::LTRO, &built.larges_at);
    }
    writer.add(pack::VCEL, &sight.cells);
    writer.add(pack::VLST, &sight.lists);
    writer.add(pack::VBIT, &sight.bits);
    writer.add(pack::COLV, &collision.verts);
    writer.add(pack::COLT, &collision.tris);
    writer.add(pack::COLN, &collision.nodes);
    writer.add(pack::NAVG, &nav);
    writer.add(pack::WEAP, &weapons);
    if !figure_bytes.is_empty() {
        writer.add(pack::FIGS, &figure_bytes);
    }
    if !sounds.is_empty() {
        writer.add(pack::SNDS, &sounds);
        writer.add(pack::SNDD, &samples);
    }
    let bytes = writer.finish();
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    std::fs::write(out.join("duty.pack"), &bytes).map_err(|e| e.to_string())?;
    let sha: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
    let receipt = json!({
        "pack": { "bytes": bytes.len(), "sha256": sha, "version": pack::VERSION },
        "profile": profile,
        "source": { "triangles": world.source_triangles, "kept": kept, "dropped": world.dropped.iter().map(|d| json!({ "name": d.0, "triangles": d.1 })).collect::<Vec<_>>() },
        "world": { "cells": [nx, nz], "chunks": built.chunks.len(), "vertices": built.vertices.len(), "triangles": built.triangles, "large": built.large, "litPoints": built.lit_points },
        "textures": { "count": texels.records.len(), "bytes": texels.bytes.len() },
        "sight": { "cells": sight.open, "chunksSeen": sight.chunks, "clustersSeen": sight.clusters, "clusters": built.clusters.len() },
        "collision": { "triangles": collision.tris.len(), "vertices": collision.verts.len(), "nodes": collision.nodes.len(), "surfaces": collision.surfaces },
        "seconds": t0.elapsed().as_secs_f32(),
    });
    std::fs::write(out.join("receipt.json"), serde_json::to_string_pretty(&receipt).unwrap()).map_err(|e| e.to_string())?;
    // ---- the level from straight above, for the interface's map: north up, a pixel every `metres`
    let half = number(&profile["map"]["half"], 80.0);
    let side = profile["map"]["side"].as_u64().unwrap_or(256) as usize;
    let (cx, cz) = (level.origin[0], level.origin[2]);
    let height = 500.0f32;
    let above = preview::render(&bytes, &preview::Camera { eye: v3(cx, height, cz), yaw: 0.0, pitch: -core::f32::consts::FRAC_PI_2, fov: 2.0 * (half / height).atan().to_degrees() }, side, side, true);
    above.image.save(&out.join("map.png"))?;
    std::fs::write(out.join("map.json"), serde_json::to_string_pretty(&json!({ "side": side, "x0": cx - half, "z0": cz - half, "metres": 2.0 * half / side as f32 })).unwrap()).map_err(|e| e.to_string())?;
    println!("pack: {} KiB, sha256 {} ({:.1} s in all)", bytes.len() / 1024, &sha[..12], t0.elapsed().as_secs_f32());
    Ok(())
}

fn shot(out: &Path, argv: &[String]) -> Result<(), String> {
    let value = |flag: &str| argv.iter().position(|a| a == flag).and_then(|i| argv.get(i + 1)).cloned();
    let bytes = std::fs::read(out.join("duty.pack")).map_err(|e| format!("{}: {e} (run `bun tools/duty.ts cook` first)", out.join("duty.pack").display()))?;
    let scene: Scene = pack::read(pack::section(&bytes, pack::SCEN).ok_or("the pack has another version")?, 0).ok_or("scene")?;
    let eye = match value("--eye") {
        Some(text) => {
            let p: Vec<f32> = text.split(',').filter_map(|c| c.parse().ok()).collect();
            v3(p[0], p[1], p[2])
        }
        None => v3(scene.start[0], scene.start[1] + 1.66, scene.start[2]),
    };
    let yaw = value("--yaw").and_then(|v| v.parse::<f32>().ok()).map_or(scene.start[3], f32::to_radians);
    let pitch = value("--pitch").and_then(|v| v.parse::<f32>().ok()).map_or(0.0, f32::to_radians);
    let fov = value("--fov").and_then(|v| v.parse().ok()).unwrap_or(80.0);
    let size: Vec<usize> = value("--size").unwrap_or("960x544".into()).split('x').filter_map(|c| c.parse().ok()).collect();
    let frame = preview::render(&bytes, &preview::Camera { eye, yaw, pitch, fov }, size[0], size[1], argv.iter().any(|a| a == "--all"));
    let png = PathBuf::from(value("--png").unwrap_or(out.join("shot.png").to_string_lossy().into_owned()));
    frame.image.save(&png)?;
    let c = &frame.counts;
    println!("  chunks: {} listed, {} in the view; clusters: {} of them, {} unseen from the cell, {} outside the view, {} turned away, {} drawn ({} to test, {} large triangles)", c.listed, c.chunks, c.clusters, c.unseen, c.outside, c.turned, c.drawn, c.cut, c.large);
    println!("  by level (sent, showing): {:?}", frame.by_level);
    let f = frame.fate;
    println!("  of the triangles sent: {} behind the eye, {} turned away, {} off the screen, {} between pixels, {} behind what was drawn, {} drawn", f[0], f[1], f[2], f[3], f[4], f[5]);
    if argv.iter().any(|a| a == "--chunks") {
        let chunks: Vec<pack::Chunk> = { let b = pack::section(&bytes, pack::CHNK).unwrap(); (0..b.len() / core::mem::size_of::<pack::Chunk>()).map(|i| pack::read(b, i * core::mem::size_of::<pack::Chunk>()).unwrap()).collect() };
        let mut rows = frame.by_chunk.clone();
        rows.sort_by_key(|r| core::cmp::Reverse(r.2));
        for (id, lod, sent, showing) in rows.iter().take(24) {
            let c = &chunks[*id as usize];
            println!("    chunk {id} texture {} flags {} level {lod}: {sent} sent, {showing} showing; box {:.1} {:.1} {:.1} … {:.1} {:.1} {:.1}", c.texture, c.flags, c.min[0], c.min[1], c.min[2], c.max[0], c.max[1], c.max[2]);
        }
    }
    println!("{}: eye {:.1} {:.1} {:.1}, {} draws, {} triangles sent ({} showing)", png.display(), eye.x, eye.y, eye.z, c.draws, c.triangles, frame.seen);
    Ok(())
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let value = |flag: &str, fallback: &str| argv.iter().position(|a| a == flag).and_then(|i| argv.get(i + 1)).cloned().unwrap_or(fallback.into());
    let (ir, out) = (PathBuf::from(value("--in", ".pocket-build/duty/ir")), PathBuf::from(value("--out", ".pocket-build/duty/psp30")));
    let result = match argv.first().map(String::as_str) {
        Some("cook") => std::fs::read(value("--profile", "profiles/psp30.json")).map_err(|e| format!("profile: {e}")).and_then(|b| serde_json::from_slice::<Value>(&b).map_err(|e| format!("profile: {e}"))).and_then(|profile| cook(&ir, &out, &profile)),
        Some("shot") => shot(&out, &argv),
        Some("figures") => figure_shot::run(&ir, &out, &value("--profile", "profiles/psp30.json")),
        _ => Err("usage: duty-cook <cook|shot> --in IR --out DIR --profile FILE".into()),
    };
    if let Err(e) = result {
        eprintln!("duty-cook: {e}");
        std::process::exit(1);
    }
}
