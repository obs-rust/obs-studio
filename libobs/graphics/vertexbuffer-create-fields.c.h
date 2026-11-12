#include "libobs/build/libobs-graphics.pch.h"

/* Keep private context/device layout and variadic logging at the C boundary. */
graphics_t *obs_graphics_vertexbuffer_create_context(void);
graphics_t *obs_graphics_vertexbuffer_create_context(void)
{
	graphics_t *graphics = gs_get_context();
	if (!graphics)
		blog(LOG_DEBUG, "%s: called while not in a graphics context", "gs_vertexbuffer_create");
	return graphics;
}

gs_vertbuffer_t *obs_graphics_vertexbuffer_create_dispatch(graphics_t *graphics, struct gs_vb_data *data,
							   uint32_t flags);
gs_vertbuffer_t *obs_graphics_vertexbuffer_create_dispatch(graphics_t *graphics, struct gs_vb_data *data,
							   uint32_t flags)
{
	return graphics->exports.device_vertexbuffer_create(graphics->device, data, flags);
}
