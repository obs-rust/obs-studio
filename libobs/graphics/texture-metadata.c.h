#include "libobs/build/libobs-graphics.pch.h"

/* Same unity group as graphics.c.h: retain its validation helpers verbatim. */
#define ptr_valid(ptr, func) gs_obj_valid(ptr, func, #ptr)
#define gs_valid_p(func, param1) (gs_valid(func) && ptr_valid(param1, func))

uint32_t gs_texture_get_width(const gs_texture_t *tex)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid_p("gs_texture_get_width", tex))
		return 0;

	return graphics->exports.gs_texture_get_width(tex);
}

uint32_t gs_texture_get_height(const gs_texture_t *tex)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid_p("gs_texture_get_height", tex))
		return 0;

	return graphics->exports.gs_texture_get_height(tex);
}

enum gs_color_format gs_texture_get_color_format(const gs_texture_t *tex)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid_p("gs_texture_get_color_format", tex))
		return GS_UNKNOWN;

	return graphics->exports.gs_texture_get_color_format(tex);
}

#undef gs_valid_p
#undef ptr_valid
