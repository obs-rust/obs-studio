#include "libobs/build/libobs-graphics.pch.h"

/* Native layout/dispatch only. Rust checks context before any field access. */
graphics_t *obs_graphics_target_size_context(void);
graphics_t *obs_graphics_target_size_context(void)
{
	return gs_get_context();
}
void obs_graphics_target_size_debug(const char *function);
void obs_graphics_target_size_debug(const char *function)
{
	blog(LOG_DEBUG, "%s: called while not in a graphics context", function);
}
void obs_graphics_target_size_size(graphics_t *graphics, uint32_t *x, uint32_t *y);
void obs_graphics_target_size_size(graphics_t *graphics, uint32_t *x, uint32_t *y)
{
	graphics->exports.device_get_size(graphics->device, x, y);
}
uint32_t obs_graphics_target_size_width(graphics_t *graphics);
uint32_t obs_graphics_target_size_width(graphics_t *graphics)
{
	return graphics->exports.device_get_width(graphics->device);
}
uint32_t obs_graphics_target_size_height(graphics_t *graphics);
uint32_t obs_graphics_target_size_height(graphics_t *graphics)
{
	return graphics->exports.device_get_height(graphics->device);
}
