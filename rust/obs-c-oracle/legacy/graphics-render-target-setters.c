#include "libobs/build/libobs-graphics.pch.h"

/* Private dependencies defined in graphics.c.h, later in the unity group. */
static THREAD_LOCAL graphics_t *thread_graphics;
static inline bool gs_valid(const char *f);

void gs_set_render_target(gs_texture_t *tex, gs_zstencil_t *zstencil)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_set_render_target"))
		return;

	graphics->exports.device_set_render_target(graphics->device, tex, zstencil);
}

void gs_set_cube_render_target(gs_texture_t *cubetex, int side, gs_zstencil_t *zstencil)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_set_cube_render_target"))
		return;

	graphics->exports.device_set_cube_render_target(graphics->device, cubetex, side, zstencil);
}
