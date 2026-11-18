#include "libobs/build/libobs-graphics.pch.h"

/* Native layout and vtable dispatch only; Rust guards context before access. */
graphics_t *obs_graphics_samplerstate_context(void);
graphics_t *obs_graphics_samplerstate_context(void)
{
	return gs_get_context();
}
void obs_graphics_samplerstate_debug(const char *function, const char *reason);
void obs_graphics_samplerstate_debug(const char *function, const char *reason)
{
	blog(LOG_DEBUG, "%s: %s", function, reason);
}
gs_samplerstate_t *obs_graphics_samplerstate_create(graphics_t *graphics, const struct gs_sampler_info *info);
gs_samplerstate_t *obs_graphics_samplerstate_create(graphics_t *graphics, const struct gs_sampler_info *info)
{
	return graphics->exports.device_samplerstate_create(graphics->device, info);
}
void obs_graphics_samplerstate_destroy(graphics_t *graphics, gs_samplerstate_t *samplerstate);
void obs_graphics_samplerstate_destroy(graphics_t *graphics, gs_samplerstate_t *samplerstate)
{
	graphics->exports.gs_samplerstate_destroy(samplerstate);
}
