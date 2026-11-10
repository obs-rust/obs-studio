#include "libobs/build/libobs.pch.h"

/* Private layout access only. Validation and dispatch remain in Rust. */
graphics_t *obs_texture_metadata_context(void);
graphics_t *obs_texture_metadata_context(void)
{
	return gs_get_context();
}
void obs_texture_metadata_debug(const char *function, const char *reason);
void obs_texture_metadata_debug(const char *function, const char *reason)
{
	blog(LOG_DEBUG, "%s: %s", function, reason);
}
uint32_t (*obs_texture_metadata_width_field(graphics_t *graphics))(const gs_texture_t *);
uint32_t (*obs_texture_metadata_width_field(graphics_t *graphics))(const gs_texture_t *)
{
	return graphics->exports.gs_texture_get_width;
}
uint32_t (*obs_texture_metadata_height_field(graphics_t *graphics))(const gs_texture_t *);
uint32_t (*obs_texture_metadata_height_field(graphics_t *graphics))(const gs_texture_t *)
{
	return graphics->exports.gs_texture_get_height;
}
enum gs_color_format (*obs_texture_metadata_format_field(graphics_t *graphics))(const gs_texture_t *);
enum gs_color_format (*obs_texture_metadata_format_field(graphics_t *graphics))(const gs_texture_t *)
{
	return graphics->exports.gs_texture_get_color_format;
}
