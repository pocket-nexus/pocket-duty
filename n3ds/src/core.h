/* The C interface of the Rust core (../core/src/lib.rs) and the pack's records
 * the renderer reads (crates/duty-pack). dt_sizes() checks at start that both
 * sides agree. */
#ifndef DUTY_CORE_H
#define DUTY_CORE_H
#include <stdint.h>

/* duty_pack::chunk flags */
enum { DT_GROUND = 1, DT_TWO_SIDED = 2, DT_BLEND = 4, DT_CUTOUT = 8 };
/* duty_pack::texture formats and flags */
enum { DT_T8 = 0, DT_RGB565 = 1, DT_RGBA5551 = 2, DT_A8 = 3 };
enum { DT_TEX_CUTOUT = 1, DT_TEX_CLAMP = 2 };
/* duty_interface::pad */
enum {
  DT_UP = 1, DT_DOWN = 1 << 1, DT_LEFT = 1 << 2, DT_RIGHT = 1 << 3, DT_NORTH = 1 << 4, DT_EAST = 1 << 5, DT_SOUTH = 1 << 6, DT_WEST = 1 << 7,
  DT_L = 1 << 8, DT_R = 1 << 9, DT_START = 1 << 10, DT_SELECT = 1 << 11
};
#define DT_BATCHES 12
/* Vertices of the effects at most (duty_sim::fx::VERTICES); dt_sizes says the same. */
#define DT_FX_VERTICES 496

typedef struct {
  float min[3], cell, max[3], quantum;
  uint32_t cells_x, cells_z;
  float origin_y, lod[2], sun[3], sun_colour[3], sky_colour[3], haze[2];
  uint32_t haze_colour, shadow_texture;
  float shadow_rows[2][3];
  uint32_t sky_texture, sky_columns;
  float sky_top, sky_floor;
  uint32_t zenith;
  float start[4];
  uint32_t fx_texture;
  float level_origin[3], level_x[3], level_z[3];
} DtScene;

typedef struct {
  uint32_t offset;
  uint16_t width, height;
  uint8_t levels, format;
  uint16_t flags;
} DtTexture;

typedef struct {
  const DtScene *scene;
  const void *chunks;
  uint32_t chunk_count;
  const void *clusters;
  uint32_t cluster_count;
  const void *cells;
  uint32_t cell_count;
  const uint16_t *lists;
  uint32_t list_count;
  const uint8_t *bits;
  uint32_t bit_bytes;
  const DtTexture *textures;
  uint32_t texture_count;
  const uint8_t *texels;
  uint32_t texel_bytes;
  const void *solid_verts;
  uint32_t solid_vert_count;
  const void *solid_tris;
  uint32_t solid_tri_count;
  const void *solid_nodes;
  uint32_t solid_node_count;
  const uint8_t *nav;
  uint32_t nav_bytes;
  const void *weapons;
  uint32_t weapon_bytes;
  const uint8_t *figures;
  uint32_t figure_bytes;
  const void *sounds;
  uint32_t sound_bytes;
  const uint8_t *samples;
  uint32_t sample_bytes;
} DtPack;

typedef struct {
  uint32_t held;
  float sx, sy, rx, ry;
} DtPad;

typedef struct {
  uint32_t vertex, index, count;
  uint16_t texture, cell, flags;
  uint8_t opacity, pad;
  float uv[4];
} DtDraw;

typedef struct {
  float bones[4][12];
  const uint8_t *vertices;
  const uint16_t *indices;
  uint32_t count;
} DtBatch;

typedef struct {
  float place[12];
  float sun[3], up[3];
  float lit;
  uint32_t held;
  float fov;
  uint32_t batches;
  DtBatch batch[DT_BATCHES];
} DtFigure;

typedef struct {
  float eye[3];
  float view[16];
  float fov;
  float haze[3], haze_from, haze_to;
  float sun_colour[3], sky_colour[3];
  uint32_t stood;
  float flash, flash_at[3];
  uint32_t pace, option;
  float nearer;
} DtView;

typedef struct {
  float frame, worst;
  uint32_t late, frames;
  float cpu, gpu;
  uint32_t draws, triangles;
} DtPerf;

/* Texture coordinates, colour, position: the effects, the sky and the muzzle's flash. */
typedef struct {
  float uv[2];
  uint8_t colour[4];
  float pos[3];
} DtFxVertex;

void dt_sizes(uint32_t out[8]);
/* The version of the pack the core reads (duty_pack::VERSION). */
uint32_t dt_pack_version(void);
/* `clock`: microseconds, for the core's own timers. */
const char *dt_init(const DtPack *pack, uint32_t (*clock)(void));
void dt_stage(uint32_t error, const char *message, uint32_t len);
void dt_prefs_stored(const char *text, uint32_t len);
uint32_t dt_prefs_take(char *out, uint32_t cap);
uint32_t dt_interface_open(void);
uint32_t dt_guest_due(uint32_t buttons, uint32_t touching, uint32_t every);
void dt_control(const char *text, uint32_t len);
void dt_step(const DtPad *pad, uint32_t ticks);
void dt_view(DtView *out);
uint32_t dt_choose(const DtDraw **draws);
uint32_t dt_figures(const DtFigure **figures, DtView *view);
void dt_effects(DtFxVertex *out, uint32_t *covering, uint32_t *glowing);
uint32_t dt_sky(DtFxVertex *vertices, uint16_t *indices, uint32_t *fan);
void dt_mix(int16_t *out, uint32_t frames);
void dt_report(const DtPerf *perf);
uint32_t dt_status(char *out, uint32_t cap, const char *extra, uint32_t extra_len);
#endif
