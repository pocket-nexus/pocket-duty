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
    /// Soldier: each bone's position in the bind pose (hit zones run between them).
    pub joints: [[f32; 3]; bone::SOLDIER],
}
