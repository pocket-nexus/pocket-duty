//! A frame of a pack, drawn on this computer the way the GE draws it: one texture a draw, its
//! texel times the vertex's colour times two, the ground once more under its shadow picture, the
//! sky behind. It reads the pack's own sections, so it shows what the device will be given.

use duty_pack::{self as pack, chunk, texture, Chunk, Cluster, Scene, Texture, ViewCell, WorldVertex};
use duty_sim::clip::Guard;
use duty_sim::mat::{self, Mat4};
use duty_sim::math::{forward, v3, V3};
use duty_sim::view::{self, Draw};

use crate::tex::{fetch, Image};

pub struct Camera {
    pub eye: V3,
    pub yaw: f32,
    pub pitch: f32,
    /// Degrees, top to bottom.
    pub fov: f32,
}

pub struct Frame {
    pub image: Image,
    pub counts: view::Counts,
    /// Triangles that reached the screen with at least one pixel on top.
    pub seen: usize,
    /// Triangles sent at each level of detail, and of them the ones that showed.
    pub by_level: [(u32, u32); 3],
    /// Triangles sent per picture.
    pub by_texture: Vec<u32>,
    /// Per chunk drawn: its number, level, triangles sent and showing.
    pub by_chunk: Vec<(u16, u8, u32, u32)>,
}

fn records<T: Copy>(pack_bytes: &[u8], tag: u32) -> Vec<T> {
    let bytes = pack::section(pack_bytes, tag).unwrap_or(&[]);
    (0..bytes.len() / core::mem::size_of::<T>()).map(|i| pack::read(bytes, i * core::mem::size_of::<T>()).unwrap()).collect()
}

/// A texel between its four neighbours.
fn sample(record: &Texture, texels: &[u8], u: f32, v: f32) -> [f32; 4] {
    let clamp = record.flags & texture::CLAMP != 0;
    let (w, h) = (record.width as f32, record.height as f32);
    let (x, y) = (u * w - 0.5, v * h - 0.5);
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let at = |i: i32, j: i32| {
        let (i, j) = if clamp { (i.clamp(0, w as i32 - 1), j.clamp(0, h as i32 - 1)) } else { (i, j) };
        fetch(record, texels, 0, i, j)
    };
    let (a, b, c, d) = (at(x0 as i32, y0 as i32), at(x0 as i32 + 1, y0 as i32), at(x0 as i32, y0 as i32 + 1), at(x0 as i32 + 1, y0 as i32 + 1));
    core::array::from_fn(|k| (a[k] * (1.0 - fx) + b[k] * fx) * (1.0 - fy) + (c[k] * (1.0 - fx) + d[k] * fx) * fy)
}

#[derive(Clone, Copy)]
struct Point {
    clip: [f32; 4],
    uv: [f32; 2],
    colour: [f32; 3],
    world: [f32; 3],
}

fn mix(a: &Point, b: &Point, t: f32) -> Point {
    Point {
        clip: core::array::from_fn(|k| a.clip[k] + (b.clip[k] - a.clip[k]) * t),
        uv: core::array::from_fn(|k| a.uv[k] + (b.uv[k] - a.uv[k]) * t),
        colour: core::array::from_fn(|k| a.colour[k] + (b.colour[k] - a.colour[k]) * t),
        world: core::array::from_fn(|k| a.world[k] + (b.world[k] - a.world[k]) * t),
    }
}

pub fn render(pack_bytes: &[u8], camera: &Camera, width: usize, height: usize, all_chunks: bool) -> Frame {
    let scene: Scene = pack::read(pack::section(pack_bytes, pack::SCEN).expect("scene"), 0).expect("scene");
    let textures: Vec<Texture> = records(pack_bytes, pack::TEXT);
    let texels = pack::section(pack_bytes, pack::TEXD).unwrap_or(&[]);
    let chunks: Vec<Chunk> = records(pack_bytes, pack::CHNK);
    let vertices: Vec<WorldVertex> = records(pack_bytes, pack::WVTX);
    let indices: Vec<u16> = records(pack_bytes, pack::WIDX);
    let cells: Vec<ViewCell> = records(pack_bytes, pack::VCEL);
    let lists: Vec<u16> = records(pack_bytes, pack::VLST);

    let look = forward(camera.yaw, camera.pitch);
    let near = 0.12;
    let vp: Mat4 = mat::mul(&mat::perspective(camera.fov, width as f32 / height as f32, near, 600.0), &mat::view(camera.eye, look, 0.0));
    let planes = mat::unit_planes(&mat::planes(&vp));
    let clusters: Vec<Cluster> = records(pack_bytes, pack::CLUS);
    let bits = pack::section(pack_bytes, pack::VBIT).unwrap_or(&[]);
    let world = view::World { scene: &scene, chunks: &chunks, clusters: &clusters, cells: &cells, lists: &lists, bits };
    let guard = Guard::new(&vp, near, 480.0, 272.0, 2048.0);
    let mut draws: Vec<Draw> = Vec::new();
    let counts = view::select(&world, &planes, camera.eye, &guard, &view::Options { nearer: 1.0, everything: all_chunks }, &mut draws);
    // What is solid first, then the glass.
    draws.sort_by_key(|d| chunks[d.chunk as usize].flags & chunk::BLEND != 0);

    let pick: Option<(usize, usize)> = std::env::var("DUTY_PICK").ok().and_then(|t| t.split_once(',').and_then(|(x, y)| Some((x.parse().ok()?, y.parse().ok()?))));
    let mut colour = vec![[0.0f32; 3]; width * height];
    let mut depth = vec![0.0f32; width * height];
    let mut seen = 0usize;
    let mut by_level = [(0u32, 0u32); 3];
    let mut by_texture = vec![0u32; textures.len()];
    let mut by_chunk: Vec<(u16, u8, u32, u32)> = Vec::new();
    let shadow = textures.get(scene.shadow_texture as usize);
    for d in &draws {
        let c = &chunks[d.chunk as usize];
        let lod = &c.lod[d.lod as usize];
        let centre = scene.cell_centre(c.cell as u32);
        let record = &textures[c.texture as usize];
        by_level[d.lod as usize].0 += d.count / 3;
        by_texture[c.texture as usize] += d.count / 3;
        if by_chunk.last().map(|b| b.0) != Some(d.chunk) {
            by_chunk.push((d.chunk, d.lod, 0, 0));
        }
        by_chunk.last_mut().unwrap().2 += d.count / 3;
        for t in 0..d.count as usize / 3 {
            let corner = |k: usize| {
                let v = &vertices[lod.vertex as usize + indices[(lod.index + d.first) as usize + t * 3 + k] as usize];
                let world: [f32; 3] = core::array::from_fn(|a| centre[a] + v.pos[a] as f32 * scene.quantum);
                let clip: [f32; 4] = core::array::from_fn(|r| vp[r * 4] * world[0] + vp[r * 4 + 1] * world[1] + vp[r * 4 + 2] * world[2] + vp[r * 4 + 3]);
                Point {
                    clip,
                    uv: [v.uv[0] as f32 / 32768.0 * c.uv_scale[0] + c.uv_offset[0], v.uv[1] as f32 / 32768.0 * c.uv_scale[1] + c.uv_offset[1]],
                    colour: [(v.colour & 31) as f32 / 31.0, ((v.colour >> 5) & 63) as f32 / 63.0, (v.colour >> 11) as f32 / 31.0],
                    world,
                }
            };
            // Cut at the near plane.
            let mut poly: Vec<Point> = vec![corner(0), corner(1), corner(2)];
            let mut cut: Vec<Point> = Vec::with_capacity(4);
            for i in 0..poly.len() {
                let (a, b) = (&poly[i], &poly[(i + 1) % poly.len()]);
                let (da, db) = (a.clip[3] - near, b.clip[3] - near);
                if da >= 0.0 {
                    cut.push(*a);
                }
                if (da >= 0.0) != (db >= 0.0) {
                    cut.push(mix(a, b, da / (da - db)));
                }
            }
            poly = cut;
            if poly.len() < 3 {
                continue;
            }
            let screen: Vec<[f32; 3]> = poly.iter().map(|p| [(p.clip[0] / p.clip[3] * 0.5 + 0.5) * width as f32, (0.5 - p.clip[1] / p.clip[3] * 0.5) * height as f32, 1.0 / p.clip[3]]).collect();
            let mut drew = false;
            for i in 1..poly.len() - 1 {
                let (p, s) = ([&poly[0], &poly[i], &poly[i + 1]], [screen[0], screen[i], screen[i + 1]]);
                let area = (s[1][0] - s[0][0]) * (s[2][1] - s[0][1]) - (s[2][0] - s[0][0]) * (s[1][1] - s[0][1]);
                // (the screen's y runs down: a face turned to the eye has a negative area here)
                if area == 0.0 || (area > 0.0 && c.flags & chunk::TWO_SIDED == 0) {
                    continue;
                }
                let (x0, x1) = (s.iter().map(|v| v[0]).fold(f32::MAX, f32::min).max(0.0) as usize, (s.iter().map(|v| v[0]).fold(f32::MIN, f32::max).ceil().min(width as f32)) as usize);
                let (y0, y1) = (s.iter().map(|v| v[1]).fold(f32::MAX, f32::min).max(0.0) as usize, (s.iter().map(|v| v[1]).fold(f32::MIN, f32::max).ceil().min(height as f32)) as usize);
                for y in y0..y1 {
                    for x in x0..x1 {
                        let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                        let edge = |a: [f32; 3], b: [f32; 3]| (b[0] - a[0]) * (py - a[1]) - (b[1] - a[1]) * (px - a[0]);
                        let (w0, w1, w2) = (edge(s[1], s[2]) / area, edge(s[2], s[0]) / area, edge(s[0], s[1]) / area);
                        if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                            continue;
                        }
                        let inv = w0 * s[0][2] + w1 * s[1][2] + w2 * s[2][2];
                        let at = y * width + x;
                        if inv <= depth[at] {
                            continue;
                        }
                        // Perspective: the corners' values over their depths.
                        let (b0, b1, b2) = (w0 * s[0][2] / inv, w1 * s[1][2] / inv, w2 * s[2][2] / inv);
                        let uv = [p[0].uv[0] * b0 + p[1].uv[0] * b1 + p[2].uv[0] * b2, p[0].uv[1] * b0 + p[1].uv[1] * b1 + p[2].uv[1] * b2];
                        let texel = sample(record, texels, uv[0], uv[1]);
                        if c.flags & chunk::CUTOUT != 0 && texel[3] < 0.5 {
                            continue;
                        }
                        let mut out: [f32; 3] = core::array::from_fn(|k| ((p[0].colour[k] * b0 + p[1].colour[k] * b1 + p[2].colour[k] * b2) * texel[k] * 2.0).min(1.0));
                        if c.flags & chunk::GROUND != 0 {
                            if let Some(shadow) = shadow {
                                let w: [f32; 3] = core::array::from_fn(|k| p[0].world[k] * b0 + p[1].world[k] * b1 + p[2].world[k] * b2);
                                let r = &scene.shadow_rows;
                                let s = sample(shadow, texels, w[0] * r[0][0] + w[2] * r[0][1] + r[0][2], w[0] * r[1][0] + w[2] * r[1][1] + r[1][2]);
                                out = core::array::from_fn(|k| (out[k] * s[k] * 2.0).min(1.0));
                            }
                        }
                        if pick == Some((x, y)) {
                            println!("pick {x},{y}: chunk {} lod {} texture {} flags {} cell {}, vertex colours {:?} {:?} {:?}, texel {:?}, out {:?}, world {:?}", d.chunk, d.lod, c.texture, c.flags, c.cell, p[0].colour, p[1].colour, p[2].colour, texel, out, p[0].world);
                        }
                        if c.flags & chunk::BLEND != 0 {
                            let a = c.opacity as f32 / 255.0;
                            colour[at] = core::array::from_fn(|k| colour[at][k] * (1.0 - a) + out[k] * a);
                        } else {
                            colour[at] = out;
                            depth[at] = inv;
                        }
                        drew = true;
                    }
                }
            }
            seen += drew as usize;
            by_level[d.lod as usize].1 += drew as u32;
            by_chunk.last_mut().unwrap().3 += drew as u32;
        }
    }
    // The sky where nothing was drawn.
    let right = look.cross(V3::UP).norm_or(v3(1.0, 0.0, 0.0));
    let up = right.cross(look);
    let tan = (camera.fov.to_radians() * 0.5).tan();
    let zenith = [(scene.zenith & 255) as f32 / 255.0, ((scene.zenith >> 8) & 255) as f32 / 255.0, ((scene.zenith >> 16) & 255) as f32 / 255.0];
    for y in 0..height {
        for x in 0..width {
            let at = y * width + x;
            if depth[at] > 0.0 {
                continue;
            }
            let (sx, sy) = (((x as f32 + 0.5) / width as f32 * 2.0 - 1.0) * tan * width as f32 / height as f32, (1.0 - (y as f32 + 0.5) / height as f32 * 2.0) * tan);
            let d = (look + right * sx + up * sy).norm();
            let elevation = d.y.asin();
            let around = (d.z.atan2(d.x) / core::f32::consts::TAU).rem_euclid(1.0) * scene.sky_columns as f32;
            let v = (scene.sky_top - elevation) / (scene.sky_top - scene.sky_floor);
            colour[at] = if scene.sky_columns == 0 || v < 0.0 {
                zenith
            } else {
                let column = (around as usize).min(scene.sky_columns as usize - 1);
                let s = sample(&textures[scene.sky_texture as usize + column], texels, around - column as f32, v.min(1.0));
                // The top rows meet the zenith's colour.
                let fade = (v * 12.0).min(1.0);
                core::array::from_fn(|k| zenith[k] + (s[k] - zenith[k]) * fade)
            };
        }
    }
    Frame { image: Image { width, height, data: colour.iter().map(|c| [c[0], c[1], c[2], 1.0]).collect() }, counts, seen, by_level, by_texture, by_chunk }
}
