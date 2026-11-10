#include "libobs/build/libobs-graphics.pch.h"

/* Same unity group as graphics.c.h: retain its validation helpers verbatim. */
#define ptr_valid(ptr, func) gs_obj_valid(ptr, func, #ptr)
#define gs_valid_p(func, param1) (gs_valid(func) && ptr_valid(param1, func))

void gs_cubetexture_destroy(gs_texture_t *cubetex)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_cubetexture_destroy"))
		return;
	if (!cubetex)
		return;

	graphics->exports.gs_cubetexture_destroy(cubetex);
}

uint32_t gs_cubetexture_get_size(const gs_texture_t *cubetex)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid_p("gs_cubetexture_get_size", cubetex))
		return 0;

	return graphics->exports.gs_cubetexture_get_size(cubetex);
}

enum gs_color_format gs_cubetexture_get_color_format(const gs_texture_t *cubetex)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid_p("gs_cubetexture_get_color_format", cubetex))
		return GS_UNKNOWN;

	return graphics->exports.gs_cubetexture_get_color_format(cubetex);
}

#undef gs_valid_p
#undef ptr_valid
