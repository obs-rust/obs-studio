#include "libobs/build/libobs-graphics.pch.h"

/* Same unity group as graphics.c.h: retain its validation helpers verbatim. */
#define ptr_valid(ptr, func) gs_obj_valid(ptr, func, #ptr)
#define gs_valid_p(func, param1) (gs_valid(func) && ptr_valid(param1, func))

uint32_t gs_voltexture_get_width(const gs_texture_t *voltex)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid_p("gs_voltexture_get_width", voltex))
		return 0;

	return graphics->exports.gs_voltexture_get_width(voltex);
}

uint32_t gs_voltexture_get_height(const gs_texture_t *voltex)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid_p("gs_voltexture_get_height", voltex))
		return 0;

	return graphics->exports.gs_voltexture_get_height(voltex);
}

uint32_t gs_voltexture_get_depth(const gs_texture_t *voltex)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid_p("gs_voltexture_get_depth", voltex))
		return 0;

	return graphics->exports.gs_voltexture_get_depth(voltex);
}

enum gs_color_format gs_voltexture_get_color_format(const gs_texture_t *voltex)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid_p("gs_voltexture_get_color_format", voltex))
		return GS_UNKNOWN;

	return graphics->exports.gs_voltexture_get_color_format(voltex);
}

#undef gs_valid_p
#undef ptr_valid
