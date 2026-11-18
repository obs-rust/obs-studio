#include "libobs/build/libobs-graphics.pch.h"

/* Private dependencies defined in graphics.c.h, later in the unity group. */
static THREAD_LOCAL graphics_t *thread_graphics;
static inline bool gs_valid(const char *f);
static inline bool gs_obj_valid(const void *obj, const char *f, const char *name);

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
