#include "libobs/build/libobs-ported-graphics.pch.h"

/* Private layout and the original validation log stay in C. Standalone
 * Rust-static consumers compile this same adapter beside their other private
 * graphics adapters, so neither a layout mirror nor a Rust blog call is needed. */
bool *obs_graphics_linear_srgb_field(const char *function);
bool *obs_graphics_linear_srgb_field(const char *function)
{
	graphics_t *graphics = gs_get_context();
	if (!graphics) {
		blog(LOG_DEBUG, "%s: called while not in a graphics context", function);
		return NULL;
	}
	return &graphics->linear_srgb;
}

/* Private layout access only. Validation and dispatch remain in Rust. */
graphics_t *obs_stagesurface_context(void);
graphics_t *obs_stagesurface_context(void)
{
	return gs_get_context();
}
void obs_stagesurface_debug(const char *function, const char *reason);
void obs_stagesurface_debug(const char *function, const char *reason)
{
	blog(LOG_DEBUG, "%s: %s", function, reason);
}
uint32_t (*obs_stagesurface_width_field(graphics_t *graphics))(const gs_stagesurf_t *);
uint32_t (*obs_stagesurface_width_field(graphics_t *graphics))(const gs_stagesurf_t *)
{
	return graphics->exports.gs_stagesurface_get_width;
}
uint32_t (*obs_stagesurface_height_field(graphics_t *graphics))(const gs_stagesurf_t *);
uint32_t (*obs_stagesurface_height_field(graphics_t *graphics))(const gs_stagesurf_t *)
{
	return graphics->exports.gs_stagesurface_get_height;
}
enum gs_color_format (*obs_stagesurface_format_field(graphics_t *graphics))(const gs_stagesurf_t *);
enum gs_color_format (*obs_stagesurface_format_field(graphics_t *graphics))(const gs_stagesurf_t *)
{
	return graphics->exports.gs_stagesurface_get_color_format;
}

/* The planar texture creators (texture-planar-creators.c.h): the context, its
 * device and its two planar exports, and the three log lines with the C's
 * literal formats (gs_valid()'s and the odd-size errors). */
typedef bool (*obs_texture_planar_create_t)(gs_device_t *device, gs_texture_t **tex_y, gs_texture_t **tex_uv,
					    uint32_t width, uint32_t height, uint32_t flags);
graphics_t *obs_texture_planar_context(void);
graphics_t *obs_texture_planar_context(void)
{
	return gs_get_context();
}
void obs_texture_planar_log(int which, const char *function);
void obs_texture_planar_log(int which, const char *function)
{
	switch (which) {
	case 0:
		blog(LOG_DEBUG, "%s: called while not in a graphics context", function);
		break;
	case 1:
		blog(LOG_ERROR, "NV12 textures must have dimensions "
				"divisible by 2.");
		break;
	case 2:
		blog(LOG_ERROR, "P010 textures must have dimensions "
				"divisible by 2.");
		break;
	}
}
gs_device_t *obs_texture_planar_device(graphics_t *graphics);
gs_device_t *obs_texture_planar_device(graphics_t *graphics)
{
	return graphics->device;
}
obs_texture_planar_create_t obs_texture_planar_nv12_field(graphics_t *graphics);
obs_texture_planar_create_t obs_texture_planar_nv12_field(graphics_t *graphics)
{
	return graphics->exports.device_texture_create_nv12;
}
obs_texture_planar_create_t obs_texture_planar_p010_field(graphics_t *graphics);
obs_texture_planar_create_t obs_texture_planar_p010_field(graphics_t *graphics)
{
	return graphics->exports.device_texture_create_p010;
}
