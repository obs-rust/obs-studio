#include "libobs/build/libobs-graphics.pch.h"

/* Private layout/dispatch only. Rust checks context before any field access. */
graphics_t *obs_graphics_device_queries_context(void);
graphics_t *obs_graphics_device_queries_context(void)
{
	return gs_get_context();
}

void obs_graphics_device_queries_debug(const char *function);
void obs_graphics_device_queries_debug(const char *function)
{
	blog(LOG_DEBUG, "%s: called while not in a graphics context", function);
}

int obs_graphics_device_queries_type(graphics_t *graphics);
int obs_graphics_device_queries_type(graphics_t *graphics)
{
	return graphics->exports.device_get_type();
}

bool obs_graphics_device_queries_has_adapter_count(graphics_t *graphics);
bool obs_graphics_device_queries_has_adapter_count(graphics_t *graphics)
{
	return graphics->exports.gs_get_adapter_count != NULL;
}

uint32_t obs_graphics_device_queries_count(graphics_t *graphics);
uint32_t obs_graphics_device_queries_count(graphics_t *graphics)
{
	return graphics->exports.gs_get_adapter_count();
}

bool obs_graphics_device_queries_has_nv12(graphics_t *graphics);
bool obs_graphics_device_queries_has_nv12(graphics_t *graphics)
{
	return graphics->exports.device_nv12_available != NULL;
}

bool obs_graphics_device_queries_nv12(graphics_t *graphics);
bool obs_graphics_device_queries_nv12(graphics_t *graphics)
{
	return graphics->exports.device_nv12_available(graphics->device);
}

bool obs_graphics_device_queries_has_p010(graphics_t *graphics);
bool obs_graphics_device_queries_has_p010(graphics_t *graphics)
{
	return graphics->exports.device_p010_available != NULL;
}

bool obs_graphics_device_queries_p010(graphics_t *graphics);
bool obs_graphics_device_queries_p010(graphics_t *graphics)
{
	return graphics->exports.device_p010_available(graphics->device);
}

gs_shader_t *obs_graphics_device_queries_vertex_shader(graphics_t *graphics);
gs_shader_t *obs_graphics_device_queries_vertex_shader(graphics_t *graphics)
{
	return graphics->exports.device_get_vertex_shader(graphics->device);
}

gs_shader_t *obs_graphics_device_queries_pixel_shader(graphics_t *graphics);
gs_shader_t *obs_graphics_device_queries_pixel_shader(graphics_t *graphics)
{
	return graphics->exports.device_get_pixel_shader(graphics->device);
}
