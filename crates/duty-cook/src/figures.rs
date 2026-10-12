//! The soldiers and the held weapons: from `figures.dir` to the pack's `FIGS` section
//! (`duty_pack::figure`).
//!
//! The export holds a soldier as some 25 000 skinned triangles and a held weapon with its arms as
//! some 100 000, both built of closed shells that pass through one another. A handheld draws 700
//! and 2 300.
//!
//! A held weapon is lowered in two steps that are tried together until the result fits its
//! budgets (one for the weapon, one for each arm):
//!
//! 1. every part is simplified to an error, each collapse putting a vertex on one of its
//!    neighbours, so a kept vertex keeps its place and colour;
//! 2. the triangles of the result that the eye never sees are left out: the weapon is looked at
//!    from the eye in every pose its clips, its aiming, a sprint and a lowering bring it to.
//!
//! The error is halved or raised until what is left after step 2 is the budget. It is measured in
//! a space where equal sizes are equal sizes on the screen in the hip pose (`on_screen`), which
//! gives the triangles to what is near the eye.
//!
//! A soldier is not simplified. Its fifty shells (jacket, pouches, straps, laces) can each only
//! shrink on its own, and at 700 triangles what collapses leave of them is slivers and limbs
//! three faces round. A soldier is drawn again instead (`swept`): rings of points are taken around
//! the rig's own lines (down each leg, along each arm, up the spine and the head), each point
//! where rays from the line leave the region's shells, with the colour and the bones of the mesh
//! there; the rifle is a box swept along its bore. Step 2 then drops what the regions hide of one
//! another from all round in moments of the clips, and the count of sides and stations is the
//! richest whose triangles that show fit the budget.
//!
//! Either way the result is cut into draws: a held weapon's by part, a soldier's by sets of four
//! bones (`draws_by_bones`).

use std::collections::HashMap;
use std::path::Path;

use duty_pack::figure::{self, bone, clip, model, part, Batch, Clip, FigureHeader, Info, Model, Pose, SkinVertex, BATCH_BONES};
use meshopt::{SimplifyOptions, VertexDataAdapter};
use rayon::prelude::*;
use serde_json::Value;

use crate::figure_shot::{carried, cross, dot, sub, unit, Canvas, Depth, View, V3};
use crate::ir::Dir;

/// The mesh a figure's triangles are chosen from: a held weapon's as the export has it, a
/// soldier's as it is drawn again. A vertex follows two bones at most.
struct Source {
    pos: Vec<V3>,
    col: Vec<[u8; 3]>,
    /// The two bones (the same one twice for a vertex with one) and the first one's share.
    skin: Vec<([u8; 2], f32)>,
    /// A vertex of a surface the reference draws from both sides.
    two_sided: Vec<bool>,
    tris: Vec<[u32; 3]>,
}

impl Source {
    fn place(&self, v: usize, poses: &[Pose]) -> V3 {
        let ([a, b], share) = self.skin[v];
        let p = figure::apply(&poses[a as usize], self.pos[v]);
        if a == b || share >= 1.0 {
            return p;
        }
        let q = figure::apply(&poses[b as usize], self.pos[v]);
        core::array::from_fn(|k| p[k] * share + q[k] * (1.0 - share))
    }
}

/// Triangles that belong together: a region of the soldier, a part of the weapon.
struct Group {
    name: String,
    /// Corners, three a triangle.
    idx: Vec<u32>,
    /// The budget its triangles count against.
    class: usize,
}

/// The bones' poses at one moment and where it is looked at from.
struct Stance {
    poses: Vec<Pose>,
    views: Vec<View>,
}

/// A triangle of the result, wound so that its front is the side that shows.
#[derive(Clone, Copy)]
struct Kept {
    v: [u32; 3],
    group: usize,
}

/// A group's triangles simplified until the next collapse would move the surface by more than
/// `error`, measured in the space of `positions` (three floats a vertex, as bytes). A collapse
/// puts a vertex on one of its neighbours, so what is left uses the vertices it had.
fn simplify(group: &Group, positions: &[u8], error: f32) -> Vec<u32> {
    let adapter = VertexDataAdapter::new(positions, 12, 0).expect("positions");
    let options = SimplifyOptions::ErrorAbsolute | SimplifyOptions::Permissive | SimplifyOptions::Prune;
    meshopt::simplify_with_locks(&group.idx, &adapter, &vec![false; positions.len() / 12], 0, error, options, None)
}

/// For each triangle: bit 0 when some view of some stance shows its front, bit 1 its back (which
/// only a two-sided one can show).
fn seen(source: &Source, tris: &[[u32; 3]], stances: &[Stance]) -> Vec<u8> {
    let mut local: HashMap<u32, u32> = HashMap::new();
    let mut used: Vec<u32> = Vec::new();
    let corners: Vec<[u32; 3]> = tris
        .iter()
        .map(|t| {
            t.map(|v| {
                *local.entry(v).or_insert_with(|| {
                    used.push(v);
                    used.len() as u32 - 1
                })
            })
        })
        .collect();
    stances
        .par_iter()
        .map(|stance| {
            let placed: Vec<V3> = used.iter().map(|&v| source.place(v as usize, &stance.poses)).collect();
            let mut flags = vec![0u8; tris.len()];
            let (width, height) = (stance.views[0].width, stance.views[0].height);
            let mut depth = Depth::new(width, height);
            let mut owner = vec![u32::MAX; width * height];
            for view in &stance.views {
                depth.near.fill(f32::NEG_INFINITY);
                owner.fill(u32::MAX);
                for (t, c) in corners.iter().enumerate() {
                    depth.fill(view, c.map(|i| placed[i as usize]), source.two_sided[tris[t][0] as usize], |at, _, front| owner[at] = (t as u32) << 1 | !front as u32);
                }
                for &o in &owner {
                    if o != u32::MAX {
                        flags[(o >> 1) as usize] |= 1 << (o & 1);
                    }
                }
            }
            flags
        })
        .reduce(
            || vec![0u8; tris.len()],
            |mut a, b| {
                for (x, y) in a.iter_mut().zip(b) {
                    *x |= y;
                }
                a
            },
        )
}

/// The groups simplified to their classes' errors, and of that what the stances show; with it how
/// many triangles each class is left with.
fn lowered(source: &Source, groups: &[Group], positions: &[u8], stances: &[Stance], errors: &[f32]) -> (Vec<Kept>, Vec<usize>) {
    let simplified: Vec<Vec<u32>> = groups.par_iter().map(|g| simplify(g, positions, errors[g.class])).collect();
    let mut tris = Vec::new();
    let mut owner = Vec::new();
    for (g, idx) in simplified.iter().enumerate() {
        for t in idx.chunks_exact(3) {
            tris.push([t[0], t[1], t[2]]);
            owner.push(g);
        }
    }
    let flags = seen(source, &tris, stances);
    let mut kept = Vec::new();
    let mut counts = vec![0usize; errors.len()];
    for (i, t) in tris.iter().enumerate() {
        if flags[i] & 1 != 0 {
            kept.push(Kept { v: *t, group: owner[i] });
            counts[groups[owner[i]].class] += 1;
        }
        if flags[i] & 2 != 0 {
            kept.push(Kept { v: [t[0], t[2], t[1]], group: owner[i] });
            counts[groups[owner[i]].class] += 1;
        }
    }
    (kept, counts)
}

/// The least error for each class that leaves it within its budget, and what is left then.
fn fit(source: &Source, groups: &[Group], positions: &[u8], stances: &[Stance], budgets: &[usize], range: (f32, f32)) -> (Vec<Kept>, Vec<f32>) {
    let mut low = vec![range.0; budgets.len()];
    let mut high = vec![range.1; budgets.len()];
    for _ in 0..13 {
        let middle: Vec<f32> = low.iter().zip(&high).map(|(a, b)| (a * b).sqrt()).collect();
        let (_, counts) = lowered(source, groups, positions, stances, &middle);
        for c in 0..budgets.len() {
            if counts[c] > budgets[c] {
                low[c] = middle[c];
            } else {
                high[c] = middle[c];
            }
        }
    }
    // What one class hides of another moves a little as the others settle: step up until all fit.
    loop {
        let (kept, counts) = lowered(source, groups, positions, stances, &high);
        let over: Vec<usize> = (0..budgets.len()).filter(|&c| counts[c] > budgets[c]).collect();
        if over.is_empty() || over.iter().all(|&c| high[c] >= range.1 * 4.0) {
            return (kept, high);
        }
        for c in over {
            high[c] *= 1.04;
        }
    }
}

fn face(source: &Source, k: &Kept) -> (V3, f32) {
    let [a, b, c] = k.v.map(|v| source.pos[v as usize]);
    let n = cross(sub(b, a), sub(c, a));
    let l = dot(n, n).sqrt();
    (if l > 1e-20 { [n[0] / l, n[1] / l, n[2] / l] } else { [0.0, 1.0, 0.0] }, l * 0.5)
}

/// A normal for every corner of the result: the faces that meet at the corner's place and turn
/// less from its own face than the angle whose cosine is `crease`, weighed by their areas.
fn normals(source: &Source, kept: &[Kept], crease: f32) -> Vec<[V3; 3]> {
    let faces: Vec<(V3, f32)> = kept.iter().map(|k| face(source, k)).collect();
    let key = |v: u32| source.pos[v as usize].map(|c| (c * 20000.0).round() as i32);
    let mut at: HashMap<[i32; 3], Vec<u32>> = HashMap::new();
    for (t, k) in kept.iter().enumerate() {
        for v in k.v {
            at.entry(key(v)).or_default().push(t as u32);
        }
    }
    kept.iter()
        .enumerate()
        .map(|(t, k)| {
            k.v.map(|v| {
                let mut sum = [0.0f32; 3];
                for &other in &at[&key(v)] {
                    let (n, area) = faces[other as usize];
                    if dot(n, faces[t].0) > crease {
                        for c in 0..3 {
                            sum[c] += n[c] * area;
                        }
                    }
                }
                if dot(sum, sum) > 1e-24 {
                    unit(sum)
                } else {
                    faces[t].0
                }
            })
        })
        .collect()
}

/// One draw as it is written.
struct Draw {
    bones: Vec<u8>,
    vertices: Vec<SkinVertex>,
    indices: Vec<u16>,
}

fn abgr(c: [u8; 3]) -> u32 {
    0xff00_0000 | (c[2] as u32) << 16 | (c[1] as u32) << 8 | c[0] as u32
}

/// The triangles `which` of the result as one draw that blends `bones` (four at most).
fn draw(source: &Source, kept: &[Kept], corner_normals: &[[V3; 3]], which: &[usize], bones: Vec<u8>) -> Draw {
    let mut vertices: Vec<SkinVertex> = Vec::new();
    let mut indices = Vec::new();
    let mut have: HashMap<(u32, [i8; 3]), u16> = HashMap::new();
    for &t in which {
        for (k, &v) in kept[t].v.iter().enumerate() {
            let normal = corner_normals[t][k].map(|c| (c * 127.0).round() as i8);
            let index = *have.entry((v, normal)).or_insert_with(|| {
                let ([a, b], share) = source.skin[v as usize];
                let mut weights = [0u8; 4];
                let first = ((share * 128.0).round() as i32).clamp(0, 128) as u8;
                weights[bones.iter().position(|x| *x == a).expect("the batch lists the bone")] = if a == b { 128 } else { first };
                if a != b {
                    weights[bones.iter().position(|x| *x == b).expect("the batch lists the bone")] += 128 - first;
                }
                vertices.push(SkinVertex { weights, colour: abgr(source.col[v as usize]), normal, pad: 0, pos: source.pos[v as usize] });
                (vertices.len() - 1) as u16
            });
            indices.push(index);
        }
    }
    assert!(vertices.len() <= 65536, "a batch holds more vertices than its indices reach");
    Draw { bones, vertices, indices }
}

/// The result cut into draws of at most four bones each. A vertex keeps one set of weights in every
/// draw it is in, so two draws meet without a gap in any pose. `parents` is the rig: the sets of
/// bones tried are those that hang together in it, and each time the set that draws most of what
/// is left is taken.
fn draws_by_bones(source: &mut Source, kept: &[Kept], corner_normals: &[[V3; 3]], parents: &[i64]) -> Vec<Draw> {
    let mask = |skin: &[([u8; 2], f32)], k: &Kept| k.v.iter().fold(0u32, |m, &v| m | 1 << skin[v as usize].0[0] | 1 << skin[v as usize].0[1]);
    // A triangle that five or more bones move loses its weakest second bone until four do.
    for k in kept {
        while mask(&source.skin, k).count_ones() > 4 {
            let weakest = k.v.iter().copied().filter(|&v| source.skin[v as usize].0[0] != source.skin[v as usize].0[1]).min_by(|&a, &b| {
                let minor = |v: u32| source.skin[v as usize].1.min(1.0 - source.skin[v as usize].1);
                minor(a).total_cmp(&minor(b))
            });
            let v = weakest.expect("three vertices of one bone each name three bones at most") as usize;
            let ([a, b], share) = source.skin[v];
            let major = if share >= 0.5 { a } else { b };
            source.skin[v] = ([major, major], 1.0);
        }
    }
    let mut masks: Vec<u32> = kept.iter().map(|k| mask(&source.skin, k)).collect();
    let used = masks.iter().fold(0u32, |a, m| a | m);
    // Every set of up to four used bones that is in one piece in the rig, where a bone no vertex
    // follows is passed over (its children hang on its parent).
    let above = |b: usize| -> Option<usize> {
        let mut p = parents.get(b).copied().unwrap_or(-1);
        while p >= 0 && used >> p & 1 == 0 {
            p = parents[p as usize];
        }
        (p >= 0).then_some(p as usize)
    };
    let mut joined = vec![0u32; 32];
    for b in (0..parents.len().min(32)).filter(|b| used >> b & 1 != 0) {
        if let Some(p) = above(b) {
            joined[b] |= 1 << p;
            joined[p] |= 1 << b;
        }
    }
    let mut sets: Vec<u32> = (0..32).filter(|b| used >> b & 1 != 0).map(|b| 1u32 << b).collect();
    let mut from = 0;
    for _ in 1..4 {
        let until = sets.len();
        for i in from..until {
            let next = (0..32).filter(|b| sets[i] >> b & 1 != 0).fold(0u32, |a, b| a | joined[b]) & !sets[i];
            for b in (0..32).filter(|b| next >> b & 1 != 0) {
                if !sets.contains(&(sets[i] | 1 << b)) {
                    sets.push(sets[i] | 1 << b);
                }
            }
        }
        from = until;
    }
    let mut left: Vec<usize> = (0..kept.len()).collect();
    let mut chosen: Vec<u32> = Vec::new();
    // The sets that each draw a good share of the figure.
    loop {
        let count = |set: u32| left.iter().filter(|&&t| masks[t] & !set == 0).count();
        let best = sets.iter().copied().max_by_key(|set| (count(*set), core::cmp::Reverse(set.count_ones()), core::cmp::Reverse(*set))).unwrap();
        if count(best) < 12.max(kept.len() / 28) {
            break;
        }
        chosen.push(best);
        left.retain(|&t| masks[t] & !best != 0);
    }
    // What is left lies where two of those meet (a shoulder, a hip). Such a triangle joins the set
    // it can at the least cost: a vertex gives up a bone outside the set, and with it that bone's
    // share of the pull; a vertex with no bone in the set goes to the set's bone next to its own in
    // the rig. Giving up a bone only ever narrows the other triangles the vertex is in, but going
    // to another one can take a triangle out of its set, so the pass repeats until none moves.
    let next_to = |b: u8, set: u32| -> Option<u8> {
        let mut up = above(b as usize);
        while let Some(p) = up {
            if set >> p & 1 != 0 {
                return Some(p as u8);
            }
            up = above(p);
        }
        (0..32u8).find(|c| set >> c & 1 != 0 && above(*c as usize) == Some(b as usize))
    };
    for _ in 0..6 {
        masks = kept.iter().map(|k| mask(&source.skin, k)).collect();
        left = (0..kept.len()).filter(|&t| chosen.iter().all(|set| masks[t] & !set != 0)).collect();
        let mut moved = false;
        for &t in &left {
            let mut cheapest: Option<(f32, [([u8; 2], f32); 3])> = None;
            for &set in &chosen {
                let inside = |b: u8| set >> b & 1 != 0;
                let mut cost = 0.0;
                let mut skins = [([0u8; 2], 1.0f32); 3];
                let fits = kept[t].v.iter().enumerate().all(|(k, &v)| {
                    let ([a, b], share) = source.skin[v as usize];
                    skins[k] = match (inside(a), inside(b)) {
                        (true, true) => ([a, b], share),
                        (true, false) => {
                            cost += 1.0 - share;
                            ([a, a], 1.0)
                        }
                        (false, true) => {
                            cost += share;
                            ([b, b], 1.0)
                        }
                        (false, false) => match next_to(if share >= 0.5 { a } else { b }, set) {
                            Some(near) => {
                                cost += 2.0;
                                ([near, near], 1.0)
                            }
                            None => return false,
                        },
                    };
                    true
                });
                if fits && cheapest.map_or(true, |c| cost < c.0) {
                    cheapest = Some((cost, skins));
                }
            }
            if let Some((_, skins)) = cheapest {
                for (k, &v) in kept[t].v.iter().enumerate() {
                    moved |= source.skin[v as usize].0 != skins[k].0;
                    source.skin[v as usize] = skins[k];
                }
            }
        }
        if !moved {
            break;
        }
    }
    masks = kept.iter().map(|k| mask(&source.skin, k)).collect();
    left = (0..kept.len()).filter(|&t| chosen.iter().all(|set| masks[t] & !set != 0)).collect();
    // And what still fits none of them is drawn by sets of its own.
    for &t in &left {
        if !sets.contains(&masks[t]) {
            sets.push(masks[t]);
        }
    }
    while !left.is_empty() {
        let best = sets.iter().copied().max_by_key(|set| (left.iter().filter(|&&t| masks[t] & !set == 0).count(), core::cmp::Reverse(set.count_ones()), core::cmp::Reverse(*set))).unwrap();
        chosen.push(best);
        left.retain(|&t| masks[t] & !best != 0);
    }
    // Each triangle goes to the first set that holds it; a set keeps the bones its triangles use,
    // and two sets that fit in four bones together are one draw.
    let mut lists: Vec<(u32, Vec<usize>)> = chosen.iter().map(|_| (0, Vec::new())).collect();
    for t in 0..kept.len() {
        let at = chosen.iter().position(|set| masks[t] & !set == 0).unwrap();
        lists[at].0 |= masks[t];
        lists[at].1.push(t);
    }
    lists.retain(|l| !l.1.is_empty());
    loop {
        let mut pair = None;
        'search: for i in 0..lists.len() {
            for j in i + 1..lists.len() {
                if (lists[i].0 | lists[j].0).count_ones() <= 4 {
                    pair = Some((i, j));
                    break 'search;
                }
            }
        }
        let Some((i, j)) = pair else { break };
        let (set, tris) = lists.remove(j);
        lists[i].0 |= set;
        lists[i].1.extend(tris);
    }
    lists.iter().map(|(set, tris)| draw(source, kept, corner_normals, tris, (0..32u8).filter(|b| set >> b & 1 != 0).collect())).collect()
}

struct Built {
    model: Model,
    draws: Vec<Draw>,
}

fn built(id: u32, bones: usize, draws: Vec<Draw>) -> Built {
    let (mut min, mut max) = ([f32::MAX; 3], [f32::MIN; 3]);
    for v in draws.iter().flat_map(|d| &d.vertices) {
        for k in 0..3 {
            min[k] = min[k].min(v.pos[k]);
            max[k] = max[k].max(v.pos[k]);
        }
    }
    let triangles = draws.iter().map(|d| d.indices.len() / 3).sum::<usize>() as u32;
    Built { model: Model { id, bones: bones as u32, batches: draws.len() as u32, triangles, min, max }, draws }
}

impl Built {
    fn bytes(&self) -> usize {
        core::mem::size_of::<Model>() + self.draws.iter().map(|d| core::mem::size_of::<Batch>() + d.vertices.len() * core::mem::size_of::<SkinVertex>() + (d.indices.len() * 2 + 3 & !3)).sum::<usize>()
    }

    fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(duty_pack::bytes_of(&[self.model]));
        for d in &self.draws {
            let mut bones = [0u8; BATCH_BONES];
            bones[..d.bones.len()].copy_from_slice(&d.bones);
            out.extend_from_slice(duty_pack::bytes_of(&[Batch { bones, bone_count: d.bones.len() as u32, vertices: d.vertices.len() as u32, indices: d.indices.len() as u32 }]));
            out.extend_from_slice(duty_pack::bytes_of(&d.vertices));
            out.extend_from_slice(duty_pack::bytes_of(&d.indices));
            out.resize(out.len() + 3 & !3, 0);
        }
    }
}

fn triples<T: Copy>(flat: &[T]) -> Vec<[T; 3]> {
    flat.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect()
}

fn list3(v: &Value) -> V3 {
    core::array::from_fn(|i| v[i].as_f64().unwrap_or(0.0) as f32)
}

/// A clip of the export: its record and its poses, frame by frame, bone by bone.
struct Motion {
    clip: Clip,
    poses: Vec<Pose>,
}

impl Motion {
    fn read(dir: &Dir, meta: &Value, model: u32, bones: usize) -> Motion {
        let flat = dir.f32s(meta["poses"].as_str().expect("a clip names its poses"));
        let poses: Vec<Pose> = flat.chunks_exact(12).map(|m| core::array::from_fn(|r| core::array::from_fn(|c| m[r * 4 + c]))).collect();
        let frames = meta["frames"].as_u64().unwrap_or(0) as u32;
        assert_eq!(poses.len(), frames as usize * bones, "a clip holds a pose for every frame and bone");
        Motion { clip: Clip { id: meta["id"].as_u64().unwrap_or(0) as u32, model, bones: bones as u32, frames, rate: meta["rate"].as_f64().unwrap_or(20.0) as f32, looped: meta["looped"].as_u64().unwrap_or(0) as u32 }, poses }
    }

    /// Every bone's pose a share of the way through the clip.
    fn at(&self, share: f32) -> Vec<Pose> {
        let bones = self.clip.bones as usize;
        let f = (((self.clip.frames - 1) as f32 * share).round() as usize).min(self.clip.frames as usize - 1);
        self.poses[f * bones..(f + 1) * bones].to_vec()
    }
}

// ---------------------------------------------------------------- the soldiers

/// Directions a soldier is looked at from: all round, from a little under level to well above.
fn around(count: usize) -> Vec<V3> {
    (0..count)
        .map(|i| {
            let y = 1.0 - 2.0 * (i as f32 + 0.5) / count as f32;
            let (r, a) = ((1.0 - y * y).sqrt(), i as f32 * 2.399_963);
            [r * a.cos(), y, r * a.sin()]
        })
        .filter(|d| d[1] > -0.34 && d[1] < 0.96)
        .collect()
}

/// A soldier as the export has it.
struct Body {
    pos: Vec<V3>,
    col: Vec<[u8; 3]>,
    bones: Vec<u8>,
    weights: Vec<f32>,
    tris: Vec<[u32; 3]>,
    /// The part of the reference's mesh each vertex is of, and the parts' names.
    part: Vec<u8>,
    names: Vec<String>,
}

fn body(dir: &Dir, meta: &Value) -> Body {
    let name = |key: &str| meta[key].as_str().expect("a soldier names its arrays");
    Body {
        pos: triples(&dir.f32s(name("position"))),
        col: triples(dir.u8s(name("colour"))),
        bones: dir.u8s(name("bones")).to_vec(),
        weights: dir.f32s(name("weights")),
        tris: triples(&dir.u32s(name("index"))),
        part: dir.u8s(name("part")).to_vec(),
        names: meta["parts"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default(),
    }
}

const REGIONS: [&str; 7] = ["trunk", "head", "arm R", "arm L", "leg R", "leg L", "rifle"];
const RIFLE: usize = 6;

/// The region a bone belongs to.
fn region(b: usize) -> usize {
    match b {
        bone::NECK..=bone::HEAD_TOP => 1,
        bone::CLAVICLE_R..=bone::FINGERS_R => 2,
        bone::CLAVICLE_L..=bone::FINGERS_L => 3,
        bone::UP_LEG_R..=bone::TOE_R => 4,
        bone::UP_LEG_L..=bone::TOE_L => 5,
        _ => 0,
    }
}

fn add(a: V3, b: V3, k: f32) -> V3 {
    [a[0] + b[0] * k, a[1] + b[1] * k, a[2] + b[2] * k]
}

/// What the mesh is at a place: where, its colour, and how much each bone moves it. Sums until `settle`.
#[derive(Clone)]
struct Point {
    pos: V3,
    col: [f32; 3],
    moved: [f32; bone::SOLDIER],
    count: f32,
}

impl Point {
    fn new() -> Point {
        Point { pos: [0.0; 3], col: [0.0; 3], moved: [0.0; bone::SOLDIER], count: 0.0 }
    }

    fn settle(mut self) -> Point {
        let k = 1.0 / self.count.max(1.0);
        self.col = self.col.map(|c| c * k);
        self.moved = self.moved.map(|m| m * k);
        self.count = 1.0;
        self
    }
}

/// One region of the soldier's mesh: the shells a sweep looks at.
struct Lump<'a> {
    body: &'a Body,
    alias: &'a [u8],
    tris: Vec<[u32; 3]>,
}

impl Lump<'_> {
    /// The last place a ray meets the region's shells within `reach`: how far, and which triangle.
    fn farthest(&self, from: V3, dir: V3, reach: f32) -> Option<(f32, usize, V3)> {
        let mut best: Option<(f32, usize, V3)> = None;
        for (i, t) in self.tris.iter().enumerate() {
            let [a, b, c] = t.map(|v| self.body.pos[v as usize]);
            let (e1, e2) = (sub(b, a), sub(c, a));
            let p = cross(dir, e2);
            let det = dot(e1, p);
            if det.abs() < 1e-12 {
                continue;
            }
            let from_a = sub(from, a);
            let u = dot(from_a, p) / det;
            if !(0.0..=1.0).contains(&u) {
                continue;
            }
            let q = cross(from_a, e1);
            let v = dot(dir, q) / det;
            if v < 0.0 || u + v > 1.0 {
                continue;
            }
            let t = dot(e2, q) / det;
            if t > 1e-5 && t < reach && best.map_or(true, |b| t > b.0) {
                best = Some((t, i, [1.0 - u - v, u, v]));
            }
        }
        best
    }

    /// Adds the colour and the bones of the mesh at a place on a triangle.
    fn gather(&self, tri: usize, shares: V3, into: &mut Point) {
        for (c, &v) in self.tris[tri].iter().enumerate() {
            for k in 0..3 {
                into.col[k] += self.body.col[v as usize][k] as f32 * shares[c];
            }
            for k in 0..4 {
                into.moved[self.alias[self.body.bones[v as usize * 4 + k] as usize] as usize] += self.body.weights[v as usize * 4 + k] * shares[c];
            }
        }
        into.count += 1.0;
    }

    /// The surface where a sheaf of rays about one direction leaves the region: fifteen rays over
    /// `half` radians to either side and part of the way to the neighbouring rings. The point
    /// stands at the reach six tenths of them stay within, so a strap or an aerial does not pull
    /// it out and a gap between two pouches does not pull it in; its colour and bones are theirs
    /// together.
    fn point(&self, centre: V3, frame: &[V3; 3], angle: f32, half: f32, spread: (f32, f32)) -> Option<Point> {
        let toward = |a: f32| add(frame[0].map(|c| c * a.cos()), frame[1], a.sin());
        let mut reaches = Vec::new();
        let mut point = Point::new();
        for turn in [-0.8f32, -0.4, 0.0, 0.4, 0.8] {
            for step in [-1.0f32, 0.0, 1.0] {
                let from = add(centre, frame[2], 0.35 * if step < 0.0 { -spread.0 } else { spread.1 * step });
                if let Some((t, tri, shares)) = self.farthest(from, toward(angle + turn * half), 0.7) {
                    reaches.push(t);
                    self.gather(tri, shares, &mut point);
                }
            }
        }
        if reaches.len() < 5 {
            return None;
        }
        reaches.sort_by(|a, b| a.total_cmp(b));
        point.pos = add(centre, toward(angle), reaches[reaches.len() * 6 / 10]);
        Some(point.settle())
    }
}

/// The two bones that move a point most, and the first one's share. A second bone with under a
/// fifth of the pull is not worth a blend.
fn two_bones(moved: &[f32; bone::SOLDIER]) -> ([u8; 2], f32) {
    let mut order: Vec<usize> = (0..bone::SOLDIER).collect();
    order.sort_by(|a, b| moved[*b].total_cmp(&moved[*a]).then(a.cmp(b)));
    let (a, b) = (order[0], order[1]);
    if moved[b] / (moved[a] + moved[b]).max(1e-9) >= 0.2 {
        ([a as u8, b as u8], moved[a] / (moved[a] + moved[b]))
    } else {
        ([a as u8, a as u8], 1.0)
    }
}

fn push(source: &mut Source, p: &Point) -> u32 {
    source.pos.push(p.pos);
    source.col.push(p.col.map(|c| (c + 0.5).clamp(0.0, 255.0) as u8));
    source.skin.push(two_bones(&p.moved));
    source.two_sided.push(false);
    source.pos.len() as u32 - 1
}

/// A tube round a line of the rig: a ring of `sides` points at every mark `(segment, share of it)`
/// of the line `path`, faces between one ring and the next, and a fan at either end to the last
/// place the line itself meets the region (the crown of the helmet, the toe of the boot), and no
/// nearer than `plunge` metres at the start and at the end: an end that stands in another region
/// (a neck in the collar, a thigh in the hips) reaches into it. `ahead` is the direction the first
/// point of every ring is nearest to, `phase` the turn of that point from it in sides.
fn tube(lump: &Lump, path: &[V3], marks: &[(usize, f32)], sides: usize, ahead: V3, phase: f32, plunge: (f32, f32), source: &mut Source) {
    let along = |i: usize| unit(sub(path[i + 1], path[i]));
    let mut rings: Vec<(V3, V3, Vec<u32>)> = Vec::new();
    let places: Vec<V3> = marks.iter().map(|&(i, f)| add(path[i], sub(path[i + 1], path[i]), f)).collect();
    let mut u = ahead;
    for (k, &(i, f)) in marks.iter().enumerate() {
        // At a joint the ring lies in the plane that halves the bend.
        let normal = if f <= 0.0 && i > 0 {
            unit(add(along(i - 1), along(i), 1.0))
        } else if f >= 1.0 && i + 2 < path.len() {
            unit(add(along(i), along(i + 1), 1.0))
        } else {
            along(i)
        };
        // The ring's first direction: the last ring's, laid into this ring's plane.
        u = unit(add(u, normal, -dot(u, normal)));
        let frame = [u, cross(normal, u), normal];
        let gap = |a: usize, b: usize| dot(sub(places[b], places[a]), sub(places[b], places[a])).sqrt();
        let spread = (if k > 0 { gap(k - 1, k) } else { 0.0 }, if k + 1 < marks.len() { gap(k, k + 1) } else { 0.0 });
        // The middle of the region here: of where rays from the line leave it.
        let mut middle = [0.0f32; 3];
        let mut met = 0.0;
        for j in 0..12 {
            let a = j as f32 * core::f32::consts::TAU / 12.0;
            let toward = add(frame[0].map(|c| c * a.cos()), frame[1], a.sin());
            if let Some((t, _, _)) = lump.farthest(places[k], toward, 0.7) {
                middle = add(middle, add(places[k], toward, t), 1.0);
                met += 1.0;
            }
        }
        if met < 8.0 {
            continue;
        }
        let centre = middle.map(|c| c / met);
        let half = core::f32::consts::PI / sides as f32;
        let mut points: Vec<Option<Point>> = (0..sides).map(|j| lump.point(centre, &frame, (j as f32 + phase) * 2.0 * half, half, spread)).collect();
        if points.iter().flatten().count() * 2 < sides {
            continue;
        }
        // A direction in which the region has nothing takes its neighbour's point, drawn in to the centre's side.
        for j in 0..sides {
            if points[j].is_none() {
                let from = (1..sides).find_map(|d| points[(j + d) % sides].clone().or_else(|| points[(j + sides - d) % sides].clone())).unwrap();
                let a = (j as f32 + phase) * 2.0 * half;
                points[j] = Some(Point { pos: add(centre, add(frame[0].map(|c| c * a.cos()), frame[1], a.sin()), 0.01), ..from });
            }
        }
        rings.push((centre, normal, points.iter().flatten().map(|p| push(source, p)).collect()));
    }
    for pair in rings.windows(2) {
        let (a, b) = (&pair[0].2, &pair[1].2);
        for j in 0..sides {
            let n = (j + 1) % sides;
            source.tris.push([a[j], a[n], b[n]]);
            source.tris.push([a[j], b[n], b[j]]);
        }
    }
    // The ends: a fan to where the line leaves the region, or to the ring's middle.
    for (end, forward) in [(rings.first(), false), (rings.last(), true)] {
        let Some((centre, normal, ring)) = end else { continue };
        let out = if forward { *normal } else { normal.map(|c| -c) };
        let reach = lump.farthest(*centre, out, 0.4).map_or(0.0, |h| h.0).max(if forward { plunge.1 } else { plunge.0 });
        let mut tip = Point::new();
        for &v in ring {
            for k in 0..3 {
                tip.col[k] += source.col[v as usize][k] as f32;
            }
            let ([a, b], share) = source.skin[v as usize];
            tip.moved[a as usize] += share;
            tip.moved[b as usize] += 1.0 - share;
            tip.count += 1.0;
        }
        tip.pos = add(*centre, out, reach);
        let apex = push(source, &tip.settle());
        for j in 0..sides {
            let n = (j + 1) % sides;
            source.tris.push(if forward { [ring[j], ring[n], apex] } else { [ring[n], ring[j], apex] });
        }
    }
}

/// The rifle as a box swept along its bore from `origin`, `top` being the way its sights stand off
/// it: at each of `stations` places a rectangle, as wide and as tall as the rifle is there. The places are where straight sides between them leave the
/// rifle's outline least: the front and the back of the magazine, of the grip, of the sight.
fn boxed(lump: &Lump, origin: V3, bore: V3, top: V3, stations: usize, carrier: u8, source: &mut Source) {
    let up = unit(add(top, bore, -dot(top, bore)));
    let right = cross(up, bore);
    let local = |p: V3| {
        let d = sub(p, origin);
        [dot(d, right), dot(d, up), dot(d, bore)]
    };
    const SLABS: usize = 160;
    let (mut from, mut to) = (f32::MAX, f32::MIN);
    for t in &lump.tris {
        for v in t {
            let z = local(lump.body.pos[*v as usize])[2];
            from = from.min(z);
            to = to.max(z);
        }
    }
    let thick = (to - from) / SLABS as f32;
    // Every slab's extent across and up, and its colour in each quarter of that.
    let mut extent = vec![[f32::MAX, f32::MIN, f32::MAX, f32::MIN]; SLABS];
    let mut samples: Vec<(usize, f32, f32, [u8; 3])> = Vec::new();
    for t in &lump.tris {
        for k in 0..3 {
            let (a, b) = (t[k] as usize, t[(k + 1) % 3] as usize);
            let (pa, pb) = (local(lump.body.pos[a]), local(lump.body.pos[b]));
            let steps = ((pb[2] - pa[2]).abs() / (thick * 0.5)).ceil() as usize + 1;
            for i in 0..=steps {
                let p = add(pa, sub(pb, pa), i as f32 / steps as f32);
                let slab = (((p[2] - from) / thick) as usize).min(SLABS - 1);
                let e = &mut extent[slab];
                *e = [e[0].min(p[0]), e[1].max(p[0]), e[2].min(p[1]), e[3].max(p[1])];
                samples.push((slab, p[0], p[1], lump.body.col[if i * 2 < steps { a } else { b }]));
            }
        }
    }
    for i in 1..SLABS {
        if extent[i][0] > extent[i][1] {
            extent[i] = extent[i - 1];
        }
    }
    let mut colour = vec![[[0.0f32; 4]; 4]; SLABS];
    for (slab, x, y, c) in samples {
        let e = extent[slab];
        let quarter = (x > (e[0] + e[1]) * 0.5) as usize | ((y > (e[2] + e[3]) * 0.5) as usize) << 1;
        for k in 0..3 {
            colour[slab][quarter][k] += c[k] as f32;
        }
        colour[slab][quarter][3] += 1.0;
    }
    // Start from the two ends; each time, the stretch that straight sides miss by most in all is cut
    // at the slab they miss by most.
    let mut chosen = vec![0, SLABS - 1];
    while chosen.len() < stations.max(2) {
        let mut worst = (0.0f32, 0usize);
        for pair in chosen.windows(2) {
            let miss = |i: usize| -> f32 {
                let f = (i - pair[0]) as f32 / (pair[1] - pair[0]) as f32;
                (0..4).map(|k| (extent[i][k] - (extent[pair[0]][k] + (extent[pair[1]][k] - extent[pair[0]][k]) * f)).abs() * if k < 2 { 0.5 } else { 1.0 }).sum()
            };
            let all: f32 = (pair[0] + 1..pair[1]).map(miss).sum();
            if all > worst.0 {
                worst = (all, (pair[0] + 1..pair[1]).max_by(|a, b| miss(*a).total_cmp(&miss(*b))).unwrap());
            }
        }
        if worst.0 <= 0.0 {
            break;
        }
        chosen.push(worst.1);
        chosen.sort();
    }
    let mut rings: Vec<[u32; 4]> = Vec::new();
    for (k, &slab) in chosen.iter().enumerate() {
        let z = if k == 0 { from } else if k + 1 == chosen.len() { to } else { from + (slab as f32 + 0.5) * thick };
        let e = extent[slab];
        // Counter-clockwise seen from ahead of the muzzle: top right, top left, bottom left, bottom right.
        rings.push([(e[1], e[3], 3), (e[0], e[3], 2), (e[0], e[2], 0), (e[1], e[2], 1)].map(|(x, y, quarter)| {
            let c = colour[slab][quarter];
            let mean = (0..4).fold([0.0f32; 4], |a, q| core::array::from_fn(|i| a[i] + colour[slab][q][i]));
            let c = if c[3] > 0.0 { c } else { mean };
            let mut point = Point::new();
            point.pos = add(add(add(origin, right, x), up, y), bore, z);
            point.col = [c[0] / c[3].max(1.0), c[1] / c[3].max(1.0), c[2] / c[3].max(1.0)];
            point.moved[carrier as usize] = 1.0;
            push(source, &point)
        }));
    }
    for pair in rings.windows(2) {
        for j in 0..4 {
            let n = (j + 1) % 4;
            source.tris.push([pair[0][j], pair[0][n], pair[1][n]]);
            source.tris.push([pair[0][j], pair[1][n], pair[1][j]]);
        }
    }
    let (first, last) = (rings[0], rings[rings.len() - 1]);
    source.tris.extend([[first[0], first[2], first[1]], [first[0], first[3], first[2]], [last[0], last[1], last[2]], [last[0], last[2], last[3]]]);
}

/// How finely a soldier is drawn: the sides of a leg's, an arm's, the trunk's and the head's rings,
/// and the rifle's stations.
type Detail = [usize; 5];
const DETAILS: [Detail; 15] = [
    [4, 3, 6, 6, 8],
    [4, 4, 6, 6, 10],
    [5, 4, 6, 6, 12],
    [5, 4, 8, 6, 14],
    [5, 5, 8, 8, 14],
    [6, 5, 8, 8, 16],
    [6, 5, 10, 8, 16],
    [6, 6, 10, 8, 18],
    [6, 6, 10, 10, 18],
    [6, 6, 10, 10, 22],
    [6, 6, 12, 10, 22],
    [8, 6, 12, 10, 22],
    [8, 6, 12, 12, 24],
    [8, 8, 12, 12, 24],
    [8, 8, 16, 12, 28],
];

/// The soldier drawn again at one detail: every region a tube round its line of the rig, the rifle
/// a box along its bore. `joints` and `tails` are the bones' ends in the bind pose.
fn swept(body: &Body, alias: &[u8], joints: &[V3], tails: &[V3], bore: (V3, V3, V3), detail: Detail) -> (Source, Vec<Group>) {
    // A part of the reference's mesh goes with the rifle, or with the region whose bones carry most of it.
    let mut carried = vec![[0.0f32; RIFLE]; body.names.len()];
    for v in 0..body.pos.len() {
        for k in 0..4 {
            carried[body.part[v] as usize][region(body.bones[v * 4 + k] as usize)] += body.weights[v * 4 + k];
        }
    }
    let of_part: Vec<usize> = (0..body.names.len()).map(|p| if body.names[p].starts_with("wpn") { RIFLE } else { (0..RIFLE).max_by(|a, b| carried[p][*a].total_cmp(&carried[p][*b])).unwrap() }).collect();
    let lump = |r: usize| Lump { body, alias, tris: body.tris.iter().copied().filter(|t| of_part[body.part[t[0] as usize] as usize] == r).collect() };
    let j = |b: usize| joints[b];
    let beyond = |b: usize| add(tails[b], sub(tails[b], joints[b]), 1.0);
    let up = [0.0, 1.0, 0.0];
    let ahead = [0.0, 0.0, 1.0];

    let mut source = Source { pos: Vec::new(), col: Vec::new(), skin: Vec::new(), two_sided: Vec::new(), tris: Vec::new() };
    let mut groups = Vec::new();
    let mut close = |source: &mut Source, name: &str, first: usize| {
        groups.push(Group { name: name.to_string(), idx: source.tris[first..].iter().flatten().copied().collect(), class: 0 });
    };

    // The trunk, from under the hips to the neck: a flat face ahead, where the plate and the pouches are.
    let first = source.tris.len();
    tube(&lump(0), &[add(j(bone::HIPS), up, -0.16), j(bone::HIPS), j(bone::SPINE), j(bone::SPINE1), j(bone::SPINE2), j(bone::NECK), j(bone::HEAD)], &[(0, 0.4), (1, 0.0), (2, 0.0), (3, 0.0), (4, 0.0), (4, 0.5), (5, 0.0), (5, 0.6)], detail[2], ahead, 0.5, (0.0, 0.05), &mut source);
    close(&mut source, REGIONS[0], first);
    // The head, from inside the collar to the crown of the helmet.
    let first = source.tris.len();
    tube(&lump(1), &[j(bone::SPINE2), j(bone::NECK), j(bone::HEAD), j(bone::HEAD_TOP), add(j(bone::HEAD_TOP), up, 0.12)], &[(0, 0.6), (1, 0.3), (2, 0.0), (2, 0.28), (2, 0.52), (2, 0.74), (2, 0.93)], detail[3], ahead, 0.0, (0.08, 0.0), &mut source);
    close(&mut source, REGIONS[1], first);
    for (r, shoulder) in [(2, bone::UPPER_ARM_R), (3, bone::UPPER_ARM_L)] {
        let first = source.tris.len();
        tube(&lump(r), &[j(shoulder), j(shoulder + 1), j(shoulder + 2), j(shoulder + 3), beyond(shoulder + 3)], &[(0, 0.1), (0, 0.55), (1, 0.0), (1, 0.5), (2, 0.0), (2, 0.85)], detail[1], up, 0.0, (0.06, 0.0), &mut source);
        close(&mut source, REGIONS[r], first);
    }
    for (r, hip) in [(4, bone::UP_LEG_R), (5, bone::UP_LEG_L)] {
        let first = source.tris.len();
        // Two rings at the knee, which bends furthest; the boot from the ankle to the toe.
        tube(&lump(r), &[j(hip), j(hip + 1), j(hip + 2), j(hip + 3), beyond(hip + 3)], &[(0, 0.08), (0, 0.46), (0, 0.86), (1, 0.14), (1, 0.52), (1, 0.9), (2, 0.0), (2, 0.6), (3, 0.3)], detail[0], ahead, 0.0, (0.08, 0.0), &mut source);
        close(&mut source, REGIONS[r], first);
    }
    let first = source.tris.len();
    boxed(&lump(RIFLE), bore.0, bore.1, bore.2, detail[4], bone::HAND_R as u8, &mut source);
    close(&mut source, REGIONS[RIFLE], first);
    (source, groups)
}

fn soldier(dir: &Dir, meta: &Value, alias: &[u8], rig: &Value, motions: &[Motion], budget: usize) -> (Built, String) {
    let body = body(dir, meta);
    let parents: Vec<i64> = rig["parents"].as_array().map(|a| a.iter().map(|v| v.as_i64().unwrap_or(-1)).collect()).unwrap_or_default();
    let joints: Vec<V3> = (0..bone::SOLDIER).map(|b| list3(&rig["joints"][b])).collect();
    let tails: Vec<V3> = (0..bone::SOLDIER).map(|b| list3(&rig["tails"][b])).collect();
    // The toes' bend is not drawn: the boot is the foot's.
    let mut alias = alias.to_vec();
    alias[bone::TOE_R] = alias[bone::FOOT_R];
    alias[bone::TOE_L] = alias[bone::FOOT_L];
    let (muzzle, breech) = (list3(&meta["weapon"]["muzzle"]), list3(&meta["weapon"]["boreOrigin"]));
    // (the magazine hangs straight under the bore)
    let bore = (breech, unit(sub(muzzle, breech)), sub(breech, list3(&meta["weapon"]["magBottom"])));

    let motion = |id: u32| motions.iter().find(|m| m.clip.id == id).expect("the export holds every soldier clip");
    let mut moments = vec![vec![figure::UNMOVED; bone::SOLDIER]];
    for (id, shares) in [(clip::WALK_AIM, &[0.0, 0.25, 0.5, 0.75][..]), (clip::RUN, &[0.0, 0.25, 0.5, 0.75]), (clip::PATROL, &[0.25, 0.75]), (clip::CROUCH_WALK, &[0.0, 0.5]), (clip::CROUCH_AIM, &[0.0]), (clip::AIM, &[0.0]), (clip::IDLE, &[0.0]), (clip::RELOAD, &[0.25, 0.5, 0.75]), (clip::HIT, &[0.3])] {
        moments.extend(shares.iter().map(|s| motion(id).at(*s)));
    }
    let directions = around(96);
    let stances: Vec<Stance> = moments.into_iter().map(|poses| Stance { poses, views: directions.iter().map(|d| View::flat(448, 512, *d, [0.0, 0.95, 0.15], 2.3)).collect() }).collect();

    // The richest drawing whose triangles that show are within the budget (the plainest, if none is).
    let mut chosen: Option<(Source, Vec<Group>, Vec<Kept>, Detail, usize)> = None;
    for detail in DETAILS {
        let (source, groups) = swept(&body, &alias, &joints, &tails, bore, detail);
        let mut owner = vec![0; source.tris.len()];
        let mut at = 0;
        for (g, group) in groups.iter().enumerate() {
            owner[at..at + group.idx.len() / 3].fill(g);
            at += group.idx.len() / 3;
        }
        let flags = seen(&source, &source.tris, &stances);
        let kept: Vec<Kept> = (0..source.tris.len()).filter(|&t| flags[t] & 1 != 0).map(|t| Kept { v: source.tris[t], group: owner[t] }).collect();
        if kept.len() > budget && chosen.is_some() {
            break;
        }
        let drawn = source.tris.len();
        chosen = Some((source, groups, kept, detail, drawn));
    }
    let (mut source, groups, kept, detail, drawn) = chosen.expect("a soldier is drawn at one detail at least");
    // A body is round: only a fold sharper than 80° shows as one.
    let corner_normals = normals(&source, &kept, 0.17);
    let per_group: Vec<String> = groups.iter().enumerate().map(|(g, group)| format!("{} {}", group.name, kept.iter().filter(|k| k.group == g).count())).collect();
    let draws = draws_by_bones(&mut source, &kept, &corner_normals, &parents);
    let made = built(meta["model"].as_u64().unwrap_or(0) as u32, bone::SOLDIER, draws);
    let line = format!(
        "{}: {} triangles → drawn again with sides {:?} as {} → {} that show ({}), {} batches of {:?} bones, {} vertices, {} bytes",
        meta["name"].as_str().unwrap_or("soldier"),
        body.tris.len(),
        detail,
        drawn,
        made.model.triangles,
        per_group.join(", "),
        made.draws.len(),
        made.draws.iter().map(|d| d.bones.len()).collect::<Vec<_>>(),
        made.draws.iter().map(|d| d.vertices.len()).sum::<usize>(),
        made.bytes()
    );
    (made, line)
}

// ---------------------------------------------------------------- the weapons in the hands

/// Metres from the eye at which a metre of the simplifier's space is a metre.
const REFERENCE: f32 = 0.3;
/// Nearer than this (and behind the eye) nothing grows further in the simplifier's space.
const NEAREST: f32 = 0.1;

/// A point of the eye's space in the simplifier's: across and up as the screen shows them at the
/// point's depth, and depth so that a step along it is the same size as a step across.
fn on_screen(p: V3) -> V3 {
    let depth = -p[2];
    if depth >= NEAREST {
        let s = REFERENCE / depth;
        [p[0] * s, p[1] * s, -REFERENCE * (depth / REFERENCE).ln()]
    } else {
        let s = REFERENCE / NEAREST;
        [p[0] * s, p[1] * s, -REFERENCE * (NEAREST / REFERENCE).ln() + (p[2] + NEAREST) * s]
    }
}

fn held_source(dir: &Dir, meta: &Value) -> Source {
    let name = |key: &str| meta[key].as_str().expect("a held weapon names its arrays");
    let pos = triples(&dir.f32s(name("position")));
    let parts = dir.u8s(name("part"));
    let mut two_sided = vec![false; pos.len()];
    for mesh in meta["meshes"].as_array().into_iter().flatten() {
        if mesh["doubleSided"].as_bool() == Some(true) {
            let first = mesh["first"].as_u64().unwrap_or(0) as usize;
            two_sided[first..first + mesh["vertices"].as_u64().unwrap_or(0) as usize].fill(true);
        }
    }
    Source { col: triples(dir.u8s(name("colour"))), skin: parts.iter().map(|p| ([*p, *p], 1.0)).collect(), two_sided, tris: triples(&dir.u32s(name("index"))), pos }
}

const PART_NAMES: [&str; part::WEAPON] = ["body", "magazine", "bolt", "charging", "trigger", "upper arm R", "forearm R", "hand R", "upper arm L", "forearm L", "hand L"];

fn held(dir: &Dir, meta: &Value, motions: &[Motion], info: &Info, budgets: [usize; 3]) -> (Built, String) {
    let source = held_source(dir, meta);
    // A part is simplified alone; the weapon's parts share one budget and each arm has its own.
    let mut groups: Vec<Group> = (0..part::WEAPON).map(|p| Group { name: PART_NAMES[p].to_string(), idx: Vec::new(), class: if p < part::UPPER_ARM_R { 0 } else if p < part::UPPER_ARM_L { 1 } else { 2 } }).collect();
    for t in &source.tris {
        groups[source.skin[t[0] as usize].0[0] as usize].idx.extend_from_slice(t);
    }
    let positions: Vec<u8> = source.pos.iter().flat_map(|p| on_screen(*p).into_iter().flat_map(|c| c.to_le_bytes())).collect();

    // Every pose the eye sees the weapon in: at rest, on the way to aiming and aimed, and through
    // its clips; each also with the eye turned a little, as sway, recoil and the walk turn it.
    let mut moments = vec![vec![figure::UNMOVED; part::WEAPON]];
    for share in [0.5f32, 1.0] {
        moments.push(vec![carried(info.ads_turn.map(|a| a * share), info.ads_offset.map(|a| a * share)); part::WEAPON]);
    }
    // (The reference also turns the weapon aside for a sprint and lowers it over a ledge.)
    for other in ["sprint", "lowered"] {
        if meta[other].is_object() {
            moments.push(vec![carried(list3(&meta[other]["turn"]), list3(&meta[other]["offset"])); part::WEAPON]);
        }
    }
    for m in motions.iter().filter(|m| m.clip.frames > 1) {
        let bones = m.clip.bones as usize;
        moments.extend((0..m.clip.frames as usize).step_by(3).map(|f| m.poses[f * bones..(f + 1) * bones].to_vec()));
    }
    let turn = 6.0f32.to_radians();
    let eye = View::eye(960, 544, 64.0);
    let views = vec![eye, eye.turned(turn, 0.0), eye.turned(-turn, 0.0), eye.turned(0.0, turn), eye.turned(0.0, -turn)];
    let stances: Vec<Stance> = moments.into_iter().map(|poses| Stance { poses, views: views.clone() }).collect();

    let (kept, errors) = fit(&source, &groups, &positions, &stances, &budgets, (0.0006, 0.08));
    let corner_normals: Vec<[V3; 3]> = normals(&source, &kept, 0.5);
    let mut draws = Vec::new();
    let mut per_part = Vec::new();
    for p in 0..part::WEAPON {
        // (a part the eye sees five triangles of or fewer, a trigger behind its guard, is not worth a draw)
        let which: Vec<usize> = (0..kept.len()).filter(|&t| kept[t].group == p).collect();
        let which = if which.len() < 6 { Vec::new() } else { which };
        per_part.push(format!("{} {}", PART_NAMES[p], which.len()));
        if !which.is_empty() {
            draws.push(draw(&source, &kept, &corner_normals, &which, vec![p as u8]));
        }
    }
    let made = built(meta["model"].as_u64().unwrap_or(0) as u32, part::WEAPON, draws);
    let line = format!(
        "{}: {} triangles → {} ({}), error {:.1} / {:.1} / {:.1} mm as the screen shows it at {:.0} cm, {} batches, {} vertices, {} bytes",
        meta["name"].as_str().unwrap_or("weapon"),
        source.tris.len(),
        made.model.triangles,
        per_part.join(", "),
        errors[0] * 1000.0,
        errors[1] * 1000.0,
        errors[2] * 1000.0,
        REFERENCE * 100.0,
        made.draws.len(),
        made.draws.iter().map(|d| d.vertices.len()).sum::<usize>(),
        made.bytes()
    );
    (made, line)
}

// ---------------------------------------------------------------- the section

fn budget(profile: &Value, key: &str, fallback: usize) -> usize {
    profile["figures"][key].as_u64().map_or(fallback, |n| n as usize)
}

/// Bones that never move against their parents in any clip (a hand's fingers, the top of the head)
/// are their parents for the purpose of a draw's bones.
fn aliases(parents: &[i64], motions: &[Motion]) -> Vec<u8> {
    let mut alias: Vec<u8> = (0..bone::SOLDIER as u8).collect();
    for b in 0..bone::SOLDIER.min(parents.len()) {
        let parent = parents[b];
        if parent < 0 {
            continue;
        }
        let rides = motions.iter().all(|m| {
            (0..m.clip.frames as usize).all(|f| {
                let (own, theirs) = (m.poses[f * bone::SOLDIER + b], m.poses[f * bone::SOLDIER + parent as usize]);
                (0..3).all(|r| (0..4).all(|c| (own[r][c] - theirs[r][c]).abs() < 1e-4))
            })
        });
        if rides {
            alias[b] = alias[parent as usize];
        }
    }
    alias
}

struct Cooked {
    models: Vec<Built>,
    motions: Vec<Motion>,
    infos: Vec<Info>,
    lines: Vec<String>,
}

fn all(ir: &Path, profile: &Value) -> Result<Option<Cooked>, String> {
    let path = ir.join("figures.dir");
    if !path.exists() {
        return Ok(None);
    }
    let dir = Dir::open(&path)?;
    let meta = &dir.meta;
    let mut cooked = Cooked { models: Vec::new(), motions: Vec::new(), infos: Vec::new(), lines: Vec::new() };

    let soldier_motions: Vec<Motion> = meta["clips"].as_array().into_iter().flatten().map(|c| Motion::read(&dir, c, model::VANGUARD, bone::SOLDIER)).collect();
    let parents: Vec<i64> = meta["rig"]["parents"].as_array().map(|a| a.iter().map(|v| v.as_i64().unwrap_or(-1)).collect()).unwrap_or_default();
    let alias = aliases(&parents, &soldier_motions);
    let joints: [V3; bone::SOLDIER] = core::array::from_fn(|b| list3(&meta["rig"]["joints"][b]));
    for s in meta["soldiers"].as_array().into_iter().flatten() {
        let (made, line) = soldier(&dir, s, &alias, &meta["rig"], &soldier_motions, budget(profile, "soldier", 700));
        cooked.infos.push(Info { model: made.model.id, muzzle: list3(&s["muzzle"]), joints, ..Default::default() });
        cooked.models.push(made);
        cooked.lines.push(line);
    }
    cooked.motions.extend(soldier_motions);

    for h in meta["held"].as_array().into_iter().flatten() {
        let id = h["model"].as_u64().unwrap_or(0) as u32;
        let motions: Vec<Motion> = h["clips"].as_array().into_iter().flatten().map(|c| Motion::read(&dir, c, id, part::WEAPON)).collect();
        let info = Info { model: id, muzzle: list3(&h["muzzle"]), eject: list3(&h["eject"]), eject_dir: list3(&h["ejectDir"]), bolt_travel: h["boltTravel"].as_f64().unwrap_or(0.0) as f32, ads_offset: list3(&h["adsOffset"]), ads_turn: list3(&h["adsTurn"]), ..Default::default() };
        let (made, line) = held(&dir, h, &motions, &info, [budget(profile, "weapon", 1600), budget(profile, "arm", 350), budget(profile, "arm", 350)]);
        cooked.models.push(made);
        cooked.lines.push(line);
        cooked.infos.push(info);
        cooked.motions.extend(motions);
    }
    Ok(Some(cooked))
}

/// The section's bytes; empty when the export holds no figures.
pub fn cook(ir: &Path, profile: &Value) -> Result<Vec<u8>, String> {
    let Some(cooked) = all(ir, profile)? else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    out.extend_from_slice(duty_pack::bytes_of(&[FigureHeader { models: cooked.models.len() as u32, clips: cooked.motions.len() as u32, infos: cooked.infos.len() as u32, pad: 0 }]));
    for m in &cooked.models {
        m.write(&mut out);
    }
    let models = out.len();
    for m in &cooked.motions {
        out.extend_from_slice(duty_pack::bytes_of(&[m.clip]));
        out.extend_from_slice(duty_pack::bytes_of(&m.poses));
    }
    let clips = out.len() - models;
    out.extend_from_slice(duty_pack::bytes_of(&cooked.infos));
    for line in &cooked.lines {
        println!("figures: {line}");
    }
    for m in &cooked.motions {
        println!("figures: clip {} of model {}: {} frames of {} bones at {:.3} a second{}, {} bytes", m.clip.id, m.clip.model, m.clip.frames, m.clip.bones, m.clip.rate, if m.clip.looped != 0 { ", looped" } else { "" }, core::mem::size_of::<Clip>() + m.poses.len() * core::mem::size_of::<Pose>());
    }
    println!("figures: {} models in {} bytes, {} clips in {} bytes, {} infos in {} bytes: {} bytes in all", cooked.models.len(), models - core::mem::size_of::<FigureHeader>(), cooked.motions.len(), clips, cooked.infos.len(), out.len() - models - clips, out.len());
    Ok(out)
}

/// The export's own meshes, drawn as `figure_shot` draws the cooked ones: what the lowering started from.
pub fn source_pictures(ir: &Path, out: &Path) -> Result<(), String> {
    let dir = Dir::open(&ir.join("figures.dir"))?;
    let shade = |c: [u8; 3]| c.map(|x| x as f32 / 255.0);
    for s in dir.meta["soldiers"].as_array().into_iter().flatten() {
        let source = body(&dir, s);
        let mut sheet = crate::tex::Image::new(460, 400);
        for (i, toward) in [unit([-0.6, 0.18, 0.8]), [-1.0, 0.0, 0.0]].into_iter().enumerate() {
            let view = View::flat(230, 400, toward, [0.0, 0.92, 0.1], 2.1);
            let mut canvas = Canvas::new(230, 400, [0.42, 0.44, 0.46], 1.0);
            for t in &source.tris {
                canvas.triangle(&view, t.map(|v| source.pos[v as usize]), t.map(|v| shade(source.col[v as usize])));
            }
            for y in 0..400 {
                sheet.data[y * 460 + i * 230..y * 460 + (i + 1) * 230].copy_from_slice(&canvas.image.data[y * 230..(y + 1) * 230]);
            }
        }
        sheet.save(&out.join(format!("{}_source.png", s["name"].as_str().unwrap_or("soldier"))))?;
    }
    for h in dir.meta["held"].as_array().into_iter().flatten() {
        let source = held_source(&dir, h);
        let view = View::eye(480, 272, 60.0);
        let mut canvas = Canvas::new(480, 272, [0.62, 0.64, 0.66], 2.2);
        for t in &source.tris {
            let (p, c) = (t.map(|v| source.pos[v as usize]), t.map(|v| shade(source.col[v as usize])));
            canvas.triangle(&view, p, c);
            if source.two_sided[t[0] as usize] {
                canvas.triangle(&view, [p[0], p[2], p[1]], [c[0], c[2], c[1]]);
            }
        }
        canvas.image.save(&out.join(format!("{}_source.png", h["name"].as_str().unwrap_or("weapon"))))?;
    }
    Ok(())
}
