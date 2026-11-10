#include "libobs/build/libobs-graphics.pch.h"

/* Original validation helpers, isolated for the C reference group. */
#define thread_graphics gs_get_context()
static inline bool timer_ref_obj_valid(const void *obj, const char *f, const char *name)
{
	if (!obj) {
		blog(LOG_DEBUG, "%s: Null '%s' parameter", f, name);
		return false;
	}

	return true;
}

static inline bool timer_ref_valid(const char *f)
{
	if (!thread_graphics) {
		blog(LOG_DEBUG, "%s: called while not in a graphics context", f);
		return false;
	}

	return true;
}

#define gs_valid timer_ref_valid
#define ptr_valid(ptr, func) timer_ref_obj_valid(ptr, func, #ptr)
#define gs_valid_p2(func, param1, param2) (timer_ref_valid(func) && ptr_valid(param1, func) && ptr_valid(param2, func))

void gs_timer_destroy(gs_timer_t *timer)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_timer_destroy"))
		return;
	if (!timer)
		return;

	graphics->exports.gs_timer_destroy(timer);
}

void gs_timer_begin(gs_timer_t *timer)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_timer_begin"))
		return;
	if (!timer)
		return;

	graphics->exports.gs_timer_begin(timer);
}

void gs_timer_end(gs_timer_t *timer)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_timer_end"))
		return;
	if (!timer)
		return;

	graphics->exports.gs_timer_end(timer);
}

bool gs_timer_get_data(gs_timer_t *timer, uint64_t *ticks)
{
	if (!gs_valid_p2("gs_timer_get_data", timer, ticks))
		return false;

	return thread_graphics->exports.gs_timer_get_data(timer, ticks);
}

#undef gs_valid_p2
#undef ptr_valid
#undef gs_valid

#undef thread_graphics
