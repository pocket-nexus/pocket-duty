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
use crate::math::{v3, V3};

#[derive(Clone, Copy)]
pub struct World<'a> {
    pub scene: &'a Scene,
    pub chunks: &'a [Chunk],
    pub clusters: &'a [Cluster],
    /// For each cluster, where the records of its large triangles start (`duty_pack::LTRO`).
    pub larges_at: &'a [u32],
    pub cells: &'a [ViewCell],
    pub lists: &'a [u16],
    pub bits: &'a [u8],
}

/// The chunks by the cell they stand in. A chunk's record is two lines of the data cache, and a
/// line fetched while the GE draws costs a console a microsecond: a cell that is out of the view
/// answers for all its chunks from 24 bytes.
pub struct Cells {
    /// Each chunk's cell.
    of: Vec<u16>,
    /// A box about each cell's chunks: its least corner, then its greatest.
    bounds: Vec<[[f32; 3]; 2]>,
    /// What the frame has found of each cell: 0 not asked yet, 1 out of the view, 2 wholly in it,
    /// 3 across a side of it.
    marks: Vec<u8>,
}

impl Cells {
    /// No chunks.
    pub fn none() -> Cells {
        Cells { of: Vec::new(), bounds: Vec::new(), marks: Vec::new() }
    }

    pub fn new(scene: &Scene, chunks: &[Chunk]) -> Cells {
        let count = (scene.cells_x * scene.cells_z) as usize;
        let mut bounds = alloc::vec![[[f32::MAX; 3], [f32::MIN; 3]]; count];
        let mut of = Vec::with_capacity(chunks.len());
        for c in chunks {
            let cell = (c.cell as usize).min(count - 1);
            of.push(cell as u16);
            for a in 0..3 {
                bounds[cell][0][a] = bounds[cell][0][a].min(c.min[a]);
                bounds[cell][1][a] = bounds[cell][1][a].max(c.max[a]);
            }
        }
        Cells { of, bounds, marks: alloc::vec![0; count] }
    }
}

/// A run of indices of one chunk at one level of detail.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Draw {
    pub chunk: u16,
    pub lod: u8,
    /// The run is one cluster whose sphere reaches out of the guard band: its first `large`
    /// triangles are to be tested, and cut where they leave it. Their records start at `tri`.
    pub cut: bool,
    pub large: u16,
    pub tri: u32,
    /// Where the run starts in the level's index list, and how many indices it has.
    pub first: u32,
    pub count: u32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Counts {
    pub draws: u32,
    pub triangles: u32,
    /// The same triangles with each counted once: what a renderer that draws the ground in one pass sends.
    pub once: u32,
    /// Chunks the cell lists, and of them the ones in the view; of those, the ones that reach
    /// across a side of the view, and the ones not wholly inside the guard band.
    pub listed: u32,
    pub chunks: u32,
    pub across: u32,
    pub guarded: u32,
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

/// Whether every face of a cluster is turned away from the eye: the direction from the eye to
/// the cone's apex is inside the cone (`Cluster::axis`, `cutoff`, `apex`). Asked without the
/// square root.
#[inline]
pub fn turned_away(cl: &Cluster, centre: V3, eye: V3) -> bool {
    if cl.cutoff >= 127 {
        return false;
    }
    const UNIT: f32 = 1.0 / 127.0;
    let axis = v3(cl.axis[0] as f32 * UNIT, cl.axis[1] as f32 * UNIT, cl.axis[2] as f32 * UNIT);
    let mut to = centre - eye;
    if cl.apex != 0 {
        to = to - axis * (cl.apex as f32 * cl.radius * (1.0 / 16.0));
    }
    let along = to.dot(axis);
    if along <= 0.0 {
        return false;
    }
    let cutoff = cl.cutoff as f32 * UNIT;
    along * along >= cutoff * cutoff * to.len2()
}

/// Fills `out` with the frame's draws, in the pack's chunk order (one picture after another).
/// `planes` are the view's, with unit normals; the far one is not asked (the level ends before it).
///
/// A console's floating point unit takes some twenty cycles for a multiply and an add, so what is
/// asked of a cluster is what its chunk could not answer for it: only the planes the chunk's box
/// reaches across, and the guard band only when the chunk's box is not inside it.
pub fn select(
    world: &World,
    cells: &mut Cells,
    planes: &[[f32; 4]; 6],
    eye: V3,
    guard: &Guard,
    options: &Options,
    out: &mut Vec<Draw>,
) -> Counts {
    /// The view's sides and its near plane.
    const PLANES: usize = 5;
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
    let signs = mat::plane_signs(planes);
    let near2 = [scene.lod[0] * options.nearer, scene.lod[1] * options.nearer].map(|d| d * d);
    cells.marks.fill(0);
    let mut consider = |i: usize| {
        counts.listed += 1;
        // The chunk's cell first: out of the view, the chunk's own record is not read.
        let at = cells.of[i] as usize;
        if cells.marks[at] == 0 {
            cells.marks[at] = match mat::box_across(planes, &signs, PLANES, &cells.bounds[at]) {
                None => 1,
                Some(0) => 2,
                Some(_) => 3,
            };
        }
        let mark = cells.marks[at];
        if mark == 1 {
            return;
        }
        let c = &world.chunks[i];
        let across = if mark == 2 {
            0
        } else {
            match mat::box_across(planes, &signs, PLANES, &[c.min, c.max]) {
                Some(across) => across,
                None => return,
            }
        };
        counts.chunks += 1;
        let distance2 = mat::box_distance2(eye, &c.min, &c.max);
        let lod = if distance2 < near2[0] {
            0
        } else if distance2 < near2[1] {
            1
        } else {
            2
        };
        // A chunk inside the band: none of its clusters is asked about the band.
        let guarded = !guard.far_safe(&c.min, &c.max, distance2);
        counts.across += (across != 0) as u32;
        counts.guarded += guarded as u32;
        let level = c.lod(lod);
        let passes = if c.flags & chunk::GROUND != 0 { 2 } else { 1 };
        let mut run: Option<Draw> = None;
        for id in level.cluster as usize..level.cluster as usize + level.clusters as usize {
            counts.clusters += 1;
            if bits.is_some_and(|b| b[id >> 3] >> (id & 7) & 1 == 0) {
                counts.unseen += 1;
                continue;
            }
            let cl = &world.clusters[id];
            if across != 0 {
                // A sphere that reaches across a side of the view may hold a box that does not.
                let mut outside = false;
                for k in 0..PLANES {
                    if across >> k & 1 == 0 {
                        continue;
                    }
                    let p = &planes[k];
                    let d = p[0] * cl.centre[0] + p[1] * cl.centre[1] + p[2] * cl.centre[2] + p[3];
                    if d < -cl.radius {
                        outside = true;
                        break;
                    }
                    if d < 0.0 {
                        let (half, r) = (cl.half(), &reach[k]);
                        if d + r[0] * half[0] + r[1] * half[1] + r[2] * half[2] < 0.0 {
                            outside = true;
                            break;
                        }
                    }
                }
                if outside {
                    counts.outside += 1;
                    continue;
                }
            }
            let centre = v3(cl.centre[0], cl.centre[1], cl.centre[2]);
            if turned_away(cl, centre, eye) {
                counts.turned += 1;
                continue;
            }
            counts.drawn += 1;
            counts.triangles += cl.count as u32 / 3 * passes;
            counts.once += cl.count as u32 / 3;
            let cut =
                guarded && cl.large > 0 && !guard.cluster_safe(centre, cl.radius, || cl.half());
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
                        tri: if cut { world.larges_at[id] } else { 0 },
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
