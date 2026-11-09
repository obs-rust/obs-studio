#include "libobs/build/libobs-graphics.pch.h"

void gs_vertexbuffer_destroy(gs_vertbuffer_t *vertbuffer)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_vertexbuffer_destroy"))
		return;
	if (!vertbuffer)
		return;

	graphics->exports.gs_vertexbuffer_destroy(vertbuffer);
}
