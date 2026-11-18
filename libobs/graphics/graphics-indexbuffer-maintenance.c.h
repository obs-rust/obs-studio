#include "libobs/build/libobs-graphics.pch.h"

/* Reference-only group: use the existing context getter and preserve the
 * original validators and function bodies without a separate TLS instance. */
#define thread_graphics gs_get_context()

static inline bool indexbuffer_maintenance_ref_valid(const char *f)
{
	if (!thread_graphics) {
		blog(LOG_DEBUG, "%s: called while not in a graphics context", f);
		return false;
	}

	return true;
}

static inline bool indexbuffer_maintenance_ref_obj_valid(const void *obj, const char *f, const char *name)
{
	if (!obj) {
		blog(LOG_DEBUG, "%s: Null '%s' parameter", f, name);
		return false;
	}

	return true;
}

#define gs_valid indexbuffer_maintenance_ref_valid
#define gs_obj_valid indexbuffer_maintenance_ref_obj_valid

#define ptr_valid(ptr, func) gs_obj_valid(ptr, func, #ptr)
#define gs_valid_p(func, param1) (gs_valid(func) && ptr_valid(param1, func))
#define gs_valid_p2(func, param1, param2) (gs_valid(func) && ptr_valid(param1, func) && ptr_valid(param2, func))

void gs_indexbuffer_destroy(gs_indexbuffer_t *indexbuffer)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_indexbuffer_destroy"))
		return;
	if (!indexbuffer)
		return;

	graphics->exports.gs_indexbuffer_destroy(indexbuffer);
}

void gs_indexbuffer_flush(gs_indexbuffer_t *indexbuffer)
{
	if (!gs_valid_p("gs_indexbuffer_flush", indexbuffer))
		return;

	thread_graphics->exports.gs_indexbuffer_flush(indexbuffer);
}

void gs_indexbuffer_flush_direct(gs_indexbuffer_t *indexbuffer, const void *data)
{
	if (!gs_valid_p2("gs_indexbuffer_flush_direct", indexbuffer, data))
		return;

	thread_graphics->exports.gs_indexbuffer_flush_direct(indexbuffer, data);
}

size_t gs_indexbuffer_get_num_indices(const gs_indexbuffer_t *indexbuffer)
{
	if (!gs_valid_p("gs_indexbuffer_get_num_indices", indexbuffer))
		return 0;

	return thread_graphics->exports.gs_indexbuffer_get_num_indices(indexbuffer);
}

#undef gs_valid_p2
#undef gs_valid_p
#undef ptr_valid
#undef gs_obj_valid
#undef gs_valid
#undef thread_graphics
