#include "libobs/build/libobs-graphics.pch.h"

/* Private layout and device ABI only; Rust owns the context guard. */
graphics_t *obs_graphics_device_load_context(void);
graphics_t *obs_graphics_device_load_context(void)
{
	return gs_get_context();
}

void obs_graphics_device_load_debug(const char *function);
void obs_graphics_device_load_debug(const char *function)
{
	blog(LOG_DEBUG, "%s: called while not in a graphics context", function);
}

void obs_graphics_device_load_vertexbuffer(graphics_t *graphics, gs_vertbuffer_t *buffer);
void obs_graphics_device_load_vertexbuffer(graphics_t *graphics, gs_vertbuffer_t *buffer)
{
	graphics->exports.device_load_vertexbuffer(graphics->device, buffer);
}

void obs_graphics_device_load_indexbuffer(graphics_t *graphics, gs_indexbuffer_t *buffer);
void obs_graphics_device_load_indexbuffer(graphics_t *graphics, gs_indexbuffer_t *buffer)
{
	graphics->exports.device_load_indexbuffer(graphics->device, buffer);
}

void obs_graphics_device_load_texture(graphics_t *graphics, gs_texture_t *texture, int unit);
void obs_graphics_device_load_texture(graphics_t *graphics, gs_texture_t *texture, int unit)
{
	graphics->exports.device_load_texture(graphics->device, texture, unit);
}

void obs_graphics_device_load_samplerstate(graphics_t *graphics, gs_samplerstate_t *sampler, int unit);
void obs_graphics_device_load_samplerstate(graphics_t *graphics, gs_samplerstate_t *sampler, int unit)
{
	graphics->exports.device_load_samplerstate(graphics->device, sampler, unit);
}

void obs_graphics_device_load_default_samplerstate(graphics_t *graphics, bool b_3d, int unit);
void obs_graphics_device_load_default_samplerstate(graphics_t *graphics, bool b_3d, int unit)
{
	graphics->exports.device_load_default_samplerstate(graphics->device, b_3d, unit);
}
