#include "libobs/build/libobs-graphics.pch.h"

/* Private graphics layout, the debug line and the device ABI for the Rust
 * cull and colour calls (obs-graphics::raster_entrypoints): the context
 * read, gs_valid's debug line, and one device dispatch per call. */
graphics_t *obs_graphics_raster_context(void);
graphics_t *obs_graphics_raster_context(void)
{
	return gs_get_context();
}

void obs_graphics_raster_debug(const char *function);
void obs_graphics_raster_debug(const char *function)
{
	blog(LOG_DEBUG, "%s: called while not in a graphics context", function);
}

void obs_graphics_raster_set_cull_mode(graphics_t *graphics, enum gs_cull_mode mode);
void obs_graphics_raster_set_cull_mode(graphics_t *graphics, enum gs_cull_mode mode)
{
	graphics->exports.device_set_cull_mode(graphics->device, mode);
}

enum gs_cull_mode obs_graphics_raster_get_cull_mode(graphics_t *graphics);
enum gs_cull_mode obs_graphics_raster_get_cull_mode(graphics_t *graphics)
{
	return graphics->exports.device_get_cull_mode(graphics->device);
}

void obs_graphics_raster_enable_color(graphics_t *graphics, bool red, bool green, bool blue, bool alpha);
void obs_graphics_raster_enable_color(graphics_t *graphics, bool red, bool green, bool blue, bool alpha)
{
	graphics->exports.device_enable_color(graphics->device, red, green, blue, alpha);
}
