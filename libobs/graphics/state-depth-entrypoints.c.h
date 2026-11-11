#include "libobs/build/libobs-graphics.pch.h"

/* Standalone reference-group equivalents of graphics.c.h's private helpers. */
#define thread_graphics gs_get_context()
static inline bool depth_ref_valid(const char *function)
{
	if (!thread_graphics) {
		blog(LOG_DEBUG, "%s: called while not in a graphics context", function);
		return false;
	}
	return true;
}
#define gs_valid depth_ref_valid

void gs_enable_depth_test(bool enable)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_enable_depth_test"))
		return;

	graphics->exports.device_enable_depth_test(graphics->device, enable);
}

void gs_depth_function(enum gs_depth_test test)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_depth_function"))
		return;

	graphics->exports.device_depth_function(graphics->device, test);
}

#undef gs_valid
#undef thread_graphics
