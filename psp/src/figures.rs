//! The soldiers and the weapon in the player's hands: the pack's figures
//! (`duty_pack::figure`), posed from their clips and blended by the GE.
//!
//! A batch is one draw of up to four bones. A bone's matrix is the clip's
//! pose for it between two frames; the model matrix carries the figure to
//! where it stands. The GE lights a figure itself: the sky's share on every
//! side, the sun's on the sides turned to it, and the sun's only where the
//! ground's shadow picture has sun at the figure's feet.

use core::ffi::c_void;

use alloc::vec::Vec;
use duty_pack::figure::{self as figure, bone, clip, model, part, Batch, Clip, FigureHeader, Info, Model, Pose, SkinVertex};
use duty_pack::{self as pack};
use duty_sim::game::Game;
use duty_sim::mat::{self, Mat4};
use duty_sim::math::*;
use duty_sim::soldier::{Soldier, State};
use duty_sim::tune::soldier as rules;
use duty_sim::weapon::Action;
use psp::sys::*;

use crate::gfx::{self, abgr, fmatrix, Camera, Gfx};
use crate::store::{self, Kept};

fn vtype_skin() -> VertexType {
    VertexType::WEIGHTS4 | VertexType::WEIGHT_8BIT | VertexType::COLOR_8888 | VertexType::NORMAL_8BIT | VertexType::VERTEX_32BITF | VertexType::INDEX_16BIT | VertexType::TRANSFORM_3D
}

struct BatchAt {
    bones: [u8; 4],
    count: u32,
    vertices: *const u8,
    indices: *const u16,
    index_count: u32,
}

struct ModelAt {
    id: u32,
    batches: Vec<BatchAt>,
}

struct ClipAt {
    head: Clip,
    poses: *const Pose,
}

pub struct Figures {
    _bytes: Kept<u8>,
    models: Vec<ModelAt>,
    clips: Vec<ClipAt>,
    infos: Vec<Info>,
    /// Triangles drawn this frame.
    pub triangles: u32,
}

const IDENTITY: Pose = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]];

fn blend(a: &Pose, b: &Pose, t: f32) -> Pose {
    core::array::from_fn(|r| core::array::from_fn(|c| a[r][c] + (b[r][c] - a[r][c]) * t))
}

/// `a` after `b`.
fn after(a: &Pose, b: &Pose) -> Pose {
    core::array::from_fn(|r| core::array::from_fn(|c| a[r][0] * b[0][c] + a[r][1] * b[1][c] + a[r][2] * b[2][c] + if c == 3 { a[r][3] } else { 0.0 }))
}

fn apply(m: &Pose, p: [f32; 3]) -> V3 {
    v3(m[0][0] * p[0] + m[0][1] * p[1] + m[0][2] * p[2] + m[0][3], m[1][0] * p[0] + m[1][1] * p[1] + m[1][2] * p[2] + m[1][3], m[2][0] * p[0] + m[2][1] * p[1] + m[2][2] * p[2] + m[2][3])
}

/// A turn of `angle` about the axis `axis` through the point `at`, then a move by `by`.
fn turn_about(axis: V3, angle: f32, at: V3, by: V3) -> Pose {
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

fn bone_matrix(p: &Pose) -> ScePspFMatrix4 {
    let col = |c: usize, w: f32| ScePspFVector4 { x: p[0][c], y: p[1][c], z: p[2][c], w };
    ScePspFMatrix4 { x: col(0, 0.0), y: col(1, 0.0), z: col(2, 0.0), w: col(3, 1.0) }
}

impl Figures {
    /// Reads the figures; `None` when the pack has none.
    pub unsafe fn load(file: &store::PackFile) -> Option<Figures> {
        let bytes: Kept<u8> = file.resident(pack::FIGS).ok()?;
        let head: FigureHeader = pack::read(&bytes, 0)?;
        let mut at = core::mem::size_of::<FigureHeader>();
        let mut models = Vec::new();
        for _ in 0..head.models {
            let m: Model = pack::read(&bytes, at)?;
            at += core::mem::size_of::<Model>();
            let mut batches = Vec::new();
            for _ in 0..m.batches {
                let b: Batch = pack::read(&bytes, at)?;
                at += core::mem::size_of::<Batch>();
                let (vertices, indices) = (at, at + b.vertices as usize * core::mem::size_of::<SkinVertex>());
                at = (indices + b.indices as usize * 2 + 3) & !3;
                if at > bytes.len() {
                    return None;
                }
                batches.push(BatchAt { bones: [b.bones[0], b.bones[1], b.bones[2], b.bones[3]], count: b.bone_count.min(4), vertices: bytes.as_ptr().add(vertices), indices: bytes.as_ptr().add(indices) as *const u16, index_count: b.indices });
            }
            models.push(ModelAt { id: m.id, batches });
        }
        let mut clips = Vec::new();
        for _ in 0..head.clips {
            let c: Clip = pack::read(&bytes, at)?;
            at += core::mem::size_of::<Clip>();
            clips.push(ClipAt { head: c, poses: bytes.as_ptr().add(at) as *const Pose });
            at += (c.frames * c.bones) as usize * core::mem::size_of::<Pose>();
            if at > bytes.len() {
                return None;
            }
        }
        let infos = (0..head.infos as usize).filter_map(|i| pack::read::<Info>(&bytes, at + i * core::mem::size_of::<Info>())).collect();
        Some(Figures { _bytes: bytes, models, clips, infos, triangles: 0 })
    }

    fn info(&self, model: u32) -> Option<&Info> {
        self.infos.iter().find(|i| i.model == model)
    }

    /// A bone's pose in a clip at `at`: turns of the clip when it loops, seconds when it does not.
    unsafe fn pose(&self, id: u32, bone: usize, at: f32) -> Pose {
        let Some(c) = self.clips.iter().find(|c| c.head.id == id) else { return IDENTITY };
        let (frames, bones) = (c.head.frames as usize, c.head.bones as usize);
        if frames == 0 || bone >= bones {
            return IDENTITY;
        }
        let f = if c.head.looped != 0 { (at - floor(at)) * frames as f32 } else { clamp(at * c.head.rate, 0.0, frames as f32 - 1.0) };
        let i = (f as usize).min(frames - 1);
        let next = if c.head.looped != 0 { (i + 1) % frames } else { (i + 1).min(frames - 1) };
        blend(&*c.poses.add(i * bones + bone), &*c.poses.add(next * bones + bone), f - i as f32)
    }

    /// The light a figure at a place stands in.
    unsafe fn light(&self, gfx: &Gfx, at: V3, toward_sun: V3, up: V3) {
        let scene = &gfx.scene;
        let sun = gfx.sunlit(at.x, at.z);
        sceGuEnable(GuState::Lighting);
        sceGuEnable(GuState::Light0);
        sceGuEnable(GuState::Light1);
        sceGuLightMode(LightMode::SingleColor);
        sceGuColorMaterial(LightComponent::AMBIENT | LightComponent::DIFFUSE);
        sceGuLight(0, LightType::Directional, LightComponent::DIFFUSE, &ScePspFVector3 { x: toward_sun.x, y: toward_sun.y, z: toward_sun.z });
        sceGuLightColor(0, LightComponent::DIFFUSE, abgr(scene.sun_colour.map(|c| c * sun)));
        sceGuLightColor(0, LightComponent::AMBIENT, 0);
        // The sky's fill comes from above more than from the sides: 0.55 of it from everywhere and
        // 0.6 of it more on what faces up, so that a figure in the shade still has a form.
        sceGuLight(1, LightType::Directional, LightComponent::DIFFUSE, &ScePspFVector3 { x: up.x, y: up.y, z: up.z });
        sceGuLightColor(1, LightComponent::DIFFUSE, abgr(scene.sky_colour.map(|c| c * 0.6)));
        sceGuLightColor(1, LightComponent::AMBIENT, 0);
        sceGuAmbient(abgr(scene.sky_colour.map(|c| c * 0.55)));
    }

    unsafe fn draw_model(&mut self, model: u32, poses: &[Pose]) {
        let Some(m) = self.models.iter().find(|m| m.id == model) else { return };
        let mut triangles = 0;
        for b in &m.batches {
            for slot in 0..b.count as usize {
                sceGuBoneMatrix(slot as u32, &bone_matrix(&poses[(b.bones[slot] as usize).min(poses.len() - 1)]));
            }
            sceGuDrawArray(GuPrimitive::Triangles, vtype_skin(), b.index_count as i32, b.indices as *const c_void, b.vertices as *const c_void);
            triangles += b.index_count / 3;
        }
        self.triangles += triangles;
    }

    /// The soldiers, standing and fallen. The list is open, the world drawn, the eye the origin.
    pub unsafe fn soldiers(&mut self, gfx: &Gfx, camera: &Camera, game: &Game) {
        if self.models.is_empty() {
            return;
        }
        let look = forward(camera.yaw, camera.pitch);
        sceGuDisable(GuState::Texture2D);
        sceGuDisable(GuState::Blend);
        sceGuEnable(GuState::DepthTest);
        sceGuEnable(GuState::CullFace);
        sceGuDepthMask(0);
        let scene = gfx.scene;
        for s in game.soldiers.iter().filter(|s| s.used) {
            let to = s.pos + v3(0.0, 0.9, 0.0) - camera.eye;
            let distance = to.len();
            // Behind the eye, or too far to be more than a pixel.
            if distance > 110.0 || (to.dot(look) < -1.5) {
                continue;
            }
            let mut poses = [IDENTITY; bone::SOLDIER];
            self.pose_soldier(s, &mut poses);
            // Each soldier in the pass of the cell he stands in, as the world round him is.
            gfx.pass_of_place(camera, gfx::near_place(&scene, camera.eye, s.pos.x, s.pos.z));
            self.light(gfx, s.pos, v3(scene.sun[0], scene.sun[1], scene.sun[2]), V3::UP);
            // The model faces +Z; a soldier's heading at yaw 0 is -Z.
            let (sn, cs) = (sin(s.yaw + PI), cos(s.yaw + PI));
            let at = s.pos - camera.eye;
            let root: Mat4 = [cs, 0.0, sn, at.x, 0.0, 1.0, 0.0, at.y, -sn, 0.0, cs, at.z, 0.0, 0.0, 0.0, 1.0];
            sceGuSetMatrix(MatrixMode::Model, &fmatrix(&root));
            self.draw_model(model::VANGUARD + s.variant as u32 % 3, &poses);
        }
        sceGuDisable(GuState::Lighting);
        sceGuDisable(GuState::Light1);
    }

    /// A soldier's bones: his clip, the one he leaves, what plays over them, his aim, his fall.
    unsafe fn pose_soldier(&self, s: &Soldier, poses: &mut [Pose; bone::SOLDIER]) {
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

    /// The weapon in the player's hands, in a frustum and a depth range of its own. Returns where
    /// its muzzle is, counted from the eye in the eye's own axes.
    pub unsafe fn held(&mut self, gfx: &Gfx, view: &Mat4, game: &Game) -> Option<V3> {
        let arms = &game.arms;
        let id = model::RIFLE + arms.active as u32;
        if game.player.dead || !self.models.iter().any(|m| m.id == id) {
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
        sceGuSetMatrix(MatrixMode::Projection, &fmatrix(&mat::perspective_gl(60.0 * lerp(1.0, 0.86, aim), 480.0 / 272.0, 0.02, 6.0)));
        sceGuDepthRange(65535, 61440);
        let identity: Mat4 = mat::IDENTITY;
        sceGuSetMatrix(MatrixMode::View, &fmatrix(&identity));
        sceGuSetMatrix(MatrixMode::Model, &fmatrix(&identity));
        sceGuDisable(GuState::Texture2D);
        sceGuDisable(GuState::Blend);
        sceGuEnable(GuState::DepthTest);
        sceGuEnable(GuState::CullFace);
        sceGuDepthMask(0);
        // The sun's direction as the eye sees it.
        let sun = v3(gfx.scene.sun[0], gfx.scene.sun[1], gfx.scene.sun[2]);
        let in_view = v3(view[0] * sun.x + view[1] * sun.y + view[2] * sun.z, view[4] * sun.x + view[5] * sun.y + view[6] * sun.z, view[8] * sun.x + view[9] * sun.y + view[10] * sun.z);
        self.light(gfx, player.body.pos, in_view, v3(view[1], view[5], view[9]));
        self.draw_model(id, &poses);
        sceGuDisable(GuState::Lighting);
        sceGuDisable(GuState::Light1);
        Some(apply(&poses[part::BODY], info.muzzle))
    }
}

#[allow(dead_code)]
fn _records() {
    let _ = figure::BATCH_BONES;
}
