#include "libobs/build/libobs-graphics.pch.h"

/* Same unity group as graphics.c.h: retain its validation helpers verbatim. */
#define ptr_valid(ptr, func) gs_obj_valid(ptr, func, #ptr)
#define gs_valid_p2(func, param1, param2) (gs_valid(func) && ptr_valid(param1, func) && ptr_valid(param2, func))

void gs_timer_range_destroy(gs_timer_range_t *range)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_timer_range_destroy"))
		return;
	if (!range)
		return;

	graphics->exports.gs_timer_range_destroy(range);
}

void gs_timer_range_begin(gs_timer_range_t *range)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_timer_range_begin"))
		return;
	if (!range)
		return;

	graphics->exports.gs_timer_range_begin(range);
}

void gs_timer_range_end(gs_timer_range_t *range)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_timer_range_end"))
		return;
	if (!range)
		return;

	graphics->exports.gs_timer_range_end(range);
}

bool gs_timer_range_get_data(gs_timer_range_t *range, bool *disjoint, uint64_t *frequency)
{
	if (!gs_valid_p2("gs_timer_range_get_data", disjoint, frequency))
		return false;

	return thread_graphics->exports.gs_timer_range_get_data(range, disjoint, frequency);
}

#undef gs_valid_p2
#undef ptr_valid
