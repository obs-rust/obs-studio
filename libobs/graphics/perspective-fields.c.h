#include "libobs/build/libobs-graphics.pch.h"

/* Private graphics layout, the debug line and the device ABI for the Rust
 * gs_perspective (obs-graphics::perspective): the context read, gs_valid's
 * debug line, and the one device_frustum call. */
graphics_t *obs_graphics_perspective_context(void);
graphics_t *obs_graphics_perspective_context(void)
{
	return gs_get_context();
}

void obs_graphics_perspective_debug(const char *function);
void obs_graphics_perspective_debug(const char *function)
{
	blog(LOG_DEBUG, "%s: called while not in a graphics context", function);
}

void obs_graphics_perspective_frustum(graphics_t *graphics, float left, float right, float top, float bottom,
				      float znear, float zfar);
void obs_graphics_perspective_frustum(graphics_t *graphics, float left, float right, float top, float bottom,
				      float znear, float zfar)
{
	graphics->exports.device_frustum(graphics->device, left, right, top, bottom, znear, zfar);
}
