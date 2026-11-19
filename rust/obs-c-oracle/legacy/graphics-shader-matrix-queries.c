#include "libobs/build/libobs-graphics.pch.h"

/* Private dependencies defined in graphics.c.h, later in the unity group. */
static THREAD_LOCAL graphics_t *thread_graphics;
static inline bool gs_valid(const char *f);
static inline bool gs_obj_valid(const void *obj, const char *f, const char *name);

#define gs_valid_p(func, param1) (gs_valid(func) && gs_obj_valid(param1, func, #param1))

gs_sparam_t *gs_shader_get_viewproj_matrix(const gs_shader_t *shader)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid_p("gs_shader_get_viewproj_matrix", shader))
		return NULL;

	return graphics->exports.gs_shader_get_viewproj_matrix(shader);
}

gs_sparam_t *gs_shader_get_world_matrix(const gs_shader_t *shader)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid_p("gs_shader_get_world_matrix", shader))
		return NULL;

	return graphics->exports.gs_shader_get_world_matrix(shader);
}

#undef gs_valid_p
