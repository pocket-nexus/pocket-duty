//! The soldiers and the weapon in the player's hands as a device draws them:
//! the pack's figures (`duty_pack::figure`) and, for a frame, where each bone
//! is. A device skins and lights; what is posed is the same on all of them.

use alloc::vec::Vec;

use duty_pack::figure::{bone, clip, model, part, Batch, Clip, FigureHeader, Info, Model, Pose, SkinVertex};
use duty_pack::{self as pack, Scene, Texture};

use crate::game::Game;
use crate::math::*;
use crate::soldier::{Soldier, State};
use crate::tune::soldier as rules;
use crate::weapon::Action;

/// A draw of up to four bones: where its vertices and indices are in the figures' section.
#[derive(Clone, Copy, Debug)]
pub struct BatchAt {
    pub bones: [u8; 4],
    pub count: u32,
    pub vertices: usize,
    pub vertex_count: u32,
    pub indices: usize,
    pub index_count: u32,
}

pub struct ModelAt {
    pub id: u32,
    pub batches: Vec<BatchAt>,
}

struct ClipAt {
    head: Clip,
    /// Where its poses start in the section: `frames × bones` of them.
    poses: usize,
}

/// The figures' section, read once.
pub struct Figures<'a> {
    bytes: &'a [u8],
    pub models: Vec<ModelAt>,
    clips: Vec<ClipAt>,
    infos: Vec<Info>,
}

pub const IDENTITY: Pose = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]];

/// Metres from the eye at which a soldier is drawn from his plainer model.
pub const FAR_FROM: f32 = 16.0;

fn blend(a: &Pose, b: &Pose, t: f32) -> Pose {
    core::array::from_fn(|r| core::array::from_fn(|c| a[r][c] + (b[r][c] - a[r][c]) * t))
}

/// `a` after `b`.
pub fn after(a: &Pose, b: &Pose) -> Pose {
    core::array::from_fn(|r| core::array::from_fn(|c| a[r][0] * b[0][c] + a[r][1] * b[1][c] + a[r][2] * b[2][c] + if c == 3 { a[r][3] } else { 0.0 }))
}

pub fn apply(m: &Pose, p: [f32; 3]) -> V3 {
    v3(m[0][0] * p[0] + m[0][1] * p[1] + m[0][2] * p[2] + m[0][3], m[1][0] * p[0] + m[1][1] * p[1] + m[1][2] * p[2] + m[1][3], m[2][0] * p[0] + m[2][1] * p[1] + m[2][2] * p[2] + m[2][3])
}

/// A turn of `angle` about the axis `axis` through the point `at`, then a move by `by`.
pub fn turn_about(axis: V3, angle: f32, at: V3, by: V3) -> Pose {
    let (s, c) = (sin(angle), cos(angle));
    let (x, y, z) = (axis.x, axis.y, axis.z);
    let r = [[c + x * x * (1.0 - c), x * y * (1.0 - c) - z * s, x * z * (1.0 - c) + y * s], [y * x * (1.0 - c) + z * s, c + y * y * (1.0 - c), y * z * (1.0 - c) - x * s], [z * x * (1.0 - c) - y * s, z * y * (1.0 - c) + x * s, c + z * z * (1.0 - c)]];
    let moved: [f32; 3] = core::array::from_fn(|i| [at.x, at.y, at.z][i] - (r[i][0] * at.x + r[i][1] * at.y + r[i][2] * at.z) + [by.x, by.y, by.z][i]);
    core::array::from_fn(|i| [r[i][0], r[i][1], r[i][2], moved[i]])
}

/// Three turns about the eye's axes, the one about z first, then the one about y, then x (three.js
/// writes this order as XYZ), then a move: the reference's own way of writing where the hands hold
/// a weapon.
fn euler(turn: [f32; 3], by: [f32; 3]) -> Pose {
    let x = turn_about(v3(1.0, 0.0, 0.0), turn[0], V3::ZERO, v3(by[0], by[1], by[2]));
    let y = turn_about(v3(0.0, 1.0, 0.0), turn[1], V3::ZERO, V3::ZERO);
    let z = turn_about(v3(0.0, 0.0, 1.0), turn[2], V3::ZERO, V3::ZERO);
    after(&x, &after(&y, &z))
}

/// The weapon in the player's hands for a frame.
pub struct Held {
    pub model: u32,
    pub poses: [Pose; part::WEAPON],
    /// Degrees, top to bottom, of the frustum it is drawn in.
    pub fov: f32,
    /// Where its muzzle is, counted from the eye in the eye's own axes.
    pub muzzle: V3,
}

impl<'a> Figures<'a> {
    /// Reads the figures; `None` when the section is not one.
    pub fn parse(bytes: &'a [u8]) -> Option<Figures<'a>> {
        let head: FigureHeader = pack::read(bytes, 0)?;
        let mut at = core::mem::size_of::<FigureHeader>();
        let mut models = Vec::new();
        for _ in 0..head.models {
            let m: Model = pack::read(bytes, at)?;
            at += core::mem::size_of::<Model>();
            let mut batches = Vec::new();
            for _ in 0..m.batches {
                let b: Batch = pack::read(bytes, at)?;
                at += core::mem::size_of::<Batch>();
                let (vertices, indices) = (at, at + b.vertices as usize * core::mem::size_of::<SkinVertex>());
                at = (indices + b.indices as usize * 2 + 3) & !3;
                if at > bytes.len() {
                    return None;
                }
                batches.push(BatchAt { bones: [b.bones[0], b.bones[1], b.bones[2], b.bones[3]], count: b.bone_count.min(4), vertices, vertex_count: b.vertices, indices, index_count: b.indices });
            }
            models.push(ModelAt { id: m.id, batches });
        }
        let mut clips = Vec::new();
        for _ in 0..head.clips {
            let c: Clip = pack::read(bytes, at)?;
            at += core::mem::size_of::<Clip>();
            clips.push(ClipAt { head: c, poses: at });
            at += (c.frames * c.bones) as usize * core::mem::size_of::<Pose>();
            if at > bytes.len() {
                return None;
            }
        }
        let infos = (0..head.infos as usize).filter_map(|i| pack::read::<Info>(bytes, at + i * core::mem::size_of::<Info>())).collect();
        Some(Figures { bytes, models, clips, infos })
    }

    pub fn model(&self, id: u32) -> Option<&ModelAt> {
        self.models.iter().find(|m| m.id == id)
    }

    fn info(&self, model: u32) -> Option<&Info> {
        self.infos.iter().find(|i| i.model == model)
    }

    /// The model a soldier is drawn from at `distance` from the eye: his plainer one far off, when
    /// the pack has it.
    pub fn soldier_model(&self, s: &Soldier, distance: f32) -> u32 {
        let id = model::VANGUARD + s.variant as u32 % 3;
        let plain = id + model::FAR;
        if distance > FAR_FROM && self.model(plain).is_some() {
            plain
        } else {
            id
        }
    }

    /// A bone's pose in a clip at `at`: turns of the clip when it loops, seconds when it does not.
    fn pose(&self, id: u32, bone: usize, at: f32) -> Pose {
        let Some(c) = self.clips.iter().find(|c| c.head.id == id) else { return IDENTITY };
        let (frames, bones) = (c.head.frames as usize, c.head.bones as usize);
        if frames == 0 || bone >= bones {
            return IDENTITY;
        }
        let f = if c.head.looped != 0 { (at - floor(at)) * frames as f32 } else { clamp(at * c.head.rate, 0.0, frames as f32 - 1.0) };
        let i = (f as usize).min(frames - 1);
        let next = if c.head.looped != 0 { (i + 1) % frames } else { (i + 1).min(frames - 1) };
        let read = |frame: usize| pack::read::<Pose>(self.bytes, c.poses + (frame * bones + bone) * core::mem::size_of::<Pose>()).unwrap_or(IDENTITY);
        blend(&read(i), &read(next), f - i as f32)
    }

    /// A soldier's bones: his clip, the one he leaves, what plays over them, his aim, his fall.
    pub fn soldier(&self, s: &Soldier, poses: &mut [Pose; bone::SOLDIER]) {
        let still = matches!(s.clip, clip::AIM | clip::CROUCH_AIM | clip::IDLE);
        // What plays instead of standing still: a flinch, a change of magazine, a round's kick.
        let over = if s.state == State::Dead || !still {
            None
        } else if s.hit_ago < 0.55 {
            Some((clip::HIT, s.hit_ago))
        } else if s.reload > 0.0 {
            Some((clip::RELOAD, rules::RELOAD[(s.variant == 1) as usize] - s.reload))
        } else if s.fired_ago < 0.3 && s.clip == clip::AIM {
            Some((clip::FIRE, s.fired_ago))
        } else {
            None
        };
        for (b, pose) in poses.iter_mut().enumerate() {
            *pose = match over {
                Some((id, at)) => self.pose(id, b, at),
                None => {
                    let now = self.pose(s.clip, b, s.phase);
                    if s.blend < 1.0 {
                        blend(&self.pose(s.clip_before, b, s.phase_before), &now, s.blend)
                    } else {
                        now
                    }
                }
            };
        }
        let Some(info) = self.info(model::VANGUARD) else { return };
        if abs(s.aim_pitch) > 0.01 && s.state != State::Dead {
            // The chest, the arms and the head turn up or down about the small of the back. The
            // model faces +Z with its right at -X: a turn about +X lowers what is ahead.
            let pivot = apply(&poses[bone::SPINE1], info.joints[bone::SPINE1]);
            let lift = turn_about(v3(1.0, 0.0, 0.0), -s.aim_pitch, pivot, V3::ZERO);
            for pose in poses.iter_mut().take(bone::FINGERS_L + 1).skip(bone::SPINE1) {
                *pose = after(&lift, pose);
            }
        }
        if s.state == State::Dead {
            // He goes over backwards from the round that felled him, faster as he falls, and lies.
            let t = min(s.dead_time / 0.62, 1.0);
            let away = wrap_angle(s.fall_yaw - s.yaw);
            // In his own space the way he falls: -Z is his front.
            let axis = v3(cos(away), 0.0, sin(away));
            let fall = turn_about(axis, t * t * 1.5, V3::ZERO, v3(0.0, 0.08 * t, 0.0));
            for pose in poses.iter_mut() {
                *pose = after(&fall, pose);
            }
        }
    }

    /// The weapon in the player's hands; `None` while he is down or the pack has no model of it.
    pub fn held(&self, game: &Game) -> Option<Held> {
        let arms = &game.arms;
        let id = model::RIFLE + arms.active as u32;
        if game.player.dead || self.model(id).is_none() {
            return None;
        }
        let info = *self.info(id)?;
        let w = arms.active as u32;
        let (which, at) = match arms.action {
            Action::None => (clip::REST, 0.0),
            Action::Reload { empty, at, .. } => (if empty { clip::RELOAD_EMPTY } else { clip::RELOAD_TAC }, at),
            Action::Holster { at, .. } => (clip::HOLSTER, at),
            Action::Draw { at } => (clip::DRAW, at),
        };
        // Where the hands carry everything: up to the sights, back with a round, down in a sprint,
        // and with the walk.
        let aim = smoothstep(0.0, 1.0, arms.ads);
        let player = &game.player;
        let stride = sin(game.time * 9.0) * min(player.speed() / 4.57, 1.4) * lerp(1.0, 0.25, aim);
        let sprint = if player.sprinting { 1.0 } else { 0.0 };
        let carry = euler(
            [info.ads_turn[0] * aim - 0.35 * sprint - arms.back * 1.6, info.ads_turn[1] * aim + 0.52 * sprint, info.ads_turn[2] * aim + 0.33 * sprint + arms.roll],
            [info.ads_offset[0] * aim - 0.03 * sprint + stride * 0.006, info.ads_offset[1] * aim - 0.075 * sprint - abs(stride) * 0.005, info.ads_offset[2] * aim + 0.025 * sprint + arms.back],
        );
        let mut poses = [IDENTITY; part::WEAPON];
        for (b, pose) in poses.iter_mut().enumerate() {
            *pose = after(&carry, &self.pose(clip::held(w, which), b, at));
        }
        // The bolt goes back along the weapon's own length, which at the hip is not the eye's: it
        // is the eye's when the sights are up, so the turn that raises them says where it lies. A
        // change of an empty magazine moves the bolt itself.
        if arms.bolt > 0.0 && !matches!(arms.action, Action::Reload { empty: true, .. }) {
            let (body, raise) = (&poses[part::BODY], euler(info.ads_turn, [0.0; 3]));
            let along = raise[2];
            let back = v3(
                body[0][0] * along[0] + body[0][1] * along[1] + body[0][2] * along[2],
                body[1][0] * along[0] + body[1][1] * along[1] + body[1][2] * along[2],
                body[2][0] * along[0] + body[2][1] * along[1] + body[2][2] * along[2],
            ) * (info.bolt_travel * arms.bolt);
            let slide = turn_about(V3::UP, 0.0, V3::ZERO, back);
            poses[part::BOLT] = after(&slide, &poses[part::BOLT]);
        }
        Some(Held { model: id, fov: 60.0 * lerp(1.0, 0.86, aim), muzzle: apply(&poses[part::BODY], info.muzzle), poses })
    }
}

/// How much of the sun reaches the ground at a place, 0…1: a texel of the ground's shadow
/// picture, whose green at one half is all of the sun. A figure standing there is lit by that much.
pub fn sunlit(scene: &Scene, textures: &[Texture], texels: &[u8], x: f32, z: f32) -> f32 {
    let Some(t) = textures.get(scene.shadow_texture as usize) else { return 1.0 };
    let r = &scene.shadow_rows;
    let (u, v) = (x * r[0][0] + z * r[0][1] + r[0][2], x * r[1][0] + z * r[1][1] + r[1][2]);
    if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
        return 1.0;
    }
    let texel = t.texel(texels, 0, (u * t.width as f32) as usize, (v * t.height as f32) as usize);
    saturate(texel[1] as f32 / 127.0)
}
