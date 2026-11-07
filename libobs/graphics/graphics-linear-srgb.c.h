#include "libobs/build/libobs-ported-graphics.pch.h"

extern bool *obs_graphics_linear_srgb_field(const char *function);

/* Reach the original context and validation across the extraction boundary. */
#define thread_graphics gs_get_context()
#define gs_valid(function) (obs_graphics_linear_srgb_field(function) != NULL)

bool gs_get_linear_srgb(void)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_get_linear_srgb"))
		return false;

	return graphics->linear_srgb;
}

#undef gs_valid
#undef thread_graphics
