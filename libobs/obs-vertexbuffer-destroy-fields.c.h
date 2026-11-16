#include "libobs/build/libobs-graphics.pch.h"

/* Only the private context layout, validation log and indirect call live here. */
graphics_t *obs_graphics_vertexbuffer_destroy_context(void);
void obs_graphics_vertexbuffer_destroy_dispatch(graphics_t *graphics, gs_vertbuffer_t *buffer);

graphics_t *obs_graphics_vertexbuffer_destroy_context(void)
{
	graphics_t *graphics = gs_get_context();
	if (!graphics)
		blog(LOG_DEBUG, "%s: called while not in a graphics context", "gs_vertexbuffer_destroy");
	return graphics;
}

void obs_graphics_vertexbuffer_destroy_dispatch(graphics_t *graphics, gs_vertbuffer_t *buffer)
{
	graphics->exports.gs_vertexbuffer_destroy(buffer);
}
