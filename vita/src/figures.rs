//! The soldiers and the held weapon: the pack's figures, posed by the code
//! every device shares (`duty_sim::pose`) and skinned by the vertex program
//! (`shaders/skin_v.cg`), four bones a draw.
//!
//! A soldier in the sun has his shadow on the ground under him: his own
//! triangles once more, each vertex sent down the sun's ray to the height of
//! his feet. The stencil lets each pixel of the ground darken once, whatever
//! number of his triangles lie over it.

use duty_pack as pack;
use duty_pack::figure::{bone, part, Pose};
use duty_sim::game::Game;
use duty_sim::mat::{self, Mat4};
use duty_sim::math::*;
use duty_sim::pose;
use pocket_vita_gxm::mem::{Block, Kind};
use pocket_vita_gxm::program::{F32, S8N, U8, U8N};
use vita2d_sys as g;

use crate::gpu::{self, Blend, Gpu, Param, Program, Stream, Uniforms};
use crate::store::PackFile;
use crate::world::{Drawn, Gfx, Look, HEIGHT, WIDTH};

/// The stencil bit a shadow marks the pixels it has darkened with. vita2d's clips write the low bits.
const SHADE_BIT: u8 = 0x80;
/// How much of the ground's light a soldier's shadow takes where the sun is whole.
const SHADE: f32 = 0.42;

struct Skin {
    prog: Program,
    clip: Param,
    bones: Param,
    light: Param,
    ground: Param,
}

/// The figures' programs: made once, for every map.
pub struct Skins {
    skin: Skin,
    shade: Skin,
}

pub struct Figures {
    /// The pack's figures in memory the GPU reads: the vertex streams and the indices point into it.
    block: Block,
    base: *const u8,
    posed: pose::Figures<'static>,
}

impl Skins {
    /// # Safety
    /// Render thread, outside any scene.
    pub unsafe fn new(gpu: &mut Gpu, msaa: u32) -> Result<Skins, String> {
        const SKIN_V: &str = include_str!("../shaders/skin_v.cg");
        const SKIN_F: &str = include_str!("../shaders/skin_f.cg");
        let attrs = [("aWeights", 0u16, U8, 4u8), ("aColor", 4, U8N, 4), ("aNormal", 8, S8N, 3), ("aPosition", 12, F32, 3)];
        let mut skin = |name: &str, defines: &str, blends: [Blend; 2]| -> Result<Skin, String> {
            let prog = gpu.program(name, defines, "", SKIN_V, SKIN_F, &[Stream { stride: 24, instanced: false, attrs: &attrs }], msaa, blends)?;
            Ok(Skin { clip: prog.vs.param("uClip"), bones: prog.vs.param("uBones"), light: prog.vs.param("uLight"), ground: prog.vs.param("uGround"), prog })
        };
        Ok(Skins { skin: skin("skin", "", [Blend::Opaque, Blend::Opaque])?, shade: skin("shade", "#define SHADE\n", [Blend::Alpha, Blend::Alpha])? })
    }
}

/// The four lights of a figure as the vertex program takes them.
fn lights(sun: V3, sun_colour: [f32; 3], up: V3, sky: [f32; 3]) -> [f32; 16] {
    [sun.x, sun.y, sun.z, 0.0, sun_colour[0], sun_colour[1], sun_colour[2], 0.0, up.x, up.y, up.z, 0.0, sky[0], sky[1], sky[2], 0.0]
}

impl Figures {
    /// # Safety
    /// Render thread, outside any scene.
    pub unsafe fn load(file: &mut PackFile) -> Result<Option<Figures>, String> {
        if !file.has(pack::FIGS) {
            return Ok(None);
        }
        let size = file.size(pack::FIGS)?;
        let mut block = Block::with_access(Kind::Main, size.max(4096), false)?;
        let base = block.alloc(size, 64).ok_or("the figures' block")?;
        let bytes = core::slice::from_raw_parts_mut(base, size);
        file.read_into(pack::FIGS, 0, bytes)?;
        // (the block lives as long as this does, and nothing writes it again)
        let Some(posed) = pose::Figures::parse(&*(bytes as *const [u8])) else {
            block.free();
            return Ok(None);
        };
        Ok(Some(Figures { block, base, posed }))
    }

    /// # Safety
    /// The GPU has drawn the last frame that read it.
    pub unsafe fn free(self) {
        self.block.free();
    }

    /// One model under one program: a draw for each set of four bones.
    unsafe fn model(&self, ctx: *mut g::SceGxmContext, s: &Skin, model: u32, poses: &[Pose], root: &Mat4, clip: &Mat4, light: &[f32; 16], ground: Option<[f32; 4]>, drawn: &mut Drawn) {
        let Some(m) = self.posed.model(model) else { return };
        for b in &m.batches {
            // A bone's matrix takes a bind-pose position to the figure's own space; `root` stands the figure in
            // the world, counted from the eye.
            let mut bones = [0.0f32; 48];
            for slot in 0..(b.count as usize).min(4) {
                let p = &poses[(b.bones[slot] as usize).min(poses.len() - 1)];
                for r in 0..3 {
                    for c in 0..4 {
                        bones[slot * 12 + r * 4 + c] = root[r * 4] * p[0][c] + root[r * 4 + 1] * p[1][c] + root[r * 4 + 2] * p[2][c] + if c == 3 { root[r * 4 + 3] } else { 0.0 };
                    }
                }
            }
            let v = Uniforms::vertex(ctx);
            v.set(s.clip, clip);
            v.set(s.bones, &bones);
            v.set(s.light, light);
            if let Some(ground) = ground {
                v.set(s.ground, &ground);
            }
            g::sceGxmSetVertexStream(ctx, 0, self.base.add(b.vertices).cast());
            g::sceGxmDraw(ctx, g::SceGxmPrimitiveType_SCE_GXM_PRIMITIVE_TRIANGLES, g::SceGxmIndexFormat_SCE_GXM_INDEX_FORMAT_U16, self.base.add(b.indices).cast(), b.index_count);
            drawn.draws += 1;
            drawn.triangles += b.index_count / 3;
        }
    }

    /// The soldiers in the view, their shadows first. `clip` takes a place counted from the eye to the screen
    /// (the view turned, not moved).
    ///
    /// # Safety
    /// Render thread, inside the display scene, the viewport the street's.
    pub unsafe fn soldiers(&self, ctx: *mut g::SceGxmContext, k: &Skins, gfx: &Gfx, clip: &Mat4, eye: V3, look: V3, game: &Game, set: &Look) -> Drawn {
        let mut drawn = Drawn::default();
        if self.posed.models.is_empty() {
            return drawn;
        }
        let scene = &gfx.scene;
        let sun = v3(scene.sun[0], scene.sun[1], scene.sun[2]);
        // Each soldier in the view: his poses, where he stands from the eye, his model, and the sun on him.
        let mut standing: Vec<([Pose; bone::SOLDIER], Mat4, u32, f32, f32)> = Vec::new();
        for s in game.soldiers.iter().filter(|s| s.used) {
            let to = s.pos + v3(0.0, 0.9, 0.0) - eye;
            let distance = to.len();
            // Behind the eye, or too far to be more than a pixel.
            if distance > 160.0 || to.dot(look) < -1.5 {
                continue;
            }
            let mut poses = [pose::IDENTITY; bone::SOLDIER];
            self.posed.soldier(s, &mut poses);
            // The model faces +Z; a soldier's heading at yaw 0 is -Z.
            let (sn, cs) = (sin(s.yaw + PI), cos(s.yaw + PI));
            let at = s.pos - eye;
            let root: Mat4 = [cs, 0.0, sn, at.x, 0.0, 1.0, 0.0, at.y, -sn, 0.0, cs, at.z, 0.0, 0.0, 0.0, 1.0];
            standing.push((poses, root, self.posed.soldier_model(s, distance), gfx.sunlit(s.pos.x, s.pos.z), distance));
        }
        if standing.is_empty() {
            return drawn;
        }
        let none = lights(sun, [0.0; 3], V3::UP, [0.0; 3]);
        if set.shades && sun.y > 0.05 {
            // Over the ground, under the soldiers: tested against the street's depth and not written, and a
            // pixel one of a soldier's triangles has darkened is left alone by the others.
            gpu::state_overlay(ctx, true);
            k.shade.prog.bind(ctx, false);
            for (number, (poses, root, model, lit, distance)) in standing.iter().enumerate() {
                if *lit < 0.05 || *distance > 60.0 {
                    continue;
                }
                // (each soldier's shadow marks its own pixels: two shadows that cross darken twice, as two do)
                let mark = SHADE_BIT | (number as u8 & 0x7f).max(1);
                for face in [true, false] {
                    let set_func = if face { g::sceGxmSetFrontStencilFunc } else { g::sceGxmSetBackStencilFunc };
                    set_func(ctx, g::SceGxmStencilFunc_SCE_GXM_STENCIL_FUNC_NOT_EQUAL, g::SceGxmStencilOp_SCE_GXM_STENCIL_OP_KEEP, g::SceGxmStencilOp_SCE_GXM_STENCIL_OP_KEEP, g::SceGxmStencilOp_SCE_GXM_STENCIL_OP_REPLACE, 0xff, 0xff);
                }
                g::sceGxmSetFrontStencilRef(ctx, mark as u32);
                g::sceGxmSetBackStencilRef(ctx, mark as u32);
                self.model(ctx, &k.shade, *model, poses, root, clip, &none, Some([root[7], SHADE * *lit, 0.0, 0.0]), &mut drawn);
            }
            for face in [true, false] {
                let set_func = if face { g::sceGxmSetFrontStencilFunc } else { g::sceGxmSetBackStencilFunc };
                set_func(ctx, g::SceGxmStencilFunc_SCE_GXM_STENCIL_FUNC_ALWAYS, g::SceGxmStencilOp_SCE_GXM_STENCIL_OP_KEEP, g::SceGxmStencilOp_SCE_GXM_STENCIL_OP_KEEP, g::SceGxmStencilOp_SCE_GXM_STENCIL_OP_KEEP, 0, 0);
            }
        }
        gpu::state_opaque(ctx, set.cull(false));
        k.skin.prog.bind(ctx, false);
        for (poses, root, model, lit, _) in &standing {
            let light = lights(sun, scene.sun_colour.map(|c| c * lit), V3::UP, scene.sky_colour);
            self.model(ctx, &k.skin, *model, poses, root, clip, &light, None, &mut drawn);
        }
        drawn
    }

    /// The weapon in the player's hands as this frame poses it, or none when he holds none (he is down).
    pub fn posed_held(&self, game: &Game) -> Option<pose::Held> {
        self.posed.held(game)
    }

    /// The weapon in the player's hands, in its own frustum and its own share of the depth range. Returns that
    /// frustum and where the muzzle is in it.
    ///
    /// # Safety
    /// Render thread, inside the display scene.
    pub unsafe fn held(&self, ctx: *mut g::SceGxmContext, k: &Skins, gfx: &Gfx, view: &Mat4, game: &Game, set: &Look, held: &pose::Held, drawn: &mut Drawn) -> (Mat4, V3) {
        let frustum = mat::perspective(held.fov, WIDTH / HEIGHT, 0.02, 6.0);
        let scene = &gfx.scene;
        // The sun's direction, and up, as the eye sees them.
        let sun = v3(scene.sun[0], scene.sun[1], scene.sun[2]);
        let in_view = v3(view[0] * sun.x + view[1] * sun.y + view[2] * sun.z, view[4] * sun.x + view[5] * sun.y + view[6] * sun.z, view[8] * sun.x + view[9] * sun.y + view[10] * sun.z);
        let feet = game.player.body.pos;
        let light = lights(in_view, scene.sun_colour.map(|c| c * gfx.sunlit(feet.x, feet.z)), v3(view[1], view[5], view[9]), scene.sky_colour);
        Gfx::viewport(ctx, true);
        gpu::state_opaque(ctx, set.cull(false));
        k.skin.prog.bind(ctx, false);
        let poses: &[Pose; part::WEAPON] = &held.poses;
        self.model(ctx, &k.skin, held.model, poses, &mat::IDENTITY, &frustum, &light, None, drawn);
        (frustum, held.muzzle)
    }
}
