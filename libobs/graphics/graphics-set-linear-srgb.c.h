#include "libobs/build/libobs-graphics.pch.h"

extern bool *obs_graphics_linear_srgb_field(const char *function);

#define thread_graphics gs_get_context()
#define gs_valid(function) (obs_graphics_linear_srgb_field(function) != NULL)

bool gs_set_linear_srgb(bool linear_srgb)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_set_linear_srgb"))
		return false;

	const bool previous = graphics->linear_srgb;
	graphics->linear_srgb = linear_srgb;
	return previous;
}


#undef gs_valid
#undef thread_graphics
