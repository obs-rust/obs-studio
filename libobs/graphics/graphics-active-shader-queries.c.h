#include "libobs/build/libobs-graphics.pch.h"

/* Private dependencies defined in graphics.c.h, later in the unity group. */
static THREAD_LOCAL graphics_t *thread_graphics;
static inline bool gs_valid(const char *f);

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
