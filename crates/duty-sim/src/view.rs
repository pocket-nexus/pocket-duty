//! What a frame draws of the world.
//!
//! The eye's cell lists the chunks some place in it sees, and has a bit for every cluster.
//! A listed chunk inside the view is drawn at the level of detail its distance asks for, by
//! the clusters of that level whose bit is set, whose sphere is in the view and whose faces are
//! not all turned away. Clusters that follow each other in the index list leave as one draw.

use alloc::vec::Vec;

use duty_pack::{chunk, Chunk, Cluster, Scene, ViewCell};

use crate::clip::Guard;
use crate::mat;
use crate::math::{sqrt, v3, V3};

#[derive(Clone, Copy)]
pub struct World<'a> {
    pub scene: &'a Scene,
    pub chunks: &'a [Chunk],
    pub clusters: &'a [Cluster],
    pub cells: &'a [ViewCell],
    pub lists: &'a [u16],
    pub bits: &'a [u8],
}

/// A run of indices of one chunk at one level of detail.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Draw {
    pub chunk: u16,
    pub lod: u8,
    /// The run is one cluster whose sphere reaches out of the guard band: its first `large`
    /// triangles are to be tested, and cut where they leave it.
    pub cut: bool,
    pub large: u16,
    /// Where the run starts in the level's index list, and how many indices it has.
    pub first: u32,
    pub count: u32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Counts {
    pub draws: u32,
    pub triangles: u32,
    /// Chunks the cell lists, and of them the ones in the view.
    pub listed: u32,
    pub chunks: u32,
    /// Clusters of those chunks' levels; the ones no place in the cell sees; outside the view;
    /// turned away; drawn.
    pub clusters: u32,
    pub unseen: u32,
    pub outside: u32,
    pub turned: u32,
    pub drawn: u32,
    /// Clusters handed over to be tested triangle by triangle, and their large triangles.
    pub cut: u32,
    pub large: u32,
}

#[derive(Clone, Copy)]
pub struct Options {
    /// Moves the levels' distances (1 is the pack's own).
    pub nearer: f32,
    /// Draw every chunk and every cluster the view holds, whatever the cell lists.
    pub everything: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            nearer: 1.0,
            everything: false,
        }
    }
}

/// Fills `out` with the frame's draws, in the pack's chunk order (one picture after another).
/// `planes` are the view's, with unit normals.
pub fn select(
    world: &World,
    planes: &[[f32; 4]; 6],
    eye: V3,
    guard: &Guard,
    options: &Options,
    out: &mut Vec<Draw>,
) -> Counts {
    out.clear();
    let scene = world.scene;
    let mut counts = Counts::default();
    let cell = if options.everything {
        None
    } else {
        scene
            .cell_at(eye.x, eye.z)
            .and_then(|c| world.cells.get(c as usize))
            .filter(|c| c.count > 0)
    };
    let bits = cell.map(|c| &world.bits[c.bits as usize..]);
    let reach = mat::box_reach(planes, &scene.level_x, &scene.level_z);
    let mut consider = |i: usize| {
        let c = &world.chunks[i];
        counts.listed += 1;
        if !mat::visible(planes, &c.min, &c.max) {
            return;
        }
        counts.chunks += 1;
        // A chunk wholly in view: none of its clusters is asked about the view again.
        let whole = mat::contained(planes, &c.min, &c.max);
        let distance = mat::box_distance(eye, &c.min, &c.max);
        let lod = if distance < scene.lod[0] * options.nearer {
            0
        } else if distance < scene.lod[1] * options.nearer {
            1
        } else {
            2
        };
        let level = &c.lod[lod];
        let passes = if c.flags & chunk::GROUND != 0 { 2 } else { 1 };
        let mut run: Option<Draw> = None;
        for id in level.cluster as usize..(level.cluster + level.clusters) as usize {
            let cl = &world.clusters[id];
            counts.clusters += 1;
            if bits.is_some_and(|b| b[id >> 3] >> (id & 7) & 1 == 0) {
                counts.unseen += 1;
                continue;
            }
            // A sphere that reaches across a side of the view may hold a box that does not.
            let half = cl.half();
            if !whole && !mat::cluster_visible(planes, &reach, &cl.centre, cl.radius, &half) {
                counts.outside += 1;
                continue;
            }
            let centre = v3(cl.centre[0], cl.centre[1], cl.centre[2]);
            if cl.cutoff < 127 {
                // All its faces are turned away when the direction to it stays inside their cone.
                let to = centre - eye;
                let along = (to.x * cl.axis[0] as f32
                    + to.y * cl.axis[1] as f32
                    + to.z * cl.axis[2] as f32)
                    * (1.0 / 127.0);
                if along >= cl.cutoff as f32 * (1.0 / 127.0) * sqrt(to.len2()) + cl.radius {
                    counts.turned += 1;
                    continue;
                }
            }
            counts.drawn += 1;
            counts.triangles += cl.count as u32 / 3 * passes;
            let cut = cl.large > 0
                && !guard.sphere_safe(centre, cl.radius)
                && !guard.box_safe(centre, &half);
            if cut {
                counts.cut += 1;
                counts.large += cl.large as u32;
            }
            match &mut run {
                Some(r) if !cut && !r.cut && r.first + r.count == cl.first => {
                    r.count += cl.count as u32
                }
                _ => {
                    if let Some(r) = run.take() {
                        out.push(r);
                    }
                    run = Some(Draw {
                        chunk: i as u16,
                        lod: lod as u8,
                        cut,
                        large: if cut { cl.large } else { 0 },
                        first: cl.first,
                        count: cl.count as u32,
                    });
                }
            }
        }
        if let Some(r) = run.take() {
            out.push(r);
        }
    };
    match cell {
        Some(c) => world.lists[c.first as usize..(c.first + c.count) as usize]
            .iter()
            .for_each(|i| consider(*i as usize)),
        None => (0..world.chunks.len()).for_each(&mut consider),
    }
    counts.draws = out
        .iter()
        .map(|d| {
            if world.chunks[d.chunk as usize].flags & chunk::GROUND != 0 {
                2
            } else {
                1
            }
        })
        .sum();
    counts
}
