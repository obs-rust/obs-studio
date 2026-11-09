#include "libobs/build/libobs-graphics.pch.h"

/* Follow graphics.c.h in the graphics unity group: retain its private context
 * and validation helpers until this unit is replaced by Rust. */
#define gs_valid_p(func, param1) (gs_valid(func) && gs_obj_valid(param1, func, #param1))

uint32_t gs_stagesurface_get_width(const gs_stagesurf_t *stagesurf)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid_p("gs_stagesurface_get_width", stagesurf))
		return 0;

	return graphics->exports.gs_stagesurface_get_width(stagesurf);
}

uint32_t gs_stagesurface_get_height(const gs_stagesurf_t *stagesurf)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid_p("gs_stagesurface_get_height", stagesurf))
		return 0;

	return graphics->exports.gs_stagesurface_get_height(stagesurf);
}

enum gs_color_format gs_stagesurface_get_color_format(const gs_stagesurf_t *stagesurf)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid_p("gs_stagesurface_get_color_format", stagesurf))
		return GS_UNKNOWN;

	return graphics->exports.gs_stagesurface_get_color_format(stagesurf);
}

#undef gs_valid_p
