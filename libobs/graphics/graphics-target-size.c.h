#include "libobs/build/libobs-graphics.pch.h"

/* Reference-only group: original bodies over the C-owned TLS context getter. */
#define thread_graphics gs_get_context()

static inline bool target_size_ref_valid(const char *f)
{
	if (!thread_graphics) {
		blog(LOG_DEBUG, "%s: called while not in a graphics context", f);
		return false;
	}
	return true;
}

#define gs_valid target_size_ref_valid

void gs_get_size(uint32_t *x, uint32_t *y)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_get_size"))
		return;

	graphics->exports.device_get_size(graphics->device, x, y);
}

uint32_t gs_get_width(void)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_get_width"))
		return 0;

	return graphics->exports.device_get_width(graphics->device);
}

uint32_t gs_get_height(void)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_get_height"))
		return 0;

	return graphics->exports.device_get_height(graphics->device);
}

#undef gs_valid
#undef thread_graphics
