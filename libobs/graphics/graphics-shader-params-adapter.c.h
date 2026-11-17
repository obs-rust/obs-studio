#include "libobs/build/libobs-graphics.pch.h"

/* Backend-owned layouts stay in C. Rust owns validation and dispatch order. */
graphics_t *obs_graphics_shader_params_context(void);
void obs_graphics_shader_params_log(const char *function, const char *reason);

graphics_t *obs_graphics_shader_params_context(void)
{
	return gs_get_context();
}

void obs_graphics_shader_params_log(const char *function, const char *reason)
{
	blog(LOG_DEBUG, "%s: %s", function, reason);
}

void obs_graphics_shader_params_bool(graphics_t *graphics, gs_sparam_t *param, bool value);
void obs_graphics_shader_params_bool(graphics_t *graphics, gs_sparam_t *param, bool value)
{
	graphics->exports.gs_shader_set_bool(param, value);
}

void obs_graphics_shader_params_float(graphics_t *graphics, gs_sparam_t *param, float value);
void obs_graphics_shader_params_float(graphics_t *graphics, gs_sparam_t *param, float value)
{
	graphics->exports.gs_shader_set_float(param, value);
}

void obs_graphics_shader_params_int(graphics_t *graphics, gs_sparam_t *param, int value);
void obs_graphics_shader_params_int(graphics_t *graphics, gs_sparam_t *param, int value)
{
	graphics->exports.gs_shader_set_int(param, value);
}

void obs_graphics_shader_params_matrix3(graphics_t *graphics, gs_sparam_t *param, const struct matrix3 *value);
void obs_graphics_shader_params_matrix3(graphics_t *graphics, gs_sparam_t *param, const struct matrix3 *value)
{
	graphics->exports.gs_shader_set_matrix3(param, value);
}

void obs_graphics_shader_params_matrix4(graphics_t *graphics, gs_sparam_t *param, const struct matrix4 *value);
void obs_graphics_shader_params_matrix4(graphics_t *graphics, gs_sparam_t *param, const struct matrix4 *value)
{
	graphics->exports.gs_shader_set_matrix4(param, value);
}

void obs_graphics_shader_params_vec2(graphics_t *graphics, gs_sparam_t *param, const struct vec2 *value);
void obs_graphics_shader_params_vec2(graphics_t *graphics, gs_sparam_t *param, const struct vec2 *value)
{
	graphics->exports.gs_shader_set_vec2(param, value);
}

void obs_graphics_shader_params_vec3(graphics_t *graphics, gs_sparam_t *param, const struct vec3 *value);
void obs_graphics_shader_params_vec3(graphics_t *graphics, gs_sparam_t *param, const struct vec3 *value)
{
	graphics->exports.gs_shader_set_vec3(param, value);
}

void obs_graphics_shader_params_vec4(graphics_t *graphics, gs_sparam_t *param, const struct vec4 *value);
void obs_graphics_shader_params_vec4(graphics_t *graphics, gs_sparam_t *param, const struct vec4 *value)
{
	graphics->exports.gs_shader_set_vec4(param, value);
}
