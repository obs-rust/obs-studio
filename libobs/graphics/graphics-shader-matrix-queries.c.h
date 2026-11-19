#include "libobs/build/libobs-graphics.pch.h"

/* Reference-only group: use the existing context getter and preserve the
 * original validators and function bodies without a separate TLS instance. */
#define thread_graphics gs_get_context()

static inline bool shader_matrix_ref_valid(const char *f)
{
	if (!thread_graphics) {
		blog(LOG_DEBUG, "%s: called while not in a graphics context", f);
		return false;
	}

	return true;
}

static inline bool shader_matrix_ref_obj_valid(const void *obj, const char *f, const char *name)
{
	if (!obj) {
		blog(LOG_DEBUG, "%s: Null '%s' parameter", f, name);
		return false;
	}

	return true;
}

#define gs_valid shader_matrix_ref_valid
#define gs_obj_valid shader_matrix_ref_obj_valid

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
#undef gs_obj_valid
#undef gs_valid
#undef thread_graphics
