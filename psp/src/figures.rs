//! The soldiers and the weapon in the player's hands: the pack's figures
//! (`duty_pack::figure`), posed from their clips and blended by the GE.
//!
//! A batch is one draw of up to four bones. A bone's matrix is the clip's
//! pose for it between two frames; the model matrix carries the figure to
//! where it stands. The GE lights a figure itself: the sky's share on every
//! side, the sun's on the sides turned to it, and the sun's only where the
//! ground's shadow picture has sun at the figure's feet.

use duty_pack::figure::{self as figure, bone, Pose};
use duty_pack::{self as pack};
use duty_sim::game::Game;
use duty_sim::mat::{self, Mat4};
use duty_sim::math::*;
use duty_sim::pose;
use psp::sys::*;

use crate::gfx::{self, abgr, fmatrix, Camera, Gfx, Sub};
use crate::store::{self, Kept};

fn vtype_skin() -> VertexType {
    VertexType::WEIGHTS4 | VertexType::WEIGHT_8BIT | VertexType::COLOR_8888 | VertexType::NORMAL_8BIT | VertexType::VERTEX_32BITF | VertexType::INDEX_16BIT | VertexType::TRANSFORM_3D
}

pub struct Figures {
    bytes: Kept<u8>,
    /// The section's tables and clips (`duty_sim::pose`), over `bytes`, which stay for the run.
    posed: pose::Figures<'static>,
    /// Triangles drawn this frame.
    pub triangles: u32,
}

/// A pose as the GE takes a bone's matrix: column by column.
fn columns(p: &Pose) -> [[f32; 3]; 4] {
    core::array::from_fn(|c| [p[0][c], p[1][c], p[2][c]])
}

impl Figures {
    /// Reads the figures; `None` when the pack has none.
    pub unsafe fn load(file: &store::PackFile) -> Option<Figures> {
        let bytes: Kept<u8> = file.resident(pack::FIGS).ok()?;
        let posed = pose::Figures::parse(&*(bytes.as_slice() as *const [u8]))?;
        Some(Figures { bytes, posed, triangles: 0 })
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

    /// A model's batches, each with its bones, through `sub`: the run is closed when this returns,
    /// so what the caller set with `sceGu` before it holds for these draws and no later ones.
    unsafe fn draw_model(&mut self, sub: &mut Sub, model: u32, poses: &[Pose]) {
        let Some(m) = self.posed.model(model) else { return };
        let base = self.bytes.as_ptr();
        let mut triangles = 0;
        sub.i(GeCommand::VertexType, vtype_skin().bits() as u32);
        for b in &m.batches {
            if !sub.room() {
                break;
            }
            for slot in 0..b.count as usize {
                sub.matrix(GeCommand::BoneMatrixNumber, GeCommand::BoneMatrixData, slot as u32 * 12, &columns(&poses[(b.bones[slot] as usize).min(poses.len() - 1)]));
            }
            sub.address(GeCommand::Vaddr, base.add(b.vertices) as usize);
            sub.triangles(base.add(b.indices) as *const u16, b.index_count);
            triangles += b.index_count / 3;
        }
        sub.close();
        self.triangles += triangles;
    }

    /// The soldiers, standing and fallen. The list is open, the world drawn, the eye the origin.
    pub unsafe fn soldiers(&mut self, gfx: &mut Gfx, camera: &Camera, game: &Game) {
        if self.posed.models.is_empty() {
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
            let mut poses = [pose::IDENTITY; bone::SOLDIER];
            self.posed.soldier(s, &mut poses);
            // Each soldier in the pass of the cell he stands in, as the world round him is.
            gfx.pass_of_place(camera, gfx::near_place(&scene, camera.eye, s.pos.x, s.pos.z));
            self.light(gfx, s.pos, v3(scene.sun[0], scene.sun[1], scene.sun[2]), V3::UP);
            // The model faces +Z; a soldier's heading at yaw 0 is -Z.
            let (sn, cs) = (sin(s.yaw + PI), cos(s.yaw + PI));
            let at = s.pos - camera.eye;
            let root: Mat4 = [cs, 0.0, sn, at.x, 0.0, 1.0, 0.0, at.y, -sn, 0.0, cs, at.z, 0.0, 0.0, 0.0, 1.0];
            sceGuSetMatrix(MatrixMode::Model, &fmatrix(&root));
            // (far off, his plainer drawing when the pack has one: at 16 m he is 24 pixels tall)
            let id = self.posed.soldier_model(s, distance);
            self.draw_model(&mut gfx.sub, id, &poses);
        }
        sceGuDisable(GuState::Lighting);
        sceGuDisable(GuState::Light1);
    }

    /// The weapon in the player's hands, in a frustum and a depth range of its own. Returns where
    /// its muzzle is, counted from the eye in the eye's own axes.
    pub unsafe fn held(&mut self, gfx: &mut Gfx, view: &Mat4, game: &Game) -> Option<V3> {
        let held = self.posed.held(game)?;
        sceGuSetMatrix(MatrixMode::Projection, &fmatrix(&mat::perspective_gl(held.fov, 480.0 / 272.0, 0.02, 6.0)));
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
        self.light(gfx, game.player.body.pos, in_view, v3(view[1], view[5], view[9]));
        self.draw_model(&mut gfx.sub, held.model, &held.poses);
        sceGuDisable(GuState::Lighting);
        sceGuDisable(GuState::Light1);
        Some(held.muzzle)
    }
}

#[allow(dead_code)]
fn _records() {
    let _ = figure::BATCH_BONES;
}
