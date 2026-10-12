//! What a frame draws from one place, and what choosing it takes on this computer.
//! `cargo run --release -p duty-sim --example view -- <duty.pack> x y z yaw pitch [nearer] [fov]`
//! (the eye's place, its turn and tilt in degrees, as the device's status prints them).

use duty_pack::{self as pack, Chunk, Cluster, Scene, ViewCell, WorldVertex};
use duty_sim::clip::Guard;
use duty_sim::mat;
use duty_sim::math::{forward, v3};
use duty_sim::view::{self, Options, World};

fn records<T: Copy>(bytes: &[u8], tag: u32) -> Vec<T> {
    let s = pack::section(bytes, tag).expect("a section");
    (0..s.len() / core::mem::size_of::<T>()).map(|i| pack::read::<T>(s, i * core::mem::size_of::<T>()).unwrap()).collect()
}

fn main() {
    let mut args = std::env::args().skip(1);
    let bytes = std::fs::read(args.next().expect("a pack")).expect("reading the pack");
    let n: Vec<f32> = args.map(|a| a.parse().expect("a number")).collect();
    let scene: Scene = records(&bytes, pack::SCEN)[0];
    let (chunks, clusters, cells): (Vec<Chunk>, Vec<Cluster>, Vec<ViewCell>) = (records(&bytes, pack::CHNK), records(&bytes, pack::CLUS), records(&bytes, pack::VCEL));
    let (lists, bits): (Vec<u16>, Vec<u8>) = (records(&bytes, pack::VLST), records(&bytes, pack::VBIT));
    let larges_at: Vec<u32> = records(&bytes, pack::LTRO);
    let world = World { scene: &scene, chunks: &chunks, clusters: &clusters, larges_at: &larges_at, cells: &cells, lists: &lists, bits: &bits };
    let mut by_cell = view::Cells::new(&scene, &chunks);
    let eye = v3(n[0], n[1], n[2]);
    let (yaw, pitch) = (n[3].to_radians(), n[4].to_radians());
    let (nearer, fov) = (n.get(5).copied().unwrap_or(1.0), n.get(6).copied().unwrap_or(65.0));
    let vp = mat::mul(&mat::perspective(fov, 480.0 / 272.0, 0.15, 420.0), &mat::view(eye, forward(yaw, pitch), 0.0));
    let planes = mat::unit_planes(&mat::planes(&vp));
    let guard = Guard::new(&vp, 0.15, 480.0, 272.0, 2048.0).boxes_along(&scene.level_x, &scene.level_z);
    let mut draws = Vec::new();
    // `DUTY_ALL=1`: every cluster in the view, whatever the cell lists (a pack cooked with few rays).
    let options = Options { nearer, everything: std::env::var("DUTY_ALL").is_ok() };
    let counts = view::select(&world, &mut by_cell, &planes, eye, &guard, &options, &mut draws);
    let from = std::time::Instant::now();
    for _ in 0..2000 {
        view::select(&world, &mut by_cell, &planes, eye, &guard, &options, &mut draws);
    }
    println!("{counts:?}");
    println!("{} draws, {:.1} us a choice here", draws.len(), from.elapsed().as_secs_f64() * 1e6 / 2000.0);

    // The faces turned away, by the cluster they are in: all of a cluster's faces turned away is
    // what a test of the cluster could have known.
    {
        let vertices: Vec<WorldVertex> = records(&bytes, pack::WVTX);
        let indices: Vec<u16> = records(&bytes, pack::WIDX);
        let (mut sent, mut turned, mut whole, mut flat, mut by_cutoff) = (0usize, 0usize, 0usize, 0usize, [0usize; 5]);
        for d in &draws {
            let c = &chunks[d.chunk as usize];
            if c.flags & duty_pack::chunk::TWO_SIDED != 0 {
                sent += d.count as usize / 3;
                continue;
            }
            let lod = c.lod(d.lod as usize);
            let centre = scene.cell_centre(c.cell as u32);
            for id in lod.cluster..lod.cluster + lod.clusters as u32 {
                let cl = &clusters[id as usize];
                if cl.first < d.first || cl.first >= d.first + d.count {
                    continue;
                }
                let (mut n, mut back) = (0usize, 0usize);
                for t in 0..cl.count as usize / 3 {
                    let at = (lod.index + cl.first) as usize + t * 3;
                    let p = [0, 1, 2].map(|k| {
                        let v = vertices[lod.vertex as usize + indices[at + k] as usize];
                        v3(centre[0] + v.pos[0] as f32 * scene.quantum, centre[1] + v.pos[1] as f32 * scene.quantum, centre[2] + v.pos[2] as f32 * scene.quantum)
                    });
                    let normal = (p[1] - p[0]).cross(p[2] - p[0]);
                    n += 1;
                    back += (normal.dot(p[0] - eye) >= 0.0) as usize;
                }
                sent += n;
                turned += back;
                if back == n {
                    whole += n;
                    flat += (cl.cutoff.unsigned_abs() < 12) as usize * n;
                    by_cutoff[(cl.cutoff.max(0) as usize * 5 / 128).min(4)] += n;
                }
            }
        }
        println!("faces sent {sent}: {turned} turned away, {whole} of them in clusters turned away whole ({flat} in flat ones; by the cone's cutoff, fifths of 127: {by_cutoff:?})");
    }

    // The large triangles the device would test, as its near pass does.
    let vertices: Vec<WorldVertex> = records(&bytes, pack::WVTX);
    let indices: Vec<u16> = records(&bytes, pack::WIDX);
    let (mut tested, mut verdicts, mut by_edge) = (0usize, [0usize; 3], [[0usize; 5]; 5]);
    let (mut by_rule, mut by_sphere) = (0usize, 0usize);
    let chord = guard.chord();
    for d in draws.iter().filter(|d| d.cut) {
        let c = &chunks[d.chunk as usize];
        let lod = c.lod(d.lod as usize);
        let centre = scene.cell_centre(c.cell as u32);
        let stored = guard.stored(centre, scene.quantum);
        // The cluster this draw is: the one of the level that starts where the draw does.
        let cl = (lod.cluster..lod.cluster + lod.clusters as u32).map(|i| &clusters[i as usize]).find(|cl| cl.first == d.first).expect("the draw's cluster");
        let away = (v3(cl.centre[0], cl.centre[1], cl.centre[2]) - eye).len() - cl.radius;
        let far = [1.77f32, 3.9, 7.8, 15.7].iter().filter(|x| away >= **x).count();
        for t in 0..d.large as usize {
            let at = (lod.index + d.first) as usize + t * 3;
            let v = [0, 1, 2].map(|k| vertices[lod.vertex as usize + indices[at + k] as usize]);
            let p = v.map(|v| v3(v.pos[0] as f32, v.pos[1] as f32, v.pos[2] as f32));
            let corner = p.map(|p| stored.corner(p));
            let verdict = stored.judge(&corner[0], &corner[1], &corner[2]);
            tested += 1;
            verdicts[verdict as usize] += 1;
            let edge = [(p[0] - p[1]).len(), (p[1] - p[2]).len(), (p[2] - p[0]).len()].into_iter().fold(0.0f32, f32::max) * scene.quantum;
            let size = [0.9f32, 2.0, 4.0, 8.0].iter().filter(|x| edge > **x).count();
            by_edge[far][size] += 1;
            // Tested when its edge could reach from the view to the band from as near as the cluster is.
            by_rule += (away < 1.77 || edge > chord * away) as usize;
            // The same asked of the triangle itself: how near the eye its own sphere comes.
            let middle = (p[0] + p[1] + p[2]) * (scene.quantum / 3.0) + v3(centre[0], centre[1], centre[2]);
            let radius = [0, 1, 2].map(|k| ((p[k] * scene.quantum + v3(centre[0], centre[1], centre[2])) - middle).len()).into_iter().fold(0.0f32, f32::max);
            let near = (middle - eye).len() - radius;
            by_sphere += (near < 1.77 || edge > chord * near) as usize;
        }
    }
    println!("tested {tested}: safe {} culled {} to cut {}", verdicts[0], verdicts[1], verdicts[2]);
    println!("by the cluster's distance (rows: under 1.77, 3.9, 7.8, 15.7 m, beyond) and longest edge (columns: under 0.9, 2, 4, 8 m, over):");
    for row in by_edge {
        println!("  {row:?}");
    }
    println!("tested by the edge a cluster's distance allows: {by_rule}; by each triangle's own distance: {by_sphere}");
}
