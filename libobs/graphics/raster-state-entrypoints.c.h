#include "libobs/build/libobs-graphics.pch.h"

/* In its own group for the C reference: graphics.c.h's thread_graphics,
 * read through gs_get_context(), and its static gs_valid(), verbatim. */
#define thread_graphics gs_get_context()

static inline bool raster_ref_valid(const char *f)
{
	if (!thread_graphics) {
		blog(LOG_DEBUG, "%s: called while not in a graphics context", f);
		return false;
	}

	return true;
}
#define gs_valid raster_ref_valid

void gs_set_cull_mode(enum gs_cull_mode mode)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_set_cull_mode"))
		return;

	graphics->exports.device_set_cull_mode(graphics->device, mode);
}

enum gs_cull_mode gs_get_cull_mode(void)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_get_cull_mode"))
		return GS_NEITHER;

	return graphics->exports.device_get_cull_mode(graphics->device);
}

void gs_enable_color(bool red, bool green, bool blue, bool alpha)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_enable_color"))
		return;

	graphics->exports.device_enable_color(graphics->device, red, green, blue, alpha);
}

#undef gs_valid
#undef thread_graphics
