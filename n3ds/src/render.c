/* The PICA200 renderer.
 *
 * A frame, in one pass over a 24-bit depth buffer: the sky's band of pictures
 * and its zenith with no depth; the street as the core chose it, one picture
 * after another and the glass last; the soldiers; the effects; the weapon in
 * the player's hands in a frustum and a slice of the depth range of its own.
 *
 * The PICA200 has vertex programs and a fixed chain of combiners after them:
 *
 * - The street: texel x the vertex's colour (the surface under the baked
 *   light), doubled. Ground is multiplied once more by the shadow picture,
 *   doubled, read at the vertex's own place through the second texture unit.
 *   The PSP draws the ground twice for that.
 * - Haze: the vertex program writes what the haze leaves into the colour's
 *   alpha, and the last combiner mixes the haze's colour in by it.
 * - A soldier's and the weapon's vertices are moved by four bones and lit in
 *   their vertex program.
 *
 * The rasterizer cuts a triangle at every side of the view, so nothing of the
 * PSP's guard band is here. */
#include "render.h"

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "flat_shbin.h"
#include "skin_shbin.h"
#include "world_shbin.h"

/* Metres. The same near plane as the core's choice of what is in view. */
#define NEAR_PLANE 0.12f
#define FAR_PLANE 420.0f
/* The depth range's share of the held weapon. */
#define HELD_DEPTH 0.08f

typedef struct {
  DVLB_s *dvlb;
  shaderProgram_s program;
  C3D_AttrInfo attr;
  int mvp;
} Program;

static Program world_prog, skin_prog, flat_prog;
static int world_uvs, world_shadow, world_fog;
static int skin_bones, skin_sun, skin_up, skin_suncol, skin_skycol, skin_ambient, skin_fog;
static int flat_uvs;
static const RenderData *d;
static C3D_Tex *textures;
static DtFxVertex *sky_vb, *fx_vb;
static uint16_t *sky_ib, *quad_ib;
static uint32_t sky_fan, sky_band;
RenderStats render_stats;
/* Microseconds of the last frame: the sky, the street, the soldiers, the effects and the held weapon. */
uint32_t render_parts[4];

static uint32_t lap(u64 *from) {
  u64 now = svcGetSystemTick();
  uint32_t us = (uint32_t)((now - *from) * 1000000ULL / SYSCLOCK_ARM11);
  *from = now;
  return us;
}

static bool program_init(Program *p, const u8 *shbin, u32 size) {
  p->dvlb = DVLB_ParseFile((u32 *)shbin, size);
  if (!p->dvlb)
    return false;
  shaderProgramInit(&p->program);
  shaderProgramSetVsh(&p->program, &p->dvlb->DVLE[0]);
  p->mvp = shaderInstanceGetUniformLocation(p->program.vertexShader, "mvp");
  AttrInfo_Init(&p->attr);
  return p->mvp >= 0;
}

static int uniform(Program *p, const char *name) { return shaderInstanceGetUniformLocation(p->program.vertexShader, name); }

static void use(Program *p) {
  C3D_BindProgram(&p->program);
  C3D_SetAttrInfo(&p->attr);
}

static void buffer(const void *data, unsigned stride, int count, u64 permutation) {
  C3D_BufInfo *b = C3D_GetBufInfo();
  BufInfo_Init(b);
  BufInfo_Add(b, data, stride, count, permutation);
}

static u32 rgb(const float c[3], float scale) {
  u32 v = 0;
  for (int i = 0; i < 3; i++) {
    float x = c[i] * scale;
    x = x < 0 ? 0 : x > 1 ? 1 : x;
    v |= (u32)(x * 255.0f + 0.5f) << (8 * i);
  }
  return v | 0xff000000;
}

/* A picture of the pack: its levels are stored largest first, each on a 16-byte boundary, in the tiles the
 * PICA200 reads. */
static bool texture(C3D_Tex *t, const DtTexture *r) {
  GPU_TEXCOLOR format = r->format == DT_RGB565 ? GPU_RGB565 : r->format == DT_RGBA5551 ? GPU_RGBA5551 : GPU_A8;
  unsigned texel = r->format == DT_A8 ? 1 : 2;
  C3D_TexInitParams params = {.width = r->width, .height = r->height, .maxLevel = r->levels - 1, .format = format, .type = GPU_TEX_2D, .onVram = false};
  if (!C3D_TexInitWithParams(t, NULL, params))
    return false;
  const uint8_t *src = d->texels + r->offset;
  for (unsigned l = 0; l < r->levels; l++) {
    C3D_TexLoadImage(t, src, GPU_TEXFACE_2D, l);
    src += (((r->width >> l) * (r->height >> l) * texel) + 15) & ~15u;
  }
  C3D_TexSetFilter(t, GPU_LINEAR, GPU_LINEAR);
  C3D_TexSetFilterMipmap(t, GPU_NEAREST);
  GPU_TEXTURE_WRAP_PARAM wrap = r->flags & DT_TEX_CLAMP ? GPU_CLAMP_TO_EDGE : GPU_REPEAT;
  C3D_TexSetWrap(t, wrap, wrap);
  return true;
}

/* A cell's matrix and its shadow rows, worked out once a frame for the cells it draws. */
typedef struct {
  u32 frame;
  C3D_Mtx mvp;
  C3D_FVec shadow[2];
} CellPlace;
static CellPlace *places;
static u32 place_frame;

/* The pictures of the map that was held: another map brings its own. */
void render_drop(void) {
  for (unsigned i = 0; textures && d && i < d->texture_count; i++)
    C3D_TexDelete(&textures[i]);
  free(textures);
  textures = NULL;
  /* (a matrix a cell: the next map has its own count of cells) */
  free(places);
  places = NULL;
}

/* The vertex programs and the buffers of the sky and the effects are made once; a map's pictures and its sky
 * each time a map is read. */
static bool programs_ready;

static bool programs_init(char *error, size_t n) {
  if (programs_ready)
    return true;
  if (!program_init(&world_prog, world_shbin, world_shbin_size) || !program_init(&skin_prog, skin_shbin, skin_shbin_size) || !program_init(&flat_prog, flat_shbin, flat_shbin_size)) {
    snprintf(error, n, "a vertex program did not load");
    return false;
  }
  world_uvs = uniform(&world_prog, "uvs");
  world_shadow = uniform(&world_prog, "shadow");
  world_fog = uniform(&world_prog, "fog");
  skin_bones = uniform(&skin_prog, "bones");
  skin_sun = uniform(&skin_prog, "sun");
  skin_up = uniform(&skin_prog, "up");
  skin_suncol = uniform(&skin_prog, "suncol");
  skin_skycol = uniform(&skin_prog, "skycol");
  skin_ambient = uniform(&skin_prog, "ambient");
  skin_fog = uniform(&skin_prog, "fog");
  flat_uvs = uniform(&flat_prog, "uvs");
  if (world_uvs < 0 || world_shadow < 0 || world_fog < 0 || skin_bones < 0 || skin_sun < 0 || skin_up < 0 || skin_suncol < 0 || skin_skycol < 0 || skin_ambient < 0 || skin_fog < 0 || flat_uvs < 0) {
    snprintf(error, n, "a vertex program lacks a uniform");
    return false;
  }
  /* The street's twelve bytes: texture coordinates, the colour's two bytes, the place. */
  AttrInfo_AddLoader(&world_prog.attr, 0, GPU_SHORT, 2);
  AttrInfo_AddLoader(&world_prog.attr, 1, GPU_UNSIGNED_BYTE, 2);
  AttrInfo_AddLoader(&world_prog.attr, 2, GPU_SHORT, 3);
  /* A figure's twenty-four: weights, colour, normal (and a byte of padding), place. */
  AttrInfo_AddLoader(&skin_prog.attr, 0, GPU_UNSIGNED_BYTE, 4);
  AttrInfo_AddLoader(&skin_prog.attr, 1, GPU_UNSIGNED_BYTE, 4);
  AttrInfo_AddLoader(&skin_prog.attr, 2, GPU_BYTE, 4);
  AttrInfo_AddLoader(&skin_prog.attr, 3, GPU_FLOAT, 3);
  AttrInfo_AddLoader(&flat_prog.attr, 0, GPU_FLOAT, 2);
  AttrInfo_AddLoader(&flat_prog.attr, 1, GPU_UNSIGNED_BYTE, 4);
  AttrInfo_AddLoader(&flat_prog.attr, 2, GPU_FLOAT, 3);
  programs_ready = true;
  return true;
}

bool render_init(const RenderData *data, char *error, size_t n) {
  d = data;
  if (!programs_init(error, n))
    return false;
  textures = calloc(d->texture_count, sizeof *textures);
  for (unsigned i = 0; i < d->texture_count; i++) {
    if (d->textures[i].format == DT_T8) {
      snprintf(error, n, "the pack's pictures are a PSP's: cook the n3ds30 profile");
      return false;
    }
    if (!textures || !texture(&textures[i], &d->textures[i])) {
      snprintf(error, n, "no memory for picture %u", i);
      return false;
    }
  }
  if (!sky_vb) {
    sky_vb = linearAlloc(1024 * sizeof *sky_vb);
    sky_ib = linearAlloc(4096 * 2);
    fx_vb = linearAlloc((DT_FX_VERTICES + 4) * sizeof *fx_vb);
    quad_ib = linearAlloc((DT_FX_VERTICES / 4 + 1) * 6 * 2);
  }
  if (!sky_vb || !sky_ib || !fx_vb || !quad_ib) {
    snprintf(error, n, "no memory for the sky and the effects");
    return false;
  }
  u32 vertices = dt_sky(sky_vb, sky_ib, &sky_fan);
  sky_band = d->scene->sky_columns * 8 * 6 * 6;
  GSPGPU_FlushDataCache(sky_vb, vertices * sizeof *sky_vb);
  GSPGPU_FlushDataCache(sky_ib, (sky_band + sky_fan) * 2);
  for (unsigned q = 0; q < DT_FX_VERTICES / 4 + 1; q++) {
    const uint16_t corners[6] = {0, 1, 2, 0, 2, 3};
    for (int k = 0; k < 6; k++)
      quad_ib[q * 6 + k] = q * 4 + corners[k];
  }
  GSPGPU_FlushDataCache(quad_ib, (DT_FX_VERTICES / 4 + 1) * 6 * 2);
  return true;
}

static void stage_init(int i) { C3D_TexEnvInit(C3D_GetTexEnv(i)); }

/* The colour: `f` of `a`, `b`. The alpha: `alpha`, as it is. */
static void stage(int i, GPU_COMBINEFUNC f, GPU_TEVSRC a, GPU_TEVSRC b, bool doubled, GPU_TEVSRC alpha) {
  C3D_TexEnv *e = C3D_GetTexEnv(i);
  C3D_TexEnvInit(e);
  C3D_TexEnvSrc(e, C3D_RGB, a, b, 0);
  C3D_TexEnvFunc(e, C3D_RGB, f);
  if (doubled)
    C3D_TexEnvScale(e, C3D_RGB, GPU_TEVSCALE_2);
  C3D_TexEnvSrc(e, C3D_Alpha, alpha, 0, 0);
  C3D_TexEnvFunc(e, C3D_Alpha, GPU_REPLACE);
}

/* The haze: what came before, mixed with the haze's colour by the vertex's alpha. */
static void haze_stage(int i, u32 colour, bool on) {
  C3D_TexEnv *e = C3D_GetTexEnv(i);
  C3D_TexEnvInit(e);
  if (!on) {
    C3D_TexEnvSrc(e, C3D_Both, GPU_PREVIOUS, 0, 0);
    C3D_TexEnvFunc(e, C3D_Both, GPU_REPLACE);
    return;
  }
  C3D_TexEnvSrc(e, C3D_RGB, GPU_PREVIOUS, GPU_CONSTANT, GPU_PRIMARY_COLOR);
  C3D_TexEnvOpRgb(e, GPU_TEVOP_RGB_SRC_COLOR, GPU_TEVOP_RGB_SRC_COLOR, GPU_TEVOP_RGB_SRC_ALPHA);
  C3D_TexEnvFunc(e, C3D_RGB, GPU_INTERPOLATE);
  C3D_TexEnvColor(e, colour);
  C3D_TexEnvSrc(e, C3D_Alpha, GPU_PREVIOUS, 0, 0);
  C3D_TexEnvFunc(e, C3D_Alpha, GPU_REPLACE);
}

static C3D_Mtx rows(const float m[16]) {
  C3D_Mtx out;
  for (int r = 0; r < 4; r++)
    out.r[r] = FVec4_New(m[r * 4], m[r * 4 + 1], m[r * 4 + 2], m[r * 4 + 3]);
  return out;
}

static void cell_centre(const DtScene *s, unsigned cell, float out[3]) {
  out[0] = s->min[0] + ((float)(cell % s->cells_x) + 0.5f) * s->cell;
  out[1] = s->origin_y;
  out[2] = s->min[2] + ((float)(cell / s->cells_x) + 0.5f) * s->cell;
}

static void fog_uniform(int at, const DtView *view) {
  float span = view->haze_to - view->haze_from;
  bool on = span > 0.0f && !(view->option & 8);
  C3D_FVUnifSet(GPU_VERTEX_SHADER, at, on ? 1.0f / span : 0.0f, on ? -view->haze_from / span : 0.0f, 0.0f, 0.0f);
}

/* The street's combiners: texel x colour, doubled; the ground's shadow, doubled; the haze. */
static void street_stages(bool ground, bool textured, u32 haze, bool hazy) {
  int at = 0;
  if (textured)
    stage(at++, GPU_MODULATE, GPU_TEXTURE0, GPU_PRIMARY_COLOR, true, GPU_TEXTURE0);
  else
    stage(at++, GPU_REPLACE, GPU_PRIMARY_COLOR, 0, true, GPU_CONSTANT);
  if (ground)
    stage(at++, GPU_MODULATE, GPU_PREVIOUS, GPU_TEXTURE1, true, GPU_PREVIOUS);
  haze_stage(at++, haze, hazy);
  for (; at < 4; at++)
    stage_init(at);
}

/* citro3d's own: writes what changed of the state to the command buffer. Every draw of its calls it, and it
 * looks at each of the 96 uniform registers to find what changed: 16 microseconds a draw on an Old 3DS, which
 * for the street's six hundred draws is a third of a frame. The street calls it when it changed a picture or
 * a combiner, and writes its uniforms, the vertex buffer's offset and the draw itself as the commands they are. */
void C3Di_UpdateContext(void);

/* `count` registers of a vertex program's uniforms from `at`, as citro3d holds a vector (w first). */
static inline void raw_uniform(int at, const C3D_FVec *values, int count) {
  GPUCMD_AddWrite(GPUREG_VSH_FLOATUNIFORM_CONFIG, 0x80000000 | at);
  GPUCMD_AddWrites(GPUREG_VSH_FLOATUNIFORM_DATA, (const u32 *)values, count * 4);
}

/* What C3D_DrawElements writes for triangles with 16-bit indices, without the look at the state. */
static inline void raw_triangles(const uint16_t *indices, u32 count) {
  GPUCMD_AddMaskedWrite(GPUREG_PRIMITIVE_CONFIG, 2, GPU_GEOMETRY_PRIM);
  GPUCMD_AddWrite(GPUREG_RESTART_PRIMITIVE, 1);
  GPUCMD_AddWrite(GPUREG_INDEXBUFFER_CONFIG, (osConvertVirtToPhys(indices) - 0x18000000) | (1u << 31));
  GPUCMD_AddWrite(GPUREG_NUMVERTICES, count);
  GPUCMD_AddWrite(GPUREG_VERTEX_OFFSET, 0);
  GPUCMD_AddMaskedWrite(GPUREG_GEOSTAGE_CONFIG, 2, 0x100);
  GPUCMD_AddMaskedWrite(GPUREG_GEOSTAGE_CONFIG2, 2, 0x100);
  GPUCMD_AddMaskedWrite(GPUREG_START_DRAW_FUNC0, 1, 0);
  GPUCMD_AddWrite(GPUREG_DRAWELEMENTS, 1);
  GPUCMD_AddMaskedWrite(GPUREG_START_DRAW_FUNC0, 1, 1);
  GPUCMD_AddMaskedWrite(GPUREG_GEOSTAGE_CONFIG, 2, 0);
  GPUCMD_AddMaskedWrite(GPUREG_GEOSTAGE_CONFIG2, 2, 0);
  GPUCMD_AddWrite(GPUREG_VTX_FUNC, 1);
  GPUCMD_AddMaskedWrite(GPUREG_PRIMITIVE_CONFIG, 0x8, 0);
  GPUCMD_AddMaskedWrite(GPUREG_PRIMITIVE_CONFIG, 0x8, 0);
}


static void street(const C3D_Mtx *pv, const DtView *view, const DtDraw *draws, uint32_t count, u32 haze) {
  const DtScene *s = d->scene;
  bool hazy = !(view->option & 8) && view->haze_to > view->haze_from;
  use(&world_prog);
  fog_uniform(world_fog, view);
  C3D_TexBind(1, &textures[s->shadow_texture]);
  u32 bound = UINT32_MAX, cell = UINT32_MAX, vertex = UINT32_MAX, flags = UINT32_MAX;
  float uv[4] = {0, 0, 0, 0};
  bool blending = false, changed = true;
  if (!places)
    places = calloc(s->cells_x * s->cells_z, sizeof *places);
  place_frame++;
  /* One buffer for the whole street. A level's vertices start somewhere in it, and its indices count from
   * there: each draw moves the buffer's offset, one register, where citro3d would write the whole layout. */
  buffer(d->vertices, 12, 3, 0x210);
  u32 base = osConvertVirtToPhys(d->vertices) - 0x18000000;
  for (uint32_t i = 0; i < count; i++) {
    const DtDraw *w = &draws[i];
    if (w->texture != bound) {
      bound = w->texture;
      C3D_TexBind(0, &textures[bound]);
      changed = true;
    }
    u32 state = w->flags & (DT_GROUND | DT_TWO_SIDED | DT_BLEND | DT_CUTOUT);
    if (state != flags) {
      flags = state;
      changed = true;
      C3D_CullFace(state & DT_TWO_SIDED || view->option & 1 ? GPU_CULL_NONE : GPU_CULL_BACK_CCW);
      C3D_AlphaTest(state & DT_CUTOUT, GPU_GREATER, 128);
      street_stages(state & DT_GROUND && !(view->option & 16), !(view->option & 32), haze, hazy);
      if ((state & DT_BLEND) && !blending) {
        /* Glass: its colour by its opacity over what is behind it, and it hides nothing. */
        blending = true;
        C3D_AlphaBlend(GPU_BLEND_ADD, GPU_BLEND_ADD, GPU_CONSTANT_ALPHA, GPU_ONE_MINUS_CONSTANT_ALPHA, GPU_ONE, GPU_ZERO);
        C3D_DepthTest(true, GPU_GREATER, GPU_WRITE_COLOR);
      }
    }
    if (blending) {
      C3D_BlendingColor((u32)w->opacity << 24);
      changed = true;
    }
    if (changed) {
      C3Di_UpdateContext();
      changed = false;
    }
    if (w->cell != cell) {
      cell = w->cell;
      CellPlace *place = &places[cell];
      if (place->frame != place_frame) {
        place->frame = place_frame;
        float c[3];
        cell_centre(s, cell, c);
        /* clip = projection x view x T(centre - eye) x S(a step) */
        place->mvp = *pv;
        Mtx_Translate(&place->mvp, c[0] - view->eye[0], c[1] - view->eye[1], c[2] - view->eye[2], true);
        Mtx_Scale(&place->mvp, s->quantum, s->quantum, s->quantum);
        /* The shadow picture at the vertex's place. The picture's first row is the texture's last. */
        const float (*r)[3] = s->shadow_rows;
        place->shadow[0] = FVec4_New(r[0][0] * s->quantum, 0.0f, r[0][1] * s->quantum, r[0][0] * c[0] + r[0][1] * c[2] + r[0][2]);
        place->shadow[1] = FVec4_New(-r[1][0] * s->quantum, 0.0f, -r[1][1] * s->quantum, 1.0f - (r[1][0] * c[0] + r[1][1] * c[2] + r[1][2]));
      }
      raw_uniform(world_prog.mvp, place->mvp.r, 4);
      raw_uniform(world_shadow, place->shadow, 2);
    }
    if (memcmp(uv, w->uv, sizeof uv)) {
      memcpy(uv, w->uv, sizeof uv);
      C3D_FVec scale = FVec4_New(uv[0], uv[1], -uv[2], 1.0f - uv[3]);
      raw_uniform(world_uvs, &scale, 1);
    }
    if (w->vertex != vertex) {
      vertex = w->vertex;
      GPUCMD_AddWrite(GPUREG_ATTRIBBUFFER0_OFFSET, base + vertex * 12);
    }
    raw_triangles(d->indices + w->index, w->count);
    render_stats.draws++;
    render_stats.triangles += w->count / 3;
  }
  C3D_AlphaBlend(GPU_BLEND_ADD, GPU_BLEND_ADD, GPU_ONE, GPU_ZERO, GPU_ONE, GPU_ZERO);
  C3D_AlphaTest(false, GPU_ALWAYS, 0);
  C3D_DepthTest(true, GPU_GREATER, GPU_WRITE_ALL);
  /* (the buffer's offset is no longer what citro3d wrote: whoever draws next sets its own buffer) */
}

static void figure(const C3D_Mtx *projection, const DtView *view, const DtFigure *f) {
  C3D_Mtx place, m;
  Mtx_Identity(&place);
  for (int r = 0; r < 3; r++)
    place.r[r] = FVec4_New(f->place[r * 4], f->place[r * 4 + 1], f->place[r * 4 + 2], f->place[r * 4 + 3]);
  Mtx_Multiply(&m, projection, &place);
  C3D_FVUnifMtx4x4(GPU_VERTEX_SHADER, skin_prog.mvp, &m);
  C3D_FVUnifSet(GPU_VERTEX_SHADER, skin_sun, f->sun[0], f->sun[1], f->sun[2], 0.0f);
  C3D_FVUnifSet(GPU_VERTEX_SHADER, skin_up, f->up[0], f->up[1], f->up[2], 0.0f);
  C3D_FVUnifSet(GPU_VERTEX_SHADER, skin_suncol, view->sun_colour[0] * f->lit, view->sun_colour[1] * f->lit, view->sun_colour[2] * f->lit, 0.0f);
  for (uint32_t b = 0; b < f->batches; b++) {
    const DtBatch *batch = &f->batch[b];
    C3D_FVec *bones = C3D_FVUnifWritePtr(GPU_VERTEX_SHADER, skin_bones, 12);
    for (int k = 0; k < 4; k++)
      for (int r = 0; r < 3; r++)
        bones[k * 3 + r] = FVec4_New(batch->bones[k][r * 4], batch->bones[k][r * 4 + 1], batch->bones[k][r * 4 + 2], batch->bones[k][r * 4 + 3]);
    buffer(batch->vertices, 24, 4, 0x3210);
    C3D_DrawElements(GPU_TRIANGLES, batch->count, C3D_UNSIGNED_SHORT, batch->indices);
    render_stats.draws++;
    render_stats.triangles += batch->count / 3;
  }
}

void render_frame(C3D_RenderTarget *target, const DtView *view, const DtDraw *draws, uint32_t draw_count, const DtFigure *figures, uint32_t figure_count) {
  const DtScene *s = d->scene;
  memset(&render_stats, 0, sizeof render_stats);
  u64 timer = svcGetSystemTick();
  u32 haze = rgb(view->haze, 1.0f);
  C3D_RenderTargetClear(target, C3D_CLEAR_ALL, ((haze & 0xff) << 24) | ((haze & 0xff00) << 8) | ((haze & 0xff0000) >> 8) | 0xff, 0);
  C3D_FrameDrawOn(target);
  C3D_Mtx proj, look = rows(view->view), pv;
  Mtx_PerspTilt(&proj, C3D_AngleFromDegrees(view->fov), C3D_AspectRatioTop, NEAR_PLANE, FAR_PLANE, false);
  Mtx_Multiply(&pv, &proj, &look);
  C3D_DepthMap(true, -(1.0f - HELD_DEPTH), 0.0f);
  C3D_AlphaTest(false, GPU_ALWAYS, 0);
  C3D_AlphaBlend(GPU_BLEND_ADD, GPU_BLEND_ADD, GPU_ONE, GPU_ZERO, GPU_ONE, GPU_ZERO);
  C3D_FogGasMode(GPU_NO_FOG, GPU_PLAIN_DENSITY, false);

  /* ---- the sky: a band of pictures round the horizon, then the zenith's colour */
  use(&flat_prog);
  C3D_FVUnifMtx4x4(GPU_VERTEX_SHADER, flat_prog.mvp, &pv);
  C3D_FVUnifSet(GPU_VERTEX_SHADER, flat_uvs, 1.0f, 0.0f, -1.0f, 1.0f);
  buffer(sky_vb, sizeof *sky_vb, 3, 0x210);
  C3D_CullFace(GPU_CULL_NONE);
  C3D_DepthTest(false, GPU_ALWAYS, GPU_WRITE_COLOR);
  /* (a texture unit is never unbound: citro3d reads the texture it is handed) */
  C3D_TexBind(1, &textures[s->shadow_texture]);
  C3D_TexBind(2, &textures[s->shadow_texture]);
  if (!(view->option & 128) && s->sky_columns) {
    stage(0, GPU_REPLACE, GPU_TEXTURE0, 0, false, GPU_CONSTANT);
    for (int i = 1; i < 4; i++)
      stage_init(i);
    unsigned per_column = 8 * 6 * 6;
    for (unsigned c = 0; c < s->sky_columns; c++) {
      C3D_TexBind(0, &textures[s->sky_texture + c]);
      C3D_DrawElements(GPU_TRIANGLES, per_column, C3D_UNSIGNED_SHORT, sky_ib + c * per_column);
    }
    stage(0, GPU_REPLACE, GPU_PRIMARY_COLOR, 0, false, GPU_CONSTANT);
    C3D_DrawElements(GPU_TRIANGLES, sky_fan, C3D_UNSIGNED_SHORT, sky_ib + sky_band);
    render_stats.draws += s->sky_columns + 1;
  }

  render_parts[0] = lap(&timer);

  /* ---- the street */
  C3D_DepthTest(true, GPU_GREATER, GPU_WRITE_ALL);
  street(&pv, view, draws, draw_count, haze);
  render_parts[1] = lap(&timer);

  /* ---- the soldiers */
  use(&skin_prog);
  fog_uniform(skin_fog, view);
  C3D_FVUnifSet(GPU_VERTEX_SHADER, skin_skycol, view->sky_colour[0] * 0.6f, view->sky_colour[1] * 0.6f, view->sky_colour[2] * 0.6f, 0.0f);
  C3D_FVUnifSet(GPU_VERTEX_SHADER, skin_ambient, view->sky_colour[0] * 0.55f, view->sky_colour[1] * 0.55f, view->sky_colour[2] * 0.55f, 0.0f);
  C3D_CullFace(view->option & 1 ? GPU_CULL_NONE : GPU_CULL_BACK_CCW);
  stage(0, GPU_REPLACE, GPU_PRIMARY_COLOR, 0, false, GPU_CONSTANT);
  haze_stage(1, haze, !(view->option & 8) && view->haze_to > view->haze_from);
  stage_init(2);
  stage_init(3);
  for (uint32_t i = 0; i < figure_count; i++)
    if (!figures[i].held)
      figure(&proj, view, &figures[i]);

  render_parts[2] = lap(&timer);

  /* ---- the effects: squares facing the eye, the ones that cover first, then the ones that glow */
  uint32_t covering = 0, glowing = 0;
  dt_effects(fx_vb, &covering, &glowing);
  const DtFigure *held = figure_count && figures[figure_count - 1].held ? &figures[figure_count - 1] : NULL;
  bool flash = held && view->flash > 0.0f;
  if (flash) {
    /* The muzzle's flash, a square in the weapon's own view, after the effects' vertices. */
    static const float corner[4][2] = {{-1, 1}, {1, 1}, {1, -1}, {-1, -1}};
    for (int k = 0; k < 4; k++)
      fx_vb[DT_FX_VERTICES + k] = (DtFxVertex){.uv = {0.02f + (corner[k][0] * 0.5f + 0.5f) * 0.46f, 0.52f + (0.5f - corner[k][1] * 0.5f) * 0.46f},
                                               .colour = {255, 220, 160, 240},
                                               .pos = {view->flash_at[0] + corner[k][0] * view->flash, view->flash_at[1] + corner[k][1] * view->flash, view->flash_at[2]}};
  }
  GSPGPU_FlushDataCache(fx_vb, (DT_FX_VERTICES + 4) * sizeof *fx_vb);
  use(&flat_prog);
  C3D_FVUnifSet(GPU_VERTEX_SHADER, flat_uvs, 1.0f, 0.0f, -1.0f, 1.0f);
  C3D_CullFace(GPU_CULL_NONE);
  C3D_TexBind(0, &textures[s->fx_texture]);
  {
    /* The colour is the vertex's; how opaque, the picture's times the vertex's. */
    C3D_TexEnv *e = C3D_GetTexEnv(0);
    C3D_TexEnvInit(e);
    C3D_TexEnvSrc(e, C3D_RGB, GPU_PRIMARY_COLOR, 0, 0);
    C3D_TexEnvFunc(e, C3D_RGB, GPU_REPLACE);
    C3D_TexEnvSrc(e, C3D_Alpha, GPU_TEXTURE0, GPU_PRIMARY_COLOR, 0);
    C3D_TexEnvFunc(e, C3D_Alpha, GPU_MODULATE);
    stage_init(1);
  }
  C3D_DepthTest(true, GPU_GREATER, GPU_WRITE_COLOR);
  if (covering + glowing) {
    buffer(fx_vb, sizeof *fx_vb, 3, 0x210);
    if (covering) {
      C3D_AlphaBlend(GPU_BLEND_ADD, GPU_BLEND_ADD, GPU_SRC_ALPHA, GPU_ONE_MINUS_SRC_ALPHA, GPU_ONE, GPU_ZERO);
      C3D_DrawElements(GPU_TRIANGLES, covering / 4 * 6, C3D_UNSIGNED_SHORT, quad_ib);
    }
    if (glowing) {
      C3D_AlphaBlend(GPU_BLEND_ADD, GPU_BLEND_ADD, GPU_SRC_ALPHA, GPU_ONE, GPU_ONE, GPU_ZERO);
      C3D_DrawElements(GPU_TRIANGLES, glowing / 4 * 6, C3D_UNSIGNED_SHORT, quad_ib + covering / 4 * 6);
    }
    render_stats.draws += 2;
    render_stats.triangles += (covering + glowing) / 2;
  }
  C3D_AlphaBlend(GPU_BLEND_ADD, GPU_BLEND_ADD, GPU_ONE, GPU_ZERO, GPU_ONE, GPU_ZERO);

  /* ---- the weapon in the player's hands, in a frustum and a slice of the depth range of its own */
  if (held) {
    C3D_Mtx own;
    Mtx_PerspTilt(&own, C3D_AngleFromDegrees(held->fov), C3D_AspectRatioTop, 0.02f, 6.0f, false);
    C3D_DepthMap(true, -HELD_DEPTH, 1.0f - HELD_DEPTH);
    C3D_DepthTest(true, GPU_GREATER, GPU_WRITE_ALL);
    use(&skin_prog);
    C3D_FVUnifSet(GPU_VERTEX_SHADER, skin_fog, 0.0f, 0.0f, 0.0f, 0.0f);
    C3D_CullFace(view->option & 1 ? GPU_CULL_NONE : GPU_CULL_BACK_CCW);
    stage(0, GPU_REPLACE, GPU_PRIMARY_COLOR, 0, false, GPU_CONSTANT);
    stage_init(1);
    figure(&own, view, held);
    if (flash) {
      use(&flat_prog);
      C3D_FVUnifMtx4x4(GPU_VERTEX_SHADER, flat_prog.mvp, &own);
      C3D_FVUnifSet(GPU_VERTEX_SHADER, flat_uvs, 1.0f, 0.0f, -1.0f, 1.0f);
      C3D_CullFace(GPU_CULL_NONE);
      C3D_TexEnv *e = C3D_GetTexEnv(0);
      C3D_TexEnvInit(e);
      C3D_TexEnvSrc(e, C3D_RGB, GPU_PRIMARY_COLOR, 0, 0);
      C3D_TexEnvFunc(e, C3D_RGB, GPU_REPLACE);
      C3D_TexEnvSrc(e, C3D_Alpha, GPU_TEXTURE0, GPU_PRIMARY_COLOR, 0);
      C3D_TexEnvFunc(e, C3D_Alpha, GPU_MODULATE);
      C3D_DepthTest(false, GPU_ALWAYS, GPU_WRITE_COLOR);
      C3D_AlphaBlend(GPU_BLEND_ADD, GPU_BLEND_ADD, GPU_SRC_ALPHA, GPU_ONE, GPU_ONE, GPU_ZERO);
      buffer(fx_vb + DT_FX_VERTICES, sizeof *fx_vb, 3, 0x210);
      C3D_DrawElements(GPU_TRIANGLES, 6, C3D_UNSIGNED_SHORT, quad_ib);
      C3D_AlphaBlend(GPU_BLEND_ADD, GPU_BLEND_ADD, GPU_ONE, GPU_ZERO, GPU_ONE, GPU_ZERO);
      render_stats.draws++;
    }
    C3D_DepthMap(true, -(1.0f - HELD_DEPTH), 0.0f);
  }
  render_parts[3] = lap(&timer);
  /* What the interface draws next assumes these. */
  C3D_DepthTest(false, GPU_ALWAYS, GPU_WRITE_COLOR);
  C3D_CullFace(GPU_CULL_NONE);
}
