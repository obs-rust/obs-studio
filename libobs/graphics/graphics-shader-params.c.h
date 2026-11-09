#include "libobs/build/libobs-graphics.pch.h"

/* Retain the original validators in graphics.c.h. The public getter reads
 * the same thread-local context without exposing its storage to this unit. */
static inline bool gs_valid(const char *f);
static inline bool gs_obj_valid(const void *obj, const char *f, const char *name);

#define thread_graphics gs_get_context()
#define gs_valid_p(func, param1) (gs_valid(func) && gs_obj_valid(param1, func, #param1))
#define gs_valid_p2(func, param1, param2) \
	(gs_valid(func) && gs_obj_valid(param1, func, #param1) && gs_obj_valid(param2, func, #param2))

void gs_shader_set_bool(gs_sparam_t *param, bool val)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid_p("gs_shader_set_bool", param))
		return;

	graphics->exports.gs_shader_set_bool(param, val);
}

void gs_shader_set_float(gs_sparam_t *param, float val)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid_p("gs_shader_set_float", param))
		return;

	graphics->exports.gs_shader_set_float(param, val);
}

void gs_shader_set_int(gs_sparam_t *param, int val)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid_p("gs_shader_set_int", param))
		return;

	graphics->exports.gs_shader_set_int(param, val);
}

void gs_shader_set_matrix3(gs_sparam_t *param, const struct matrix3 *val)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid_p2("gs_shader_set_matrix3", param, val))
		return;

	graphics->exports.gs_shader_set_matrix3(param, val);
}

void gs_shader_set_matrix4(gs_sparam_t *param, const struct matrix4 *val)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid_p2("gs_shader_set_matrix4", param, val))
		return;

	graphics->exports.gs_shader_set_matrix4(param, val);
}

void gs_shader_set_vec2(gs_sparam_t *param, const struct vec2 *val)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid_p2("gs_shader_set_vec2", param, val))
		return;

	graphics->exports.gs_shader_set_vec2(param, val);
}

void gs_shader_set_vec3(gs_sparam_t *param, const struct vec3 *val)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid_p2("gs_shader_set_vec3", param, val))
		return;

	graphics->exports.gs_shader_set_vec3(param, val);
}

void gs_shader_set_vec4(gs_sparam_t *param, const struct vec4 *val)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid_p2("gs_shader_set_vec4", param, val))
		return;

	graphics->exports.gs_shader_set_vec4(param, val);
}

#undef gs_valid_p2
#undef gs_valid_p
#undef thread_graphics
