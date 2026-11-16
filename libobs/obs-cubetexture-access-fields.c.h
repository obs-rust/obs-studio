#include "libobs/build/libobs.pch.h"

/* Private layout access only. Validation and dispatch remain in Rust. */
graphics_t *obs_cubetexture_access_context(void);
graphics_t *obs_cubetexture_access_context(void)
{
	return gs_get_context();
}
void obs_cubetexture_access_debug(const char *function, const char *reason);
void obs_cubetexture_access_debug(const char *function, const char *reason)
{
	blog(LOG_DEBUG, "%s: %s", function, reason);
}
uint32_t (*obs_cubetexture_access_size_field(graphics_t *graphics))(const gs_texture_t *);
uint32_t (*obs_cubetexture_access_size_field(graphics_t *graphics))(const gs_texture_t *)
{
	return graphics->exports.gs_cubetexture_get_size;
}
void (*obs_cubetexture_access_destroy_field(graphics_t *graphics))(gs_texture_t *);
void (*obs_cubetexture_access_destroy_field(graphics_t *graphics))(gs_texture_t *)
{
	return graphics->exports.gs_cubetexture_destroy;
}
enum gs_color_format (*obs_cubetexture_access_format_field(graphics_t *graphics))(const gs_texture_t *);
enum gs_color_format (*obs_cubetexture_access_format_field(graphics_t *graphics))(const gs_texture_t *)
{
	return graphics->exports.gs_cubetexture_get_color_format;
}
