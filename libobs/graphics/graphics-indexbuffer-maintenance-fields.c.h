#include "libobs/build/libobs-graphics.pch.h"

/* Private layout/dispatch only. Rust checks context and pointers first. */
graphics_t *obs_graphics_indexbuffer_maintenance_context(void);
graphics_t *obs_graphics_indexbuffer_maintenance_context(void)
{
	return gs_get_context();
}

void obs_graphics_indexbuffer_maintenance_debug(const char *function, const char *reason);
void obs_graphics_indexbuffer_maintenance_debug(const char *function, const char *reason)
{
	blog(LOG_DEBUG, "%s: %s", function, reason);
}

void obs_graphics_indexbuffer_maintenance_destroy(graphics_t *graphics, gs_indexbuffer_t *buffer);
void obs_graphics_indexbuffer_maintenance_destroy(graphics_t *graphics, gs_indexbuffer_t *buffer)
{
	graphics->exports.gs_indexbuffer_destroy(buffer);
}

void obs_graphics_indexbuffer_maintenance_flush(graphics_t *graphics, gs_indexbuffer_t *buffer);
void obs_graphics_indexbuffer_maintenance_flush(graphics_t *graphics, gs_indexbuffer_t *buffer)
{
	graphics->exports.gs_indexbuffer_flush(buffer);
}

void obs_graphics_indexbuffer_maintenance_direct(graphics_t *graphics, gs_indexbuffer_t *buffer, const void *data);
void obs_graphics_indexbuffer_maintenance_direct(graphics_t *graphics, gs_indexbuffer_t *buffer, const void *data)
{
	graphics->exports.gs_indexbuffer_flush_direct(buffer, data);
}

size_t obs_graphics_indexbuffer_maintenance_count(graphics_t *graphics, const gs_indexbuffer_t *buffer);
size_t obs_graphics_indexbuffer_maintenance_count(graphics_t *graphics, const gs_indexbuffer_t *buffer)
{
	return graphics->exports.gs_indexbuffer_get_num_indices(buffer);
}
