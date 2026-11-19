#include "libobs/build/libobs-graphics.pch.h"

/* Reference-only validators; keep the original function bodies below. */
#define thread_graphics gs_get_context()

static inline bool indexbuffer_queries_ref_valid(const char *f)
{
	if (!thread_graphics) {
		blog(LOG_DEBUG, "%s: called while not in a graphics context", f);
		return false;
	}

	return true;
}

static inline bool indexbuffer_queries_ref_obj_valid(const void *obj, const char *f, const char *name)
{
	if (!obj) {
		blog(LOG_DEBUG, "%s: Null '%s' parameter", f, name);
		return false;
	}

	return true;
}

#define gs_valid indexbuffer_queries_ref_valid
#define gs_obj_valid indexbuffer_queries_ref_obj_valid

#define ptr_valid(ptr, func) gs_obj_valid(ptr, func, #ptr)
#define gs_valid_p(func, param1) (gs_valid(func) && ptr_valid(param1, func))

void *gs_indexbuffer_get_data(const gs_indexbuffer_t *indexbuffer)
{
	if (!gs_valid_p("gs_indexbuffer_get_data", indexbuffer))
		return NULL;

	return thread_graphics->exports.gs_indexbuffer_get_data(indexbuffer);
}

enum gs_index_type gs_indexbuffer_get_type(const gs_indexbuffer_t *indexbuffer)
{
	if (!gs_valid_p("gs_indexbuffer_get_type", indexbuffer))
		return (enum gs_index_type)0;

	return thread_graphics->exports.gs_indexbuffer_get_type(indexbuffer);
}

#undef gs_valid_p
#undef ptr_valid
#undef gs_obj_valid
#undef gs_valid
#undef thread_graphics
