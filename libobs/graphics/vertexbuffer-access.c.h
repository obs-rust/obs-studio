#include "libobs/build/libobs-graphics.pch.h"

/* Follow graphics.c.h in the graphics unity group: retain its private context
 * and validation helpers until this unit is replaced by Rust. */
#define gs_valid_p(func, param1) (gs_valid(func) && gs_obj_valid(param1, func, #param1))

void gs_vertexbuffer_flush(gs_vertbuffer_t *vertbuffer)
{
	if (!gs_valid_p("gs_vertexbuffer_flush", vertbuffer))
		return;

	thread_graphics->exports.gs_vertexbuffer_flush(vertbuffer);
}

struct gs_vb_data *gs_vertexbuffer_get_data(const gs_vertbuffer_t *vertbuffer)
{
	if (!gs_valid_p("gs_vertexbuffer_get_data", vertbuffer))
		return NULL;

	return thread_graphics->exports.gs_vertexbuffer_get_data(vertbuffer);
}

#undef gs_valid_p
