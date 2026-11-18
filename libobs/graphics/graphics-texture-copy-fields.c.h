#include "libobs/build/libobs-graphics.pch.h"

/* Private layout/dispatch only. Rust validates before any field access. */
graphics_t *obs_graphics_texture_copy_context(void);
graphics_t *obs_graphics_texture_copy_context(void)
{
	return gs_get_context();
}

void obs_graphics_texture_copy_debug(const char *function, const char *reason);
void obs_graphics_texture_copy_debug(const char *function, const char *reason)
{
	blog(LOG_DEBUG, "%s: %s", function, reason);
}

void obs_graphics_texture_copy_whole(graphics_t *graphics, gs_texture_t *dst, gs_texture_t *src);
void obs_graphics_texture_copy_whole(graphics_t *graphics, gs_texture_t *dst, gs_texture_t *src)
{
	graphics->exports.device_copy_texture(graphics->device, dst, src);
}

void obs_graphics_texture_copy_region(graphics_t *graphics, gs_texture_t *dst, uint32_t dst_x, uint32_t dst_y,
				      gs_texture_t *src, uint32_t src_x, uint32_t src_y, uint32_t src_w,
				      uint32_t src_h);
void obs_graphics_texture_copy_region(graphics_t *graphics, gs_texture_t *dst, uint32_t dst_x, uint32_t dst_y,
				      gs_texture_t *src, uint32_t src_x, uint32_t src_y, uint32_t src_w, uint32_t src_h)
{
	graphics->exports.device_copy_texture_region(graphics->device, dst, dst_x, dst_y, src, src_x, src_y, src_w,
						     src_h);
}
