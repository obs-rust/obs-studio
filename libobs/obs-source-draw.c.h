#include "libobs/build/libobs.pch.h"

void obs_source_draw_set_color_matrix(const struct matrix4 *color_matrix, const struct vec3 *color_range_min,
				      const struct vec3 *color_range_max)
{
	struct vec3 color_range_min_def;
	struct vec3 color_range_max_def;

	vec3_set(&color_range_min_def, 0.0f, 0.0f, 0.0f);
	vec3_set(&color_range_max_def, 1.0f, 1.0f, 1.0f);

	gs_effect_t *effect = gs_get_effect();
	gs_eparam_t *matrix;
	gs_eparam_t *range_min;
	gs_eparam_t *range_max;

	if (!effect) {
		blog(LOG_WARNING, "obs_source_draw_set_color_matrix: no "
				  "active effect!");
		return;
	}

	if (!obs_ptr_valid(color_matrix, "obs_source_draw_set_color_matrix"))
		return;

	if (!color_range_min)
		color_range_min = &color_range_min_def;
	if (!color_range_max)
		color_range_max = &color_range_max_def;

	matrix = gs_effect_get_param_by_name(effect, "color_matrix");
	range_min = gs_effect_get_param_by_name(effect, "color_range_min");
	range_max = gs_effect_get_param_by_name(effect, "color_range_max");

	gs_effect_set_matrix4(matrix, color_matrix);
	gs_effect_set_val(range_min, color_range_min, sizeof(float) * 3);
	gs_effect_set_val(range_max, color_range_max, sizeof(float) * 3);
}

void obs_source_draw(gs_texture_t *texture, int x, int y, uint32_t cx, uint32_t cy, bool flip)
{
	if (!obs_ptr_valid(texture, "obs_source_draw"))
		return;

	gs_effect_t *effect = gs_get_effect();
	if (!effect) {
		blog(LOG_WARNING, "obs_source_draw: no active effect!");
		return;
	}

	const bool linear_srgb = gs_get_linear_srgb();

	const bool previous = gs_framebuffer_srgb_enabled();
	gs_enable_framebuffer_srgb(linear_srgb);

	gs_eparam_t *image = gs_effect_get_param_by_name(effect, "image");
	if (linear_srgb)
		gs_effect_set_texture_srgb(image, texture);
	else
		gs_effect_set_texture(image, texture);

	const bool change_pos = (x != 0 || y != 0);
	if (change_pos) {
		gs_matrix_push();
		gs_matrix_translate3f((float)x, (float)y, 0.0f);
	}

	gs_draw_sprite(texture, flip ? GS_FLIP_V : 0, cx, cy);

	if (change_pos)
		gs_matrix_pop();

	gs_enable_framebuffer_srgb(previous);
}
