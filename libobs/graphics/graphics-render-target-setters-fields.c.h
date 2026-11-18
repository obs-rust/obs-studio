#include "libobs/build/libobs-graphics.pch.h"

/* Private layout/dispatch only. Rust checks the context before field access. */
graphics_t *obs_graphics_render_target_setters_context(void);
graphics_t *obs_graphics_render_target_setters_context(void)
{
	return gs_get_context();
}

void obs_graphics_render_target_setters_debug(const char *function, const char *reason);
void obs_graphics_render_target_setters_debug(const char *function, const char *reason)
{
	blog(LOG_DEBUG, "%s: %s", function, reason);
}

void obs_graphics_render_target_setters_target(graphics_t *graphics, gs_texture_t *texture, gs_zstencil_t *depth);
void obs_graphics_render_target_setters_target(graphics_t *graphics, gs_texture_t *texture, gs_zstencil_t *depth)
{
	graphics->exports.device_set_render_target(graphics->device, texture, depth);
}

void obs_graphics_render_target_setters_cube(graphics_t *graphics, gs_texture_t *texture, int side,
					     gs_zstencil_t *depth);
void obs_graphics_render_target_setters_cube(graphics_t *graphics, gs_texture_t *texture, int side,
					     gs_zstencil_t *depth)
{
	graphics->exports.device_set_cube_render_target(graphics->device, texture, side, depth);
}
