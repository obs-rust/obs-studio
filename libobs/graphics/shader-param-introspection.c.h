#include "libobs/build/libobs-graphics.pch.h"

/* Original context and pointer validation, scoped to this extracted unit. */
#define thread_graphics gs_get_context()
static inline bool shader_param_ref_obj_valid(const void *obj, const char *f, const char *name)
{
	if (!obj) {
		blog(LOG_DEBUG, "%s: Null '%s' parameter", f, name);
		return false;
	}

	return true;
}

static inline bool shader_param_ref_valid(const char *f)
{
	if (!thread_graphics) {
		blog(LOG_DEBUG, "%s: called while not in a graphics context", f);
		return false;
	}

	return true;
}

#define ptr_valid(ptr, func) shader_param_ref_obj_valid(ptr, func, #ptr)
#define gs_valid_p(func, param1) (shader_param_ref_valid(func) && ptr_valid(param1, func))
#define gs_valid_p2(func, param1, param2) (shader_param_ref_valid(func) && ptr_valid(param1, func) && ptr_valid(param2, func))

int gs_shader_get_num_params(const gs_shader_t *shader)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid_p("gs_shader_get_num_params", shader))
		return 0;

	return graphics->exports.gs_shader_get_num_params(shader);
}

gs_sparam_t *gs_shader_get_param_by_idx(gs_shader_t *shader, uint32_t param)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid_p("gs_shader_get_param_by_idx", shader))
		return NULL;

	return graphics->exports.gs_shader_get_param_by_idx(shader, param);
}

gs_sparam_t *gs_shader_get_param_by_name(gs_shader_t *shader, const char *name)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid_p2("gs_shader_get_param_by_name", shader, name))
		return NULL;

	return graphics->exports.gs_shader_get_param_by_name(shader, name);
}

void gs_shader_get_param_info(const gs_sparam_t *param, struct gs_shader_param_info *info)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid_p2("gs_shader_get_param_info", param, info))
		return;

	graphics->exports.gs_shader_get_param_info(param, info);
}

#undef gs_valid_p2
#undef gs_valid_p
#undef ptr_valid
#undef thread_graphics
