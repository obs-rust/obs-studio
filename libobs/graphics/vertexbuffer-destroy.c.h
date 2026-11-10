#include "libobs/build/libobs-graphics.pch.h"

extern graphics_t *obs_graphics_vertexbuffer_destroy_context(void);

/* Preserve the extracted body across the separate C-reference group. */
#define thread_graphics gs_get_context()
#define gs_valid(function) (obs_graphics_vertexbuffer_destroy_context() != NULL)

void gs_vertexbuffer_destroy(gs_vertbuffer_t *vertbuffer)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_vertexbuffer_destroy"))
		return;
	if (!vertbuffer)
		return;

	graphics->exports.gs_vertexbuffer_destroy(vertbuffer);
}

#undef gs_valid
#undef thread_graphics
