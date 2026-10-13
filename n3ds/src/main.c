/* Pocket Duty on Nintendo 3DS.
 *
 * The same street and match as the PSP build, lowered by the compiler for
 * this machine (profiles/n3ds30.json): 400 x 240 on the upper screen at 30
 * frames a second, the PICA200's vertex programs and fixed combiners. The
 * match, the choice of what a frame draws, the posing and the mixing are the
 * Rust core (core/), the same code as the PSP's.
 *
 * Every 2D pixel is the interface's: the PocketJS guest of ui/ (guest.c), over
 * the street on the upper screen and alone on the lower one, where it shows
 * the level from above and the keys a thumb or a stylus presses.
 *
 * Controls: the Circle Pad walks (all the way forward runs), A B X Y look, R
 * fires, L raises the sights, up on the +Control Pad jumps and climbs, down
 * crouches and slides, left changes the magazine, right takes the next
 * weapon, SELECT changes the fire mode. On a New 3DS the C-Stick looks too.
 * START and the touch screen are the interface's. L + R + START + SELECT leaves.
 *
 * Development loop over PocketJS's paired LAN wire (port 8131, compiled in
 * from vendor/pocketjs/hosts/3ds): {"t":"duty.control","text":"..."} steers
 * the run and is answered with a duty.status record; "screenshot" captures
 * both screens; a .3dsx install replaces this program. In an emulator,
 * sdmc:/pocket-duty/boot.txt holds control words, plus shot=N (write frame N
 * of both screens to shot.bgr and shot-low.bgr there), exit=N, and
 * @N:word for a word said at frame N. */
#include <3ds.h>
#include <citro3d.h>
#include <malloc.h>
#include <math.h>
#include <pocket3d_title.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>

#include "core.h"
#include "pocket3d_packs.h"
#include "devserver.h"
#include "gfx.h"
#include "guest.h"
#include "hbldr.h"
#include "native.h"
#include "render.h"
#include "soc.h"

/* QuickJS recurses: the guest compiles its bundle and mounts its screens on this thread, and PocketJS sets
 * the script's own limit for a megabyte of stack. */
unsigned int __stacksize__ = 1024 * 1024;
extern int __system_argc;
extern char **__system_argv;

/* PocketJS's dev wire names the application's directory on the card and where a guest package would be staged
 * (its own runtime keeps both in runtime.c). This program takes no guest packages: only a .3dsx replaces it. */
const char *runtime_slot(void) { return DUTY_SLOT; }
const char *runtime_upload_path(void) { return "sdmc:/pocketjs/runtime/apps/" DUTY_SLOT "/network-upload.pocket"; }

#define TAG(a, b, c, d) ((u32)(a) | ((u32)(b) << 8) | ((u32)(c) << 16) | ((u32)(d) << 24))

static const u32 transfer = GX_TRANSFER_FLIP_VERT(0) | GX_TRANSFER_OUT_TILED(0) | GX_TRANSFER_RAW_COPY(0) | GX_TRANSFER_IN_FORMAT(GX_TRANSFER_FMT_RGBA8) | GX_TRANSFER_OUT_FORMAT(GX_TRANSFER_FMT_RGB8) |
                            GX_TRANSFER_SCALING(GX_TRANSFER_SCALE_NO);

/* What the interface asked to have kept between runs. */
#define PREFS_PATH "sdmc:/3ds/pocket-duty/interface.json"

static const char *stage = "boot";
static char app_error[256];
static FILE *boot_log;
static C3D_RenderTarget *top, *bottom;
/* The lower screen is a text console: the interface did not start. */
static bool console;

/* A line for the boot log and the dev wire. */
static void say(const char *message) {
  if (boot_log) {
    fprintf(boot_log, "%s\n", message);
    fflush(boot_log);
  }
  devserver_report_log("info", message);
}

/* Without the interface an error has no screen of its own: the lower one becomes a console for it. */
static void console_error(const char *message) {
  if (!console) {
    gfxSetDoubleBuffering(GFX_BOTTOM, false);
    consoleInit(GFX_BOTTOM, NULL);
    console = true;
  }
  printf("%s\n", message);
  GSPGPU_FlushDataCache(gfxGetFramebuffer(GFX_BOTTOM, GFX_LEFT, NULL, NULL), 320 * 240 * 2);
}

/* One frame of the interface alone, while there is no city to draw. */
static void interface_frame(void) {
  if (!guest_running() || !C3D_FrameBegin(C3D_FRAME_SYNCDRAW))
    return;
  guest_prepare();
  C3D_RenderTargetClear(top, C3D_CLEAR_ALL, 0x000000ff, 0);
  C3D_FrameDrawOn(top);
  guest_draw_top();
  guest_draw_bottom(bottom);
  C3D_FrameEnd(0);
}

/* A loading step: the interface shows it before the step, which blocks, begins. */
static void loading(const char *message) {
  say(message);
  dt_stage(0, message, strlen(message));
  /* One turn takes the state in, the next has laid it out. */
  hidScanInput();
  guest_turn_now();
  guest_turn_now();
  interface_frame();
}

/* The start failed: `app_error` says why, on the interface or, without one, on the console. */
static void failed(void) {
  say(app_error);
  dt_stage(1, app_error, strlen(app_error));
  if (!guest_running())
    console_error(app_error);
}

/* ---------------------------------------------------------------- the pack */

typedef struct {
  u32 tag, offset, size, zero;
} Section;

/* The maps' packs are pieces named by their bytes, listed in one manifest (pocket3d_packs.h, PocketJS's
 * tools/pocket3d-packs.ts): `pocket.packs` and the pieces beside it. The card's copy is read when it is
 * there (a release's zip puts it in sdmc:/3ds/pocket-duty/packs/), with every map it lists; otherwise the
 * one the `.3dsx` carries in its ROMFS. */
static const char *const PACK_DIRS[] = {"sdmc:/3ds/pocket-duty/packs/", "romfs:/packs/"};
static const char *pack_dir;
static P3dPacks *packs;
static int held = -1;
static char held_id[P3D_PACK_NAME_BYTES + 1];
static Section sections[32];
static u32 section_count, pack_bytes;
/* The piece read last stays open: a section is read from one piece or from a few in a row. */
static FILE *piece;
static char piece_name[P3D_PIECE_FILE_BYTES];

static int32_t piece_read(void *user, const char *file, uint32_t at, uint8_t *into, uint32_t bytes) {
  (void)user;
  if (!piece || strcmp(file, piece_name) != 0) {
    char path[96];
    if (piece)
      fclose(piece);
    snprintf(path, sizeof path, "%s%s", pack_dir, file);
    piece = fopen(path, "rb");
    snprintf(piece_name, sizeof piece_name, "%s", piece ? file : "");
    if (!piece)
      return 2;
  }
  return fseek(piece, at, SEEK_SET) == 0 && fread(into, 1, bytes, piece) == bytes ? 0 : 3;
}

static bool pack_read(u32 offset, void *into, u32 bytes) { return p3d_packs_read(packs, held, offset, into, bytes, piece_read, NULL) == 0; }

static const Section *section(u32 tag) {
  for (u32 i = 0; i < section_count; i++)
    if (sections[i].tag == tag)
      return &sections[i];
  return NULL;
}

/* What `read_section` handed out for the map that is held: freed when another map is read. */
static struct {
  void *p;
  bool linear;
} owned[40];
static unsigned owned_count;

/* Reads a whole section into memory from `alloc` (malloc or linearAlloc). */
static void *read_section(u32 tag, void *(*alloc)(size_t), bool in_linear, u32 *size) {
  const Section *s = section(tag);
  if (!s || owned_count == sizeof owned / sizeof *owned)
    return NULL;
  void *p = alloc(s->size ? s->size : 1);
  if (!p)
    return NULL;
  owned[owned_count].p = p;
  owned[owned_count++].linear = in_linear;
  if (!pack_read(s->offset, p, s->size))
    return NULL;
  if (size)
    *size = s->size;
  return p;
}

static void free_sections(void) {
  while (owned_count) {
    owned_count--;
    if (owned[owned_count].linear)
      linearFree(owned[owned_count].p);
    else
      free(owned[owned_count].p);
  }
}

/* Whether every piece of a pack is in the directory. */
static bool pack_whole(u32 index) {
  for (u32 i = 0; i < p3d_packs_pieces(packs, index); i++) {
    char file[P3D_PIECE_FILE_BYTES], path[96];
    p3d_packs_piece(packs, index, i, file);
    snprintf(path, sizeof path, "%s%s", pack_dir, file);
    FILE *f = fopen(path, "rb");
    if (!f)
      return false;
    fclose(f);
  }
  return true;
}

/* Finds the manifest and says which maps there is a whole pack for (`have`: ids with spaces between). */
static bool find_packs(char *have, size_t cap) {
  for (unsigned d = 0; d < sizeof PACK_DIRS / sizeof *PACK_DIRS && !packs; d++) {
    char path[96];
    snprintf(path, sizeof path, "%spocket.packs", PACK_DIRS[d]);
    FILE *f = fopen(path, "rb");
    if (!f)
      continue;
    static char text[24 * 1024];
    size_t n = fread(text, 1, sizeof text, f);
    fclose(f);
    packs = p3d_packs_parse((const uint8_t *)text, n);
    pack_dir = PACK_DIRS[d];
  }
  if (!packs) {
    snprintf(app_error, sizeof app_error, "no maps: sdmc:/3ds/pocket-duty/packs/pocket.packs is missing or is not a manifest");
    return false;
  }
  have[0] = 0;
  for (u32 i = 0; i < p3d_packs_count(packs); i++) {
    char id[P3D_PACK_NAME_BYTES + 1];
    p3d_packs_name(packs, i, id);
    if (pack_whole(i) && strlen(have) + strlen(id) + 2 < cap) {
      if (have[0])
        strcat(have, " ");
      strcat(have, id);
    }
  }
  if (!have[0]) {
    snprintf(app_error, sizeof app_error, "no map's pack is whole in %s", pack_dir);
    return false;
  }
  return true;
}

/* Opens a map's pack: its table of sections. */
static bool open_pack(const char *id) {
  held = p3d_packs_find(packs, id);
  u32 head[4];
  if (held < 0 || !pack_read(0, head, 16) || head[0] != TAG('D', 'U', 'T', 'Y') || head[1] != dt_pack_version() || head[2] > 32 || !pack_read(16, sections, head[2] * sizeof(Section))) {
    snprintf(app_error, sizeof app_error, "the pack of \"%s\" is not a version %lu pack", id, (unsigned long)dt_pack_version());
    return false;
  }
  section_count = head[2];
  char name[P3D_PACK_NAME_BYTES + 1];
  pack_bytes = (u32)p3d_packs_name(packs, held, name);
  snprintf(held_id, sizeof held_id, "%s", id);
  return true;
}

static void *linear(size_t n) { return linearAlloc(n); }
/* A chunk's record is two lines of the data cache: the section starts on one. */
static void *lined(size_t n) { return memalign(64, n); }

static u32 micros(void) { return (u32)(svcGetSystemTick() * 1000000ULL / SYSCLOCK_ARM11); }

static char maps_have[256];

static bool load(const char *id) {
  u32 sizes[8];
  dt_sizes(sizes);
  if (sizes[0] != sizeof(DtPack) || sizes[1] != sizeof(DtDraw) || sizes[2] != sizeof(DtFigure) || sizes[3] != sizeof(DtView) || sizes[4] != sizeof(DtScene) || sizes[5] != sizeof(DtTexture) || sizes[6] != sizeof(DtFxVertex) ||
      sizes[7] != DT_FX_VERTICES) {
    snprintf(app_error, sizeof app_error, "core.h does not match the core library");
    return false;
  }
  if (!packs && !find_packs(maps_have, sizeof maps_have))
    return false;
  /* (the first map there is a pack for, when none is named) */
  char first[P3D_PACK_NAME_BYTES + 1];
  snprintf(first, sizeof first, "%.*s", (int)strcspn(maps_have, " "), maps_have);
  if (!open_pack(id ? id : first))
    return false;
  static RenderData data;
  static DtPack p;
  memset(&p, 0, sizeof p);
  u32 size = 0;
  loading("Reading the map");
  p.scene = read_section(TAG('S', 'C', 'E', 'N'), malloc, false, &size);
  p.chunks = read_section(TAG('C', 'H', 'N', 'K'), lined, false, &size);
  p.chunk_count = size / 128;
  p.clusters = read_section(TAG('C', 'L', 'U', 'S'), lined, false, &size);
  p.cluster_count = size / 32;
  u8 *vertices = read_section(TAG('W', 'V', 'T', 'X'), linear, true, &size);
  if (vertices)
    GSPGPU_FlushDataCache(vertices, size);
  u16 *indices = read_section(TAG('W', 'I', 'D', 'X'), linear, true, &size);
  if (indices)
    GSPGPU_FlushDataCache(indices, size);
  if (!p.scene || !p.chunks || !p.clusters || !vertices || !indices) {
    snprintf(app_error, sizeof app_error, "the pack lacks the street, or memory is short (%lu KiB of linear memory left)", (unsigned long)linearSpaceFree() / 1024);
    return false;
  }
  loading("Reading what is seen from where");
  p.cells = read_section(TAG('V', 'C', 'E', 'L'), malloc, false, &size);
  p.cell_count = size / 12;
  p.lists = read_section(TAG('V', 'L', 'S', 'T'), malloc, false, &size);
  p.list_count = size / 2;
  p.bits = read_section(TAG('V', 'B', 'I', 'T'), malloc, false, &p.bit_bytes);
  loading("Reading the pictures");
  p.textures = read_section(TAG('T', 'E', 'X', 'T'), malloc, false, &size);
  p.texture_count = size / sizeof(DtTexture);
  p.texels = read_section(TAG('T', 'E', 'X', 'D'), malloc, false, &p.texel_bytes);
  loading("Reading the ground");
  p.solid_verts = read_section(TAG('C', 'O', 'L', 'V'), malloc, false, &size);
  p.solid_vert_count = size / 12;
  p.solid_tris = read_section(TAG('C', 'O', 'L', 'T'), malloc, false, &size);
  p.solid_tri_count = size / 16;
  p.solid_nodes = read_section(TAG('C', 'O', 'L', 'N'), malloc, false, &size);
  p.solid_node_count = size / 32;
  p.nav = read_section(TAG('N', 'A', 'V', 'G'), malloc, false, &p.nav_bytes);
  p.weapons = read_section(TAG('W', 'E', 'A', 'P'), malloc, false, &size);
  p.weapon_bytes = p.weapons ? size : 0;
  if (!p.cells || !p.lists || !p.bits || !p.textures || !p.texels || !p.solid_verts || !p.solid_tris || !p.solid_nodes || !p.nav) {
    snprintf(app_error, sizeof app_error, "the pack lacks a table, or memory is short");
    return false;
  }
  loading("Reading the soldiers");
  /* (their vertices are drawn from where they are read: linear memory) */
  p.figures = read_section(TAG('F', 'I', 'G', 'S'), linear, true, &p.figure_bytes);
  if (p.figures)
    GSPGPU_FlushDataCache(p.figures, p.figure_bytes);
  loading("Reading the sounds");
  p.sounds = read_section(TAG('S', 'N', 'D', 'S'), malloc, false, &size);
  p.sound_bytes = p.sounds ? size : 0;
  p.samples = read_section(TAG('S', 'N', 'D', 'D'), malloc, false, &p.sample_bytes);
  /* (the sections are in memory. A file left open on the ROMFS is closed by the C library at exit, after the
   * ROMFS is gone.) */
  if (piece)
    fclose(piece);
  piece = NULL;
  loading("Starting");
  dt_maps(held_id, maps_have);
  const char *e = dt_init(&p, micros);
  if (e) {
    snprintf(app_error, sizeof app_error, "%s", e);
    return false;
  }
  data.scene = p.scene;
  data.vertices = vertices;
  data.indices = indices;
  data.textures = p.textures;
  data.texture_count = p.texture_count;
  data.texels = p.texels;
  return render_init(&data, app_error, sizeof app_error);
}

/* ---------------------------------------------------------------- sound
 *
 * A thread above the frame's priority keeps the output fed: the core mixes the playing sounds (duty_sim::mix,
 * the PSP's mixer) at 22 050 frames a second.
 *
 * The DSP takes blocks of 512 frames, three ahead. It needs its firmware on the card (sdmc:/3ds/dspfirm.cdc,
 * which Luma's Rosalina menu dumps); a console without it answers d880a7fa. Then the older sound service
 * plays instead: it repeats one buffer for each ear, and the thread writes ahead of where it plays. That
 * place is not asked: the service's sample timer and the system tick count the same crystal (four ticks to
 * one of its counts), so the tick says it. */
#define SOUND_BLOCK 512
#define SOUND_BLOCKS 3
#define RING 8192
/* Frames written ahead of the playing place: 70 ms. */
#define RING_LEAD 1536
static ndspWaveBuf sound_wave[SOUND_BLOCKS];
static volatile bool sound_quit;
/* 0: none; 1: the DSP; 2: the older sound service. */
static int sounding;
static Thread sound_thread;
/* What the DSP answered when it was started (0: it started). */
static Result sound_result;
static s16 *ring[2];
static u64 ring_start, ring_written;
static u32 ring_timer;

/* Held by a worker around each call of the mixer, and by the frame's thread while a map's pack is let go of
 * and another is read: the mixer reads the pack's sounds. */
static LightLock mixing;
static void mix(s16 *out, u32 frames) {
  LightLock_Lock(&mixing);
  dt_mix(out, frames);
  LightLock_Unlock(&mixing);
}

static void sound_worker(void *arg) {
  (void)arg;
  while (!sound_quit) {
    bool fed = false;
    for (int i = 0; i < SOUND_BLOCKS; i++) {
      ndspWaveBuf *w = &sound_wave[i];
      if (w->status != NDSP_WBUF_FREE && w->status != NDSP_WBUF_DONE)
        continue;
      mix(w->data_pcm16, SOUND_BLOCK);
      DSP_FlushDataCache(w->data_pcm16, SOUND_BLOCK * 4);
      ndspChnWaveBufAdd(0, w);
      fed = true;
    }
    if (!fed)
      svcSleepThread(4000000);
  }
}

static void ring_worker(void *arg) {
  (void)arg;
  static s16 block[SOUND_BLOCK * 2];
  while (!sound_quit) {
    u64 played = (svcGetSystemTick() - ring_start) / (4ULL * ring_timer);
    /* (a frame that took too long: what was not written in time is not heard) */
    if (ring_written < played)
      ring_written = played;
    bool wrote = false;
    while (ring_written + SOUND_BLOCK <= played + RING_LEAD) {
      mix(block, SOUND_BLOCK);
      for (unsigned i = 0; i < SOUND_BLOCK; i++) {
        unsigned at = (ring_written + i) % RING;
        ring[0][at] = block[i * 2];
        ring[1][at] = block[i * 2 + 1];
      }
      ring_written += SOUND_BLOCK;
      wrote = true;
    }
    if (wrote) {
      GSPGPU_FlushDataCache(ring[0], RING * 2);
      GSPGPU_FlushDataCache(ring[1], RING * 2);
    }
    svcSleepThread(8000000);
  }
}

static int sound_start(void) {
  s32 priority = 0x30;
  svcGetThreadPriority(&priority, CUR_THREAD_HANDLE);
  sound_result = ndspInit();
  if (R_SUCCEEDED(sound_result)) {
    ndspSetOutputMode(NDSP_OUTPUT_STEREO);
    ndspChnReset(0);
    ndspChnSetInterp(0, NDSP_INTERP_LINEAR);
    ndspChnSetRate(0, 22050.0f);
    ndspChnSetFormat(0, NDSP_FORMAT_STEREO_PCM16);
    float mix[12] = {1.0f, 1.0f};
    ndspChnSetMix(0, mix);
    s16 *memory = linearAlloc(SOUND_BLOCK * 4 * SOUND_BLOCKS);
    if (memory) {
      memset(memory, 0, SOUND_BLOCK * 4 * SOUND_BLOCKS);
      for (int i = 0; i < SOUND_BLOCKS; i++) {
        sound_wave[i].data_pcm16 = memory + i * SOUND_BLOCK * 2;
        sound_wave[i].nsamples = SOUND_BLOCK;
        sound_wave[i].status = NDSP_WBUF_FREE;
      }
      sound_thread = threadCreate(sound_worker, NULL, 16 * 1024, priority - 1, -2, false);
      if (sound_thread)
        return 1;
    }
    ndspExit();
    return 0;
  }
  if (R_FAILED(csndInit()))
    return 0;
  ring[0] = linearAlloc(RING * 2);
  ring[1] = linearAlloc(RING * 2);
  if (ring[0] && ring[1]) {
    memset(ring[0], 0, RING * 2);
    memset(ring[1], 0, RING * 2);
    GSPGPU_FlushDataCache(ring[0], RING * 2);
    GSPGPU_FlushDataCache(ring[1], RING * 2);
    /* The service divides its clock by a whole number: the rate it plays at is that clock over the number. */
    ring_timer = CSND_TIMER(22050);
    csndPlaySound(8, SOUND_FORMAT_16BIT | SOUND_REPEAT, 22050, 1.0f, -1.0f, ring[0], ring[0], RING * 2);
    csndPlaySound(9, SOUND_FORMAT_16BIT | SOUND_REPEAT, 22050, 1.0f, 1.0f, ring[1], ring[1], RING * 2);
    ring_start = svcGetSystemTick();
    ring_written = 0;
    sound_thread = threadCreate(ring_worker, NULL, 16 * 1024, priority - 1, -2, false);
    if (sound_thread)
      return 2;
    CSND_SetPlayState(8, 0);
    CSND_SetPlayState(9, 0);
    csndExecCmds(true);
  }
  csndExit();
  return 0;
}

static void sound_stop(void) {
  if (!sounding)
    return;
  sound_quit = true;
  threadJoin(sound_thread, 1000000000ULL);
  threadFree(sound_thread);
  if (sounding == 1) {
    ndspChnWaveBufClear(0);
    ndspExit();
  } else {
    CSND_SetPlayState(8, 0);
    CSND_SetPlayState(9, 0);
    csndExecCmds(true);
    csndExit();
  }
}

/* ---------------------------------------------------------------- the wire */

static DtPerf perf;
static float cpu_ms, gpu_ms;
/* The interface: the frame's share of its turns (smoothed), and what starting it took of each memory. */
static float ui_ms, list_ms, busy_ms;
/* Microseconds of the last frame: the interface over the street and on the lower screen, and handing the frame to the GPU. */
static u32 ui_draw_us, end_us;
static u32 guest_heap, guest_linear;
static u32 frame_no;
static bool new3ds;

/* `text` with what a JSON string cannot hold as it is escaped or blanked. */
static void escape(char *out, size_t cap, const char *text) {
  size_t n = 0;
  for (; *text && n + 2 < cap; text++) {
    char c = *text;
    if (c == '"' || c == '\\')
      out[n++] = '\\';
    out[n++] = c < ' ' ? ' ' : c;
  }
  out[n] = 0;
}

static int status_text(char *line, size_t cap) {
  static char body[2048], extra[1024], guest[2 * 192];
  escape(guest, sizeof guest, guest_error());
  struct mallinfo heap = mallinfo();
  int n = snprintf(extra, sizeof extra,
                   "\"build\":\"" DUTY_BUILD_ID "\",\"phase\":\"%s\",\"error\":\"%s\",\"packBytes\":%lu,\"linearFree\":%lu,\"vramFree\":%lu,\"heap\":{\"used\":%lu,\"size\":%lu},\"new3ds\":%s,\"sound\":%s,\"soundResult\":\"%08lx\",\"listMs\":%.3f,\"busyMs\":%.3f,\"partsUs\":[%lu,%lu,%lu,%lu,%lu,%lu],"
                   "\"interface\":{\"up\":%s,\"open\":%s,\"error\":\"%s\",\"turns\":%lu,\"lowerDraws\":%lu,\"turnMs\":%.3f,\"scriptMs\":%.3f,\"worstTurnMs\":%.3f,\"cpuMs\":%.3f,\"commands\":%lu,\"vertices\":%lu,\"dropped\":%lu,\"heapBytes\":%lu,\"linearBytes\":%lu}",
                   stage, app_error, (unsigned long)pack_bytes, (unsigned long)linearSpaceFree(), (unsigned long)vramSpaceFree(), (unsigned long)heap.uordblks, (unsigned long)envGetHeapSize(), new3ds ? "true" : "false",
                   sounding == 1 ? "\"dsp\"" : sounding == 2 ? "\"csnd\"" : "false", (unsigned long)sound_result, list_ms, busy_ms, (unsigned long)render_parts[0], (unsigned long)render_parts[1], (unsigned long)render_parts[2], (unsigned long)render_parts[3], (unsigned long)ui_draw_us, (unsigned long)end_us, guest_running() ? "true" : "false", dt_interface_open() ? "true" : "false", guest,
                   (unsigned long)guest_turns(), (unsigned long)guest_lower_draws(), guest_turn_ms(), guest_script_ms(), guest_worst_ms(), ui_ms, (unsigned long)gfx_frame_commands(), (unsigned long)gfx_frame_vertices(), (unsigned long)gfx_dropped_vertices(), (unsigned long)guest_heap,
                   (unsigned long)guest_linear);
  if (!strcmp(stage, "running")) {
    dt_status(body, sizeof body, extra, n);
    /* {"target":... becomes {"t":"duty.status","target":... */
    return snprintf(line, cap, "{\"t\":\"duty.status\",%s", body + 1);
  }
  return snprintf(line, cap, "{\"t\":\"duty.status\",\"target\":\"3ds\",\"stage\":\"%s\",%s}", stage, extra);
}

static void report_status(void) {
  static char line[3400];
  int n = status_text(line, sizeof line);
  devserver_send_ctrl(line, n);
}

/* Control words: the core's (dt_control: mode=, ui=, pad=, at=, auto=, god=, ...) and this host's own:
 *   press=MASK       PocketJS BTN bits, pressed on the interface as a thumb would; 0 rests as long
 *   touch=X,Y        a stylus held on the lower screen; touch=off lifts it */
static void words(const char *text, size_t length) {
  static char copy[1024];
  snprintf(copy, sizeof copy, "%.*s", (int)length, text);
  char *save = NULL;
  for (char *word = strtok_r(copy, " \n", &save); word; word = strtok_r(NULL, " \n", &save)) {
    int x, y;
    if (!strncmp(word, "press=", 6))
      guest_press(strtoul(word + 6, NULL, 0));
    else if (!strncmp(word, "touch=", 6))
      guest_touch(sscanf(word + 6, "%d,%d", &x, &y) == 2, x, y);
  }
  if (!strcmp(stage, "running"))
    dt_control(text, length);
}

/* The "text" member of a control line, no escapes. */
static void controls(const char *line) {
  if (!strstr(line, "\"duty.control\""))
    return;
  const char *t = strstr(line, "\"text\":\"");
  const char *end = t ? strchr(t + 8, '"') : NULL;
  if (end)
    words(t + 8, end - (t + 8));
  report_status();
}

/* Both screens, read back from their targets after the GPU finished them. */
static bool capture(void) {
  uint8_t *upper = NULL, *lower = NULL;
  if (!devserver_screenshot_begin(frame_no, 400, 240, 320, 240, &upper, &lower))
    return false;
  C3D_SyncDisplayTransfer(top->frameBuf.colorBuf, GX_BUFFER_DIM(240, 400), (u32 *)upper, GX_BUFFER_DIM(240, 400), transfer);
  GSPGPU_InvalidateDataCache(upper, 400 * 240 * 3);
  if (bottom) {
    C3D_SyncDisplayTransfer(bottom->frameBuf.colorBuf, GX_BUFFER_DIM(240, 320), (u32 *)lower, GX_BUFFER_DIM(240, 320), transfer);
    GSPGPU_InvalidateDataCache(lower, 320 * 240 * 3);
  } else {
    memset(lower, 0, 320 * 240 * 3);
  }
  return true;
}

/* An emulator run: a screen as it is in its render target, to the memory card. */
static void write_shot(C3D_RenderTarget *target, const char *path) {
  unsigned width = target->frameBuf.height, height = target->frameBuf.width;
  u8 *out = linearAlloc(width * height * 3);
  if (!out)
    return;
  C3D_SyncDisplayTransfer(target->frameBuf.colorBuf, GX_BUFFER_DIM(height, width), (u32 *)out, GX_BUFFER_DIM(height, width), transfer);
  GSPGPU_InvalidateDataCache(out, width * height * 3);
  FILE *f = fopen(path, "wb");
  if (f) {
    fwrite(out, 1, width * height * 3, f);
    fclose(f);
  }
  linearFree(out);
}

/* What the interface asked to have stored goes to the card. */
static void keep_prefs(void) {
  static char text[4096];
  uint32_t n = dt_prefs_take(text, sizeof text);
  FILE *f = n ? fopen(PREFS_PATH, "w") : NULL;
  if (f) {
    fwrite(text, 1, n, f);
    fclose(f);
  }
}

static void read_pad(DtPad *pad) {
  static const u32 map[][2] = {{KEY_DUP, DT_UP}, {KEY_DDOWN, DT_DOWN}, {KEY_DLEFT, DT_LEFT}, {KEY_DRIGHT, DT_RIGHT}, {KEY_X, DT_NORTH}, {KEY_A, DT_EAST}, {KEY_B, DT_SOUTH}, {KEY_Y, DT_WEST},
                               {KEY_L, DT_L}, {KEY_ZL, DT_L}, {KEY_R, DT_R}, {KEY_ZR, DT_R}, {KEY_START, DT_START}, {KEY_SELECT, DT_SELECT}};
  u32 held = hidKeysHeld();
  pad->held = 0;
  for (unsigned i = 0; i < sizeof map / sizeof *map; i++)
    if (held & map[i][0])
      pad->held |= map[i][1];
  /* The Circle Pad reaches about 150 units; nothing inside a sixth of that, the rest rescaled. The C-Stick of a
   * New 3DS reads the same way, and an Old 3DS reads none. */
  circlePosition c, look = {0, 0};
  hidCircleRead(&c);
  if (new3ds)
    hidCstickRead(&look);
  float axes[4] = {c.dx, c.dy, look.dx, look.dy};
  for (int i = 0; i < 4; i++) {
    float v = fmaxf(-1.0f, fminf(1.0f, axes[i] / 150.0f)), m = fabsf(v);
    axes[i] = m < 0.16f ? 0.0f : copysignf((m - 0.16f) / 0.84f, v);
  }
  pad->sx = axes[0];
  pad->sy = axes[1];
  pad->rx = axes[2];
  pad->ry = axes[3];
}

int main(void) {
  gfxInitDefault();
  gfxSet3D(false);
  mkdir("sdmc:/pocket-duty", 0777);
  mkdir("sdmc:/3ds", 0777);
  mkdir("sdmc:/3ds/pocket-duty", 0777);
  /* Test runs: control words for the start, title=0, shot=N, exit=N. */
  static char boot[512];
  FILE *bf = fopen("sdmc:/pocket-duty/boot.txt", "r");
  if (bf) {
    size_t n = fread(boot, 1, sizeof boot - 1, bf);
    boot[n] = 0;
    fclose(bf);
  }
  long shot_at = -1, exit_at = -1;
  const char *w;
  if ((w = strstr(boot, "shot=")))
    shot_at = atol(w + 5);
  if ((w = strstr(boot, "exit=")))
    exit_at = atol(w + 5);
  /* The Pocket3D title card, on both screens, before the GPU is set up. */
  if (!strstr(boot, "title=0"))
    pocket3d_title_play();
  boot_log = fopen("sdmc:/pocket-duty/boot.log", "w");
  say("Pocket Duty " DUTY_BUILD_ID);
  char error[256] = {0};
  PocketRuntimeState state = {0};
  if (__system_argc > 0 && __system_argv)
    native_set_running_path(__system_argv[0]);
  mkdir("sdmc:/pocketjs", 0777);
  mkdir("sdmc:/pocketjs/runtime", 0777);
  devserver_allow_packages(false);
  DevserverInitResult dev = devserver_init(&state, error, sizeof error);
  say(dev == DEVSERVER_READY ? "Remote debugger ready" : error);
  /* A New 3DS runs at 804 MHz when asked; an Old 3DS ignores it. */
  osSetSpeedupEnable(true);
  APT_CheckNew3DS(&new3ds);
  /* (a frame is a few hundred draws, each with a matrix of its own: four times the usual command buffer) */
  bool gpu_ok = C3D_Init(C3D_DEFAULT_CMDBUF_SIZE * 4), gpu_stalled = false;
  top = gpu_ok ? C3D_RenderTargetCreate(240, 400, GPU_RB_RGBA8, GPU_RB_DEPTH24_STENCIL8) : NULL;
  if (top)
    C3D_RenderTargetSetOutput(top, GFX_TOP, GFX_LEFT, transfer);
  Result romfs = romfsInit();
  bool loaded = false;
  stage = "loading";
  devserver_set_runtime(&state, NULL, stage, 0);
  devserver_poll();

  /* The interface comes up before the pack is read, so it can show the reading. */
  if (top && R_SUCCEEDED(romfs)) {
    static char prefs[4096];
    FILE *kept = fopen(PREFS_PATH, "r");
    if (kept) {
      dt_prefs_stored(prefs, fread(prefs, 1, sizeof prefs - 1, kept));
      fclose(kept);
    }
    u32 heap_before = mallinfo().uordblks, linear_before = linearSpaceFree();
    /* The interface draws without depth: the lower target has a colour buffer only. */
    bottom = C3D_RenderTargetCreate(240, 320, GPU_RB_RGBA8, -1);
    if (bottom && guest_boot(error, sizeof error)) {
      C3D_RenderTargetSetOutput(bottom, GFX_BOTTOM, GFX_LEFT, transfer);
      /* A few turns mount its screens: what it holds after them is what it takes. */
      dt_stage(0, "", 0);
      for (int i = 0; i < 4; i++)
        guest_turn_now();
      interface_frame();
      guest_heap = mallinfo().uordblks - heap_before;
      guest_linear = linear_before - linearSpaceFree();
    } else {
      if (bottom)
        C3D_RenderTargetDelete(bottom);
      else
        snprintf(error, sizeof error, "no video memory for the lower screen");
      bottom = NULL;
      say(error);
      console_error("The interface did not start:");
      console_error(error);
    }
  }

  if (!gpu_ok || !top)
    snprintf(app_error, sizeof app_error, "PICA initialization failed");
  else if (R_FAILED(romfs))
    snprintf(app_error, sizeof app_error, "romfsInit failed: %08lx", (unsigned long)romfs);
  else {
    LightLock_Init(&mixing);
    loaded = load(NULL);
  }
  if (loaded) {
    sounding = sound_start();
    say(sounding == 1 ? "Sound through the DSP" : sounding == 2 ? "Sound through the older service: the DSP's firmware is not on the card (sdmc:/3ds/dspfirm.cdc)" : "No sound");
    stage = "running";
    if (boot[0])
      words(boot, strlen(boot));
  } else {
    stage = "load-error";
    failed();
  }
  devserver_set_runtime(&state, NULL, stage, 0);
  u64 last = svcGetSystemTick(), next_retry = 0;
  u32 ticks = 2, late = 0, last_count = C3D_FrameCounter(0);
  float worst = 0, avg = 33.3f;
  bool shot = false, capture_pending = false, lower_stale = false;
  while (aptMainLoop()) {
    hidScanInput();
    u32 held = hidKeysHeld();
    if ((held & (KEY_L | KEY_R | KEY_START | KEY_SELECT)) == (KEY_L | KEY_R | KEY_START | KEY_SELECT))
      break;
    if (!capture_pending)
      devserver_poll();
    u64 now = svcGetSystemTick();
    if (dev != DEVSERVER_READY && now >= next_retry) {
      dev = devserver_init(&state, error, sizeof error);
      next_retry = now + SYSCLOCK_ARM11 * 3ULL;
    }
    char launch[POCKET_RUNTIME_NATIVE_NAME_BYTES + 1], path[POCKET_NATIVE_PATH_BYTES];
    if (devserver_take_launch(launch) && native_path_for(launch, path)) {
      if (hbldr_launch_on_exit(path, error, sizeof error)) {
        devserver_flush(1000);
        devserver_report_native("launching", launch, "exiting to start it");
        devserver_poll();
        devserver_flush(1000);
        for (unsigned i = 0; i < 200; i++) {
          devserver_poll();
          svcSleepThread(1000000);
        }
        break;
      }
      devserver_report_native("launch-error", launch, error);
    }
    static char control[8192];
    size_t n;
    while ((n = devserver_recv_ctrl(control, sizeof control - 1)) > 0) {
      control[n] = 0;
      char *save = NULL;
      for (char *line = strtok_r(control, "\n", &save); line; line = strtok_r(NULL, "\n", &save)) {
        if (strstr(line, "\"screenshot\""))
          devserver_request_screenshot();
        controls(line);
      }
    }
    if (native_receiving() || gpu_stalled || !top) {
      svcSleepThread(1000000);
      continue;
    }
    if (!loaded) {
      /* (an emulator run that did not load ends here, with its error in boot.log) */
      if (exit_at >= 0)
        break;
      /* The interface says why; without it the console already has. */
      if (guest_running()) {
        guest_turn(1.0f / 60);
        interface_frame();
        /* A capture reads the targets once the GPU has finished the frame just queued. */
        if (devserver_take_screenshot_request() && C3D_FrameBegin(C3D_FRAME_SYNCDRAW)) {
          bool captured = capture();
          C3D_FrameEnd(0);
          if (captured)
            devserver_screenshot_ready();
        }
      } else {
        svcSleepThread(1000000);
      }
      continue;
    }
    if (exit_at >= 0 && frame_no < 3)
      say(frame_no == 0 ? "Frame 0" : frame_no == 1 ? "Frame 1" : "Frame 2");
    float frame_ms = (float)(now - last) * 1000.0f / SYSCLOCK_ARM11;
    last = now;

    /* ------------------------------------------------------------ input, the match, the frame's draws */
    /* The words boot.txt keeps for this frame. */
    for (const char *at = strchr(boot, '@'); at; at = strchr(at + 1, '@')) {
      char *colon;
      if (strtoul(at + 1, &colon, 10) == frame_no && *colon == ':') {
        size_t length = strcspn(colon + 1, " \n");
        words(colon + 1, length);
      }
    }
    DtPad pad;
    read_pad(&pad);
    /* Does what the interface asked on its last turn, then shows it the match as it now is. */
    /* A match asked on another map: its pack takes the place of the one in memory. */
    char wanted[P3D_PACK_NAME_BYTES + 1];
    if (dt_wanted(wanted, sizeof wanted)) {
      LightLock_Lock(&mixing);
      dt_drop();
      render_drop();
      free_sections();
      loaded = load(wanted);
      LightLock_Unlock(&mixing);
      if (!loaded) {
        stage = "load-error";
        failed();
      }
      continue;
    }
    dt_step(&pad, ticks);
    dt_report(&perf);
    keep_prefs();
    DtView view;
    dt_view(&view);
    /* (chosen while the GPU draws the frame before) */
    const DtDraw *draws = NULL;
    u32 draw_count = dt_choose(&draws);
    const DtFigure *figures = NULL;
    u32 figure_count = dt_figures(&figures, &view);
    if (exit_at >= 0 && frame_no < 2)
      say("Chosen");
    float chosen_ms = (float)(svcGetSystemTick() - now) * 1000.0f / SYSCLOCK_ARM11;

    /* ------------------------------------------------------------ the interface's turn, beside the GPU's work on the frame before */
    u64 turn_start = svcGetSystemTick();
    bool was_running = guest_running();
    /* (option 512 withholds the turn, to measure what it costs) */
    /* (a frame with no room for a turn waits a frame more for its readouts) */
    guest_readouts_every = busy_ms > 26.0f ? 3 : 2;
    if (!(view.option & 512))
      guest_turn(ticks / 60.0f);
    if (was_running && !guest_running()) {
      devserver_report_log("error", guest_error());
      /* Its last picture does not stay on the lower screen. */
      lower_stale = true;
    }
    float turn_ms = (float)(svcGetSystemTick() - turn_start) * 1000.0f / SYSCLOCK_ARM11;
    ui_ms += (turn_ms - ui_ms) * 0.05f;

    /* ------------------------------------------------------------ the previous frame leaves the GPU */
    u64 wait_start = svcGetSystemTick();
    while (!C3D_FrameBegin(C3D_FRAME_NONBLOCK)) {
      if (!capture_pending)
        devserver_poll();
      svcSleepThread(100000);
      if (svcGetSystemTick() - wait_start > SYSCLOCK_ARM11 * 4ULL) {
        stage = "gpu-timeout";
        say("GPU timeout: the debugger remains available");
        devserver_set_runtime(&state, NULL, stage, 0);
        gpu_stalled = true;
        break;
      }
    }
    if (gpu_stalled)
      continue;
    gpu_ms = C3D_GetDrawingTime();
    if (capture_pending) {
      devserver_screenshot_ready();
      capture_pending = false;
    }
    if (shot) {
      capture_pending = capture();
      shot = false;
    }
    if (exit_at >= 0 && (long)frame_no >= exit_at) {
      static char line[3400];
      status_text(line, sizeof line);
      FILE *f = fopen("sdmc:/pocket-duty/status.json", "w");
      if (f) {
        fputs(line, f);
        fclose(f);
      }
      f = fopen("sdmc:/pocket-duty/done", "w");
      if (f)
        fclose(f);
      break;
    }
    /* Hold the pace: a frame is shown for `pace` refreshes. */
    while (C3D_FrameCounter(0) - last_count < view.pace) {
      if (!capture_pending)
        devserver_poll();
      svcSleepThread(100000);
    }
    u32 count = C3D_FrameCounter(0);
    ticks = count - last_count;
    last_count = count;
    if (ticks > view.pace)
      late++;
    if (ticks > view.pace + 1)
      ticks = view.pace + 1;
    avg += (frame_ms - avg) * 0.05f;
    worst = frame_no % 120 == 0 ? frame_ms : fmaxf(worst, frame_ms);

    /* ------------------------------------------------------------ this frame */
    u64 build_start = svcGetSystemTick();
    /* The interface's vertices when it took a turn; the street; the interface over it; the lower screen on the
     * frames its list changed (it keeps what it showed on the others). */
    guest_prepare();
    render_frame(top, &view, draws, draw_count, figures, figure_count);
    u64 ui_from = svcGetSystemTick();
    guest_draw_top();
    if (bottom && (guest_lower_changed() || lower_stale || (long)frame_no == shot_at)) {
      guest_draw_bottom(bottom);
      lower_stale = false;
    }
    if (exit_at >= 0 && frame_no < 2)
      say("Drawn");
    u64 end_from = svcGetSystemTick();
    ui_draw_us = (u32)((end_from - ui_from) * 1000000ULL / SYSCLOCK_ARM11);
    C3D_FrameEnd(0);
    end_us = (u32)((svcGetSystemTick() - end_from) * 1000000ULL / SYSCLOCK_ARM11);
    /* (an emulator run's pictures: this frame, once the GPU has drawn it) */
    if ((long)frame_no == shot_at) {
      write_shot(top, "sdmc:/pocket-duty/shot.bgr");
      if (bottom)
        write_shot(bottom, "sdmc:/pocket-duty/shot-low.bgr");
    }
    float build_ms = (float)(svcGetSystemTick() - build_start) * 1000.0f / SYSCLOCK_ARM11;
    cpu_ms = chosen_ms + turn_ms + build_ms;
    list_ms += (build_ms - list_ms) * 0.1f;
    /* What the frame took of this CPU and the GPU, the interface's turn apart. */
    busy_ms += (fmaxf(chosen_ms + build_ms, gpu_ms) - busy_ms) * 0.2f;
    perf = (DtPerf){.frame = avg, .worst = worst, .late = late, .frames = frame_no, .cpu = cpu_ms, .gpu = gpu_ms, .draws = render_stats.draws, .triangles = render_stats.triangles};
    frame_no++;
    devserver_set_runtime(&state, NULL, stage, frame_no);
    devserver_set_frame_stats(frame_no, render_stats.draws, render_stats.triangles * 3, 0);
    devserver_set_frame_timing((uint32_t)(turn_ms * 1000), (uint32_t)(chosen_ms * 1000), (uint32_t)(build_ms * 1000), (uint32_t)(gpu_ms * 1000), (uint32_t)(frame_ms * 1000));
    shot = shot || devserver_take_screenshot_request();
  }

  sound_stop();
  if (gpu_ok && !gpu_stalled) {
    /* Retire the frame in flight before the targets go. */
    for (unsigned i = 0; i < 4000 && !C3D_FrameBegin(C3D_FRAME_NONBLOCK); i++) {
      devserver_poll();
      svcSleepThread(1000000);
    }
    C3D_FrameEnd(0);
    guest_shutdown();
    if (top)
      C3D_RenderTargetDelete(top);
    if (bottom)
      C3D_RenderTargetDelete(bottom);
    C3D_Fini();
  }
  if (!gpu_stalled || !capture_pending)
    devserver_shutdown();
  soc_shutdown();
  if (R_SUCCEEDED(romfs))
    romfsExit();
  if (native_exit_pending() && !native_finish_exit(error, sizeof error))
    say(error);
  if (boot_log)
    fclose(boot_log);
  gfxExit();
  return 0;
}
