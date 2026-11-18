#include "libobs/build/libobs-graphics.pch.h"

/* Reference-only group: use the existing context getter and preserve the
 * original validators and function bodies without a separate TLS instance. */
#define thread_graphics gs_get_context()

static inline bool texture_copy_ref_valid(const char *f)
{
	if (!thread_graphics) {
		blog(LOG_DEBUG, "%s: called while not in a graphics context", f);
		return false;
	}

	return true;
}

static inline bool texture_copy_ref_obj_valid(const void *obj, const char *f, const char *name)
{
	if (!obj) {
		blog(LOG_DEBUG, "%s: Null '%s' parameter", f, name);
		return false;
	}

	return true;
}

#define gs_valid texture_copy_ref_valid
#define gs_obj_valid texture_copy_ref_obj_valid

#define ptr_valid(ptr, func) gs_obj_valid(ptr, func, #ptr)
#define gs_valid_p(func, param1) (gs_valid(func) && ptr_valid(param1, func))
#define gs_valid_p2(func, param1, param2) (gs_valid(func) && ptr_valid(param1, func) && ptr_valid(param2, func))

void gs_copy_texture(gs_texture_t *dst, gs_texture_t *src)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid_p2("gs_copy_texture", dst, src))
		return;

	graphics->exports.device_copy_texture(graphics->device, dst, src);
}

void gs_copy_texture_region(gs_texture_t *dst, uint32_t dst_x, uint32_t dst_y, gs_texture_t *src, uint32_t src_x,
			    uint32_t src_y, uint32_t src_w, uint32_t src_h)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid_p("gs_copy_texture_region", dst))
		return;

	graphics->exports.device_copy_texture_region(graphics->device, dst, dst_x, dst_y, src, src_x, src_y, src_w,
						     src_h);
}

#undef gs_valid_p2
#undef gs_valid_p
#undef ptr_valid

#undef gs_obj_valid
#undef gs_valid
#undef thread_graphics
