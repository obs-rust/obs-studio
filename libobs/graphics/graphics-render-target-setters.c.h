#include "libobs/build/libobs-graphics.pch.h"

/* Reference-only context access and original validator. */
#define thread_graphics gs_get_context()
static inline bool render_target_setters_ref_valid(const char *f)
{
	if (!thread_graphics) {
		blog(LOG_DEBUG, "%s: called while not in a graphics context", f);
		return false;
	}

	return true;
}
#define gs_valid render_target_setters_ref_valid

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

#undef gs_valid
#undef thread_graphics
