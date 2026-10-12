/* The PICA200 renderer of Pocket Duty. */
#ifndef DUTY_RENDER_H
#define DUTY_RENDER_H
#include <3ds.h>
#include <citro3d.h>
#include <stdbool.h>
#include <stddef.h>

#include "core.h"

typedef struct {
  const DtScene *scene;
  /* The street's vertices (12 bytes each) and indices: linear memory, flushed. */
  const uint8_t *vertices;
  const uint16_t *indices;
  const DtTexture *textures;
  uint32_t texture_count;
  const uint8_t *texels;
} RenderData;

typedef struct {
  uint32_t draws, triangles;
} RenderStats;
extern RenderStats render_stats;
/* Microseconds of the last frame: the sky, the street, the soldiers, the effects and the held weapon. */
extern uint32_t render_parts[4];

bool render_init(const RenderData *d, char *error, size_t n);
/* The whole upper screen but the interface: the sky, the street, the soldiers, the effects, the held weapon. */
void render_frame(C3D_RenderTarget *target, const DtView *view, const DtDraw *draws, uint32_t draw_count, const DtFigure *figures, uint32_t figure_count);
#endif
