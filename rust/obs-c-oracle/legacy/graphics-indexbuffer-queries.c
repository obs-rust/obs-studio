#include "libobs/build/libobs-graphics.pch.h"

/* Private dependencies defined in graphics.c.h, later in the unity group. */
static THREAD_LOCAL graphics_t *thread_graphics;
static inline bool gs_obj_valid(const void *obj, const char *f, const char *name);
static inline bool gs_valid(const char *f);

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
