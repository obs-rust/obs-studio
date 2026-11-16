#include "libobs/build/libobs-graphics.pch.h"

/* Original validation helpers, retained for the standalone C reference. */
#define thread_graphics gs_get_context()
static inline bool cube_access_ref_obj_valid(const void *obj, const char *f, const char *name)
{
	if (!obj) {
		blog(LOG_DEBUG, "%s: Null '%s' parameter", f, name);
		return false;
	}

	return true;
}

static inline bool cube_access_ref_valid(const char *f)
{
	if (!thread_graphics) {
		blog(LOG_DEBUG, "%s: called while not in a graphics context", f);
		return false;
	}

	return true;
}

#define ptr_valid(ptr, func) cube_access_ref_obj_valid(ptr, func, #ptr)
#define gs_valid cube_access_ref_valid
#define gs_valid_p(func, param1) (cube_access_ref_valid(func) && ptr_valid(param1, func))

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

#undef gs_valid
#undef thread_graphics
