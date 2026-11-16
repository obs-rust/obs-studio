#include "libobs/build/libobs-graphics.pch.h"

/* C reference keeps the original validation behavior outside graphics.c.h. */
#define thread_graphics gs_get_context()
static inline bool stagesurface_ref_obj_valid(const void *obj, const char *f, const char *name)
{
	if (!obj) {
		blog(LOG_DEBUG, "%s: Null '%s' parameter", f, name);
		return false;
	}
	return true;
}
static inline bool stagesurface_ref_valid(const char *f)
{
	if (!thread_graphics) {
		blog(LOG_DEBUG, "%s: called while not in a graphics context", f);
		return false;
	}
	return true;
}
#define gs_valid_p(func, param1) (stagesurface_ref_valid(func) && stagesurface_ref_obj_valid(param1, func, #param1))

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

#undef thread_graphics
