#include "libobs/build/libobs-graphics.pch.h"

/* Same unity group as graphics.c.h: retain its validation helpers verbatim. */
#define ptr_valid(ptr, func) gs_obj_valid(ptr, func, #ptr)
#define gs_valid_p2(func, param1, param2) (gs_valid(func) && ptr_valid(param1, func) && ptr_valid(param2, func))

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
