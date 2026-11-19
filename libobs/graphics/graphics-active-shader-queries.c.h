#include "libobs/build/libobs-graphics.pch.h"

/* Reference-only: use the existing TLS context. */
#define thread_graphics gs_get_context()

static inline bool active_shader_ref_valid(const char *f)
{
	if (!thread_graphics) {
		blog(LOG_DEBUG, "%s: called while not in a graphics context", f);
		return false;
	}

	return true;
}

#define gs_valid active_shader_ref_valid

gs_shader_t *gs_get_vertex_shader(void)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_get_vertex_shader"))
		return NULL;

	return graphics->exports.device_get_vertex_shader(graphics->device);
}

gs_shader_t *gs_get_pixel_shader(void)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_get_pixel_shader"))
		return NULL;

	return graphics->exports.device_get_pixel_shader(graphics->device);
}

#undef gs_valid
#undef thread_graphics
