#include "libobs/build/libobs-graphics.pch.h"

/* Keep private graphics layout, context validation and device ABI in C. */
graphics_t *obs_graphics_indexbuffer_context(void);
graphics_t *obs_graphics_indexbuffer_context(void)
{
	graphics_t *graphics = gs_get_context();
	if (!graphics)
		blog(LOG_DEBUG, "%s: called while not in a graphics context", "gs_indexbuffer_create");
	return graphics;
}

gs_indexbuffer_t *obs_graphics_indexbuffer_dispatch(graphics_t *graphics, enum gs_index_type type, void *indices,
						    size_t num, uint32_t flags);
gs_indexbuffer_t *obs_graphics_indexbuffer_dispatch(graphics_t *graphics, enum gs_index_type type, void *indices,
						    size_t num, uint32_t flags)
{
	return graphics->exports.device_indexbuffer_create(graphics->device, type, indices, num, flags);
}
