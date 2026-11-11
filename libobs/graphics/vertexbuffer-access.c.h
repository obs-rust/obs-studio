#include "libobs/build/libobs-graphics.pch.h"

/* The reference group is independent of graphics.c.h's private TLS variable.
 * gs_get_context returns that same thread-local context, without retaining it. */
static bool vertexbuffer_access_valid(const char *function)
{
	if (!gs_get_context()) {
		blog(LOG_DEBUG, "%s: called while not in a graphics context", function);
		return false;
	}
	return true;
}

static bool vertexbuffer_access_obj_valid(const void *object, const char *function, const char *name)
{
	if (!object) {
		blog(LOG_DEBUG, "%s: Null '%s' parameter", function, name);
		return false;
	}
	return true;
}

#define gs_valid vertexbuffer_access_valid
#define gs_obj_valid vertexbuffer_access_obj_valid
#define thread_graphics gs_get_context()
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
#undef thread_graphics
#undef gs_obj_valid
#undef gs_valid
