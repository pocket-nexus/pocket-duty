//! Figures: the soldiers and the weapons in the player's hands.
//!
//! A figure is a model and its clips. A model is batches of triangles, each
//! bound to at most [`BATCH_BONES`] bones, which is what a GE draw blends. A
//! clip is frames of one matrix per bone: the matrix takes a vertex as the
//! model stores it (the bind pose) to where the bone carries it, in the
//! figure's own space. For a soldier that space has the feet at the origin,
//! +Y up and the soldier facing +Z; for a weapon in the hands it is the eye's:
//! +X right, +Y up, -Z ahead.
//!
//! The section `FIGS` is laid out as
//!
//! ```text
//! FigureHeader
//! Model × models      each followed by its batches: Batch, vertices, indices (u16), padded to 4 bytes
//! Clip × clips        each followed by frames × bones × Pose
//! Info × infos
//! ```
//!
//! and everything in it is found by walking it once (`Figures::parse`).

/// Bones one batch blends at most.
pub const BATCH_BONES: usize = 8;

pub mod model {
    /// The three soldiers.
    pub const VANGUARD: u32 = 0;
    pub const IRREGULAR: u32 = 1;
    pub const BREACHER: u32 = 2;
    /// A soldier's plainer drawing, for one too far off to show the difference, is his number
    /// plus this. A pack may hold none.
    pub const FAR: u32 = 4;
    /// The weapons as the player holds them, each with the arms that hold it.
    pub const RIFLE: u32 = 16;
    pub const SMG: u32 = 17;
    pub const PISTOL: u32 = 18;
}

/// A soldier's bones, in the reference rig's order.
pub mod bone {
    pub const HIPS: usize = 0;
    pub const SPINE: usize = 1;
    pub const SPINE1: usize = 2;
    pub const SPINE2: usize = 3;
    pub const NECK: usize = 4;
    pub const HEAD: usize = 5;
    pub const HEAD_TOP: usize = 6;
    pub const CLAVICLE_R: usize = 7;
    pub const UPPER_ARM_R: usize = 8;
    pub const FOREARM_R: usize = 9;
    pub const HAND_R: usize = 10;
    pub const FINGERS_R: usize = 11;
    pub const CLAVICLE_L: usize = 12;
    pub const UPPER_ARM_L: usize = 13;
    pub const FOREARM_L: usize = 14;
    pub const HAND_L: usize = 15;
    pub const FINGERS_L: usize = 16;
    pub const UP_LEG_R: usize = 17;
    pub const LEG_R: usize = 18;
    pub const FOOT_R: usize = 19;
    pub const TOE_R: usize = 20;
    pub const UP_LEG_L: usize = 21;
    pub const LEG_L: usize = 22;
    pub const FOOT_L: usize = 23;
    pub const TOE_L: usize = 24;
    pub const SOLDIER: usize = 25;
}

/// A held weapon's parts, each a bone of its model.
pub mod part {
    pub const BODY: usize = 0;
    pub const MAGAZINE: usize = 1;
    /// The bolt of the rifle and the submachine gun, the slide of the pistol.
    pub const BOLT: usize = 2;
    pub const CHARGING: usize = 3;
    pub const TRIGGER: usize = 4;
    pub const UPPER_ARM_R: usize = 5;
    pub const FOREARM_R: usize = 6;
    pub const HAND_R: usize = 7;
    pub const UPPER_ARM_L: usize = 8;
    pub const FOREARM_L: usize = 9;
    pub const HAND_L: usize = 10;
    pub const WEAPON: usize = 11;
}

/// A soldier's clips. Every soldier model plays the same ones.
pub mod clip {
    /// Standing, the rifle at the shoulder, aimed level ahead.
    pub const AIM: u32 = 0;
    /// One stride of walking with the rifle aimed; of running; of walking with it carried low.
    pub const WALK_AIM: u32 = 1;
    pub const RUN: u32 = 2;
    pub const PATROL: u32 = 3;
    /// Standing at ease.
    pub const IDLE: u32 = 4;
    pub const CROUCH_AIM: u32 = 5;
    pub const CROUCH_WALK: u32 = 6;
    /// One round's kick from `AIM`, a change of magazine, a flinch.
    pub const FIRE: u32 = 7;
    pub const RELOAD: u32 = 8;
    pub const HIT: u32 = 9;
    pub const SOLDIER: u32 = 10;
    /// A held weapon's clips, numbered per weapon from `HELD`: at rest in the hip pose, a change of
    /// a part-used magazine, of an empty one, drawing, lowering.
    pub const HELD: u32 = 32;
    pub const REST: u32 = 0;
    pub const RELOAD_TAC: u32 = 1;
    pub const RELOAD_EMPTY: u32 = 2;
    pub const DRAW: u32 = 3;
    pub const HOLSTER: u32 = 4;
    pub const HELD_CLIPS: u32 = 5;
    /// `HELD + weapon × HELD_CLIPS + REST…` is the clip's id, `weapon` being 0 rifle, 1 submachine gun, 2 pistol.
    pub const fn held(weapon: u32, which: u32) -> u32 {
        HELD + weapon * HELD_CLIPS + which
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct FigureHeader {
    pub models: u32,
    pub clips: u32,
    pub infos: u32,
    pub pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct Model {
    pub id: u32,
    pub bones: u32,
    pub batches: u32,
    pub triangles: u32,
    /// The box around the bind pose.
    pub min: [f32; 3],
    pub max: [f32; 3],
}

/// One draw: `vertices` [`SkinVertex`] and `indices` u16 follow it.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct Batch {
    /// The model's bones this batch blends, `bone_count` of them.
    pub bones: [u8; BATCH_BONES],
    pub bone_count: u32,
    pub vertices: u32,
    pub indices: u32,
}

/// A vertex in the GE's component order: weights, colour, normal, position.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct SkinVertex {
    /// How much of each of the batch's first four bones moves it, over 128 (the GE's unit for bytes).
    /// A batch may list more bones than four only when no vertex of it needs a fifth.
    pub weights: [u8; 4],
    /// The surface's colour before light, ABGR.
    pub colour: u32,
    pub normal: [i8; 3],
    pub pad: i8,
    pub pos: [f32; 3],
}

/// A bone's matrix in one frame: three rows of four, `[row][column]`, applied to a bind-pose
/// position as a column vector with 1 as its fourth component.
pub type Pose = [[f32; 4]; 3];

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct Clip {
    pub id: u32,
    /// The model whose bones its frames move (every soldier model shares the first's clips: `model` is then `VANGUARD`).
    pub model: u32,
    pub bones: u32,
    pub frames: u32,
    /// Frames a second.
    pub rate: f32,
    /// 1 when the frame after the last is the first again.
    pub looped: u32,
}

/// Points and numbers of a model that the simulation needs, in the model's bind pose.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct Info {
    pub model: u32,
    /// Soldier: the rifle's muzzle, carried by `bone::HAND_R`. Held weapon: its muzzle, carried by `part::BODY`.
    pub muzzle: [f32; 3],
    /// Held weapon: where a spent case leaves it and the way it flies, carried by `part::BODY`.
    pub eject: [f32; 3],
    pub eject_dir: [f32; 3],
    /// Held weapon: metres the bolt or slide travels back along +Z of the weapon's body when a round fires.
    pub bolt_travel: f32,
    /// Held weapon: the place and the turn (Euler XYZ, radians) that carry the hip pose's parts to
    /// the pose for aiming down the sights, in the eye's space.
    pub ads_offset: [f32; 3],
    pub ads_turn: [f32; 3],
    /// Held weapon: the same for a sprint, which lowers the weapon and turns it aside. The turn is
    /// about the eye, so the place holds what brings the weapon back in front of it: a turn and
    /// the plain difference of the two poses' places would swing the weapon out of the view.
    pub sprint_offset: [f32; 3],
    pub sprint_turn: [f32; 3],
    /// Soldier: each bone's position in the bind pose (hit zones run between them).
    pub joints: [[f32; 3]; bone::SOLDIER],
}

// ---------------------------------------------------------------- reading a section

use core::mem::size_of;

use crate::read;

/// A bone that stays where the bind pose has it.
pub const UNMOVED: Pose = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]];

/// Where a pose carries a bind-pose position.
pub fn apply(pose: &Pose, p: [f32; 3]) -> [f32; 3] {
    core::array::from_fn(|r| pose[r][0] * p[0] + pose[r][1] * p[1] + pose[r][2] * p[2] + pose[r][3])
}

/// Where a pose turns a direction.
pub fn turn(pose: &Pose, d: [f32; 3]) -> [f32; 3] {
    core::array::from_fn(|r| pose[r][0] * d[0] + pose[r][1] * d[1] + pose[r][2] * d[2])
}

/// The pose that does `first`, then `second`.
pub fn then(first: &Pose, second: &Pose) -> Pose {
    core::array::from_fn(|r| {
        core::array::from_fn(|c| second[r][0] * first[0][c] + second[r][1] * first[1][c] + second[r][2] * first[2][c] + if c == 3 { second[r][3] } else { 0.0 })
    })
}

/// A share `t` of the way from pose `a` to pose `b`: the twelve numbers mixed, then each row of
/// the turn pulled back toward unit length (one Newton step, which is enough for the few degrees
/// between two frames of a clip).
pub fn blend(a: &Pose, b: &Pose, t: f32) -> Pose {
    let mut out: Pose = core::array::from_fn(|r| core::array::from_fn(|c| a[r][c] + (b[r][c] - a[r][c]) * t));
    for row in &mut out {
        let k = 1.5 - 0.5 * (row[0] * row[0] + row[1] * row[1] + row[2] * row[2]);
        for c in 0..3 {
            row[c] *= k;
        }
    }
    out
}

/// A `FIGS` section, walked once: where its models, its clips and its points are.
#[derive(Clone, Copy)]
pub struct Figures<'a> {
    bytes: &'a [u8],
    pub header: FigureHeader,
    clips_at: usize,
    infos_at: usize,
}

/// A model and the bytes of its batches.
#[derive(Clone, Copy)]
pub struct ModelRef<'a> {
    pub model: Model,
    bytes: &'a [u8],
    at: usize,
}

/// One draw of a model: its record, its vertices ([`SkinVertex`] × `batch.vertices`) and its
/// indices (`u16` × `batch.indices`), both where the section holds them (on 4-byte boundaries of it).
#[derive(Clone, Copy)]
pub struct BatchRef<'a> {
    pub batch: Batch,
    pub vertices: &'a [u8],
    pub indices: &'a [u8],
}

/// A clip and its poses: `clip.frames` × `clip.bones` × [`Pose`].
#[derive(Clone, Copy)]
pub struct ClipRef<'a> {
    pub clip: Clip,
    pub poses: &'a [u8],
}

/// A batch at `at`, and where the next record starts.
fn batch_at(bytes: &[u8], at: usize) -> Option<(BatchRef<'_>, usize)> {
    let batch: Batch = read(bytes, at)?;
    let vertices_at = at + size_of::<Batch>();
    let indices_at = vertices_at + batch.vertices as usize * size_of::<SkinVertex>();
    let end = indices_at + batch.indices as usize * 2;
    let next = (end + 3) & !3;
    if batch.bone_count as usize > BATCH_BONES || next > bytes.len() {
        return None;
    }
    Some((BatchRef { batch, vertices: &bytes[vertices_at..indices_at], indices: &bytes[indices_at..end] }, next))
}

/// A model at `at`, and where the next record starts.
fn model_at(bytes: &[u8], at: usize) -> Option<(ModelRef<'_>, usize)> {
    let model: Model = read(bytes, at)?;
    let first = at + size_of::<Model>();
    let mut next = first;
    for _ in 0..model.batches {
        next = batch_at(bytes, next)?.1;
    }
    Some((ModelRef { model, bytes, at: first }, next))
}

/// A clip at `at`, and where the next record starts.
fn clip_at(bytes: &[u8], at: usize) -> Option<(ClipRef<'_>, usize)> {
    let clip: Clip = read(bytes, at)?;
    let first = at + size_of::<Clip>();
    let next = first + clip.frames as usize * clip.bones as usize * size_of::<Pose>();
    if clip.frames == 0 || next > bytes.len() {
        return None;
    }
    Some((ClipRef { clip, poses: &bytes[first..next] }, next))
}

impl<'a> Figures<'a> {
    /// `None` when the bytes are not a whole section.
    pub fn parse(bytes: &'a [u8]) -> Option<Figures<'a>> {
        let header: FigureHeader = read(bytes, 0)?;
        let mut at = size_of::<FigureHeader>();
        for _ in 0..header.models {
            at = model_at(bytes, at)?.1;
        }
        let clips_at = at;
        for _ in 0..header.clips {
            at = clip_at(bytes, at)?.1;
        }
        if at + header.infos as usize * size_of::<Info>() > bytes.len() {
            return None;
        }
        Some(Figures { bytes, header, clips_at, infos_at: at })
    }

    pub fn models(&self) -> Models<'a> {
        Models { bytes: self.bytes, at: size_of::<FigureHeader>(), left: self.header.models }
    }

    pub fn model(&self, id: u32) -> Option<ModelRef<'a>> {
        self.models().find(|m| m.model.id == id)
    }

    pub fn clips(&self) -> Clips<'a> {
        Clips { bytes: self.bytes, at: self.clips_at, left: self.header.clips }
    }

    pub fn clip(&self, id: u32) -> Option<ClipRef<'a>> {
        self.clips().find(|c| c.clip.id == id)
    }

    pub fn infos(&self) -> impl Iterator<Item = Info> + 'a {
        let (bytes, at) = (self.bytes, self.infos_at);
        (0..self.header.infos as usize).filter_map(move |i| read(bytes, at + i * size_of::<Info>()))
    }

    /// The points of model `model`.
    pub fn info(&self, model: u32) -> Option<Info> {
        self.infos().find(|i| i.model == model)
    }
}

pub struct Models<'a> {
    bytes: &'a [u8],
    at: usize,
    left: u32,
}

impl<'a> Iterator for Models<'a> {
    type Item = ModelRef<'a>;
    fn next(&mut self) -> Option<ModelRef<'a>> {
        if self.left == 0 {
            return None;
        }
        let (model, next) = model_at(self.bytes, self.at)?;
        self.at = next;
        self.left -= 1;
        Some(model)
    }
}

pub struct Clips<'a> {
    bytes: &'a [u8],
    at: usize,
    left: u32,
}

impl<'a> Iterator for Clips<'a> {
    type Item = ClipRef<'a>;
    fn next(&mut self) -> Option<ClipRef<'a>> {
        if self.left == 0 {
            return None;
        }
        let (clip, next) = clip_at(self.bytes, self.at)?;
        self.at = next;
        self.left -= 1;
        Some(clip)
    }
}

pub struct Batches<'a> {
    bytes: &'a [u8],
    at: usize,
    left: u32,
}

impl<'a> Iterator for Batches<'a> {
    type Item = BatchRef<'a>;
    fn next(&mut self) -> Option<BatchRef<'a>> {
        if self.left == 0 {
            return None;
        }
        let (batch, next) = batch_at(self.bytes, self.at)?;
        self.at = next;
        self.left -= 1;
        Some(batch)
    }
}

impl<'a> ModelRef<'a> {
    pub fn batches(&self) -> Batches<'a> {
        Batches { bytes: self.bytes, at: self.at, left: self.model.batches }
    }
}

impl<'a> BatchRef<'a> {
    /// The model's bones this batch blends.
    pub fn bones(&self) -> &[u8] {
        &self.batch.bones[..self.batch.bone_count as usize]
    }

    pub fn vertex(&self, i: usize) -> Option<SkinVertex> {
        read(self.vertices, i * size_of::<SkinVertex>())
    }

    pub fn index(&self, i: usize) -> Option<u16> {
        read(self.indices, i * 2)
    }

    /// Where vertex `v` stands when the model's bones are at `poses` (one per bone of the model).
    pub fn place(&self, v: &SkinVertex, poses: &[Pose]) -> [f32; 3] {
        let mut out = [0.0f32; 3];
        for (i, bone) in self.bones().iter().take(4).enumerate() {
            if v.weights[i] == 0 {
                continue;
            }
            let (p, w) = (apply(&poses[*bone as usize], v.pos), v.weights[i] as f32 / 128.0);
            for k in 0..3 {
                out[k] += p[k] * w;
            }
        }
        out
    }
}

impl<'a> ClipRef<'a> {
    /// The pose of `bone` in `frame`.
    pub fn pose(&self, frame: u32, bone: u32) -> Pose {
        read(self.poses, (frame.min(self.clip.frames - 1) * self.clip.bones + bone) as usize * size_of::<Pose>()).unwrap_or(UNMOVED)
    }

    /// Seconds one pass of the clip lasts: a clip that loops goes from its last frame on to its
    /// first, one that ends rests on its last.
    pub fn seconds(&self) -> f32 {
        (if self.clip.looped != 0 { self.clip.frames } else { self.clip.frames - 1 }) as f32 / self.clip.rate
    }

    /// The two frames around `seconds` into the clip and the share of the second one.
    pub fn frames_at(&self, seconds: f32) -> (u32, u32, f32) {
        let frames = self.clip.frames;
        let x = if seconds > 0.0 { seconds * self.clip.rate } else { 0.0 };
        if self.clip.looped != 0 {
            let whole = (x / frames as f32) as u32;
            let x = x - (whole * frames) as f32;
            let i = (x as u32).min(frames - 1);
            (i, (i + 1) % frames, x - i as f32)
        } else {
            let i = (x as u32).min(frames - 1);
            (i, (i + 1).min(frames - 1), if i + 1 < frames { x - i as f32 } else { 0.0 })
        }
    }

    /// The pose of `bone` at `seconds` into the clip, between the two frames around it.
    pub fn pose_at(&self, seconds: f32, bone: u32) -> Pose {
        let (a, b, t) = self.frames_at(seconds);
        if t <= 0.0 || a == b {
            return self.pose(a, bone);
        }
        blend(&self.pose(a, bone), &self.pose(b, bone), t)
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::bytes_of;
    use std::vec::Vec;

    #[test]
    fn a_section_reads_back() {
        let vertex = |x: f32, first: u8| SkinVertex { weights: [first, 128 - first, 0, 0], colour: 0xff00_00ff, normal: [0, 127, 0], pad: 0, pos: [x, 0.0, 0.0] };
        let lift: Pose = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 2.0], [0.0, 0.0, 1.0, 0.0]];
        let mut bytes: Vec<u8> = Vec::new();
        bytes.extend_from_slice(bytes_of(&[FigureHeader { models: 1, clips: 1, infos: 1, pad: 0 }]));
        bytes.extend_from_slice(bytes_of(&[Model { id: model::RIFLE, bones: 2, batches: 1, triangles: 1, min: [0.0; 3], max: [2.0, 0.0, 0.0] }]));
        bytes.extend_from_slice(bytes_of(&[Batch { bones: [1, 0, 0, 0, 0, 0, 0, 0], bone_count: 2, vertices: 3, indices: 3 }]));
        bytes.extend_from_slice(bytes_of(&[vertex(0.0, 128), vertex(1.0, 64), vertex(2.0, 0)]));
        // (three indices are six bytes: the batch ends two bytes short of a 4-byte boundary)
        bytes.extend_from_slice(bytes_of(&[0u16, 1, 2, 0]));
        bytes.extend_from_slice(bytes_of(&[Clip { id: clip::held(0, clip::DRAW), model: model::RIFLE, bones: 2, frames: 2, rate: 30.0, looped: 0 }]));
        bytes.extend_from_slice(bytes_of(&[UNMOVED, UNMOVED, UNMOVED, lift]));
        bytes.extend_from_slice(bytes_of(&[Info { model: model::RIFLE, bolt_travel: 0.062, ..Default::default() }]));

        let figures = Figures::parse(&bytes).unwrap();
        assert!(Figures::parse(&bytes[..bytes.len() - 1]).is_none());
        assert!(figures.model(model::SMG).is_none());
        let model = figures.model(model::RIFLE).unwrap();
        let batch = model.batches().next().unwrap();
        assert_eq!((batch.bones(), batch.index(2)), (&[1u8, 0][..], Some(2)));
        assert_eq!(figures.info(model::RIFLE).unwrap().bolt_travel, 0.062);

        // The batch's first bone is the model's bone 1, which the clip's second frame lifts by 2.
        let clip = figures.clip(clip::held(0, clip::DRAW)).unwrap();
        assert_eq!(clip.seconds(), 1.0 / 30.0);
        assert_eq!(clip.frames_at(0.5), (1, 1, 0.0));
        let poses = [clip.pose_at(1.0, 0), clip.pose_at(1.0, 1)];
        let heights: [f32; 3] = core::array::from_fn(|i| batch.place(&batch.vertex(i).unwrap(), &poses)[1]);
        assert_eq!(heights, [2.0, 1.0, 0.0]);
        assert_eq!(clip.pose_at(1.0 / 60.0, 1)[1][3], 1.0);
    }

    #[test]
    fn records_keep_their_sizes() {
        assert_eq!(size_of::<FigureHeader>(), 16);
        assert_eq!(size_of::<Model>(), 40);
        assert_eq!(size_of::<Batch>(), 20);
        assert_eq!(size_of::<SkinVertex>(), 24);
        assert_eq!(size_of::<Pose>(), 48);
        assert_eq!(size_of::<Clip>(), 24);
        // (model, muzzle, eject, eject_dir, bolt_travel, then a place and a turn for the sights and for a sprint)
        assert_eq!(size_of::<Info>(), 92 + bone::SOLDIER * 12);
    }

    #[test]
    fn poses_carry_and_mix() {
        let quarter: Pose = [[0.0, -1.0, 0.0, 1.0], [1.0, 0.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]];
        assert_eq!(apply(&quarter, [1.0, 0.0, 0.0]), [1.0, 1.0, 0.0]);
        assert_eq!(turn(&quarter, [1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]);
        assert_eq!(apply(&then(&quarter, &quarter), [1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]);
        assert_eq!(blend(&UNMOVED, &quarter, 0.0), UNMOVED);
        assert_eq!(blend(&UNMOVED, &quarter, 1.0), quarter);
    }
}
