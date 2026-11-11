#include "libobs/build/libobs.pch.h"

/* Private layout access only. Validation and dispatch remain in Rust. */
graphics_t *obs_shader_param_introspection_context(void);
graphics_t *obs_shader_param_introspection_context(void)
{
	return gs_get_context();
}
void obs_shader_param_introspection_debug(const char *function, const char *reason);
void obs_shader_param_introspection_debug(const char *function, const char *reason)
{
	blog(LOG_DEBUG, "%s: %s", function, reason);
}
int (*obs_shader_param_introspection_count_field(graphics_t *graphics))(const gs_shader_t *);
int (*obs_shader_param_introspection_count_field(graphics_t *graphics))(const gs_shader_t *)
{
	return graphics->exports.gs_shader_get_num_params;
}
gs_sparam_t *(*obs_shader_param_introspection_index_field(graphics_t *graphics))(gs_shader_t *, uint32_t);
gs_sparam_t *(*obs_shader_param_introspection_index_field(graphics_t *graphics))(gs_shader_t *, uint32_t)
{
	return graphics->exports.gs_shader_get_param_by_idx;
}
gs_sparam_t *(*obs_shader_param_introspection_name_field(graphics_t *graphics))(gs_shader_t *, const char *);
gs_sparam_t *(*obs_shader_param_introspection_name_field(graphics_t *graphics))(gs_shader_t *, const char *)
{
	return graphics->exports.gs_shader_get_param_by_name;
}
void (*obs_shader_param_introspection_info_field(graphics_t *graphics))(const gs_sparam_t *,
									struct gs_shader_param_info *);
void (*obs_shader_param_introspection_info_field(graphics_t *graphics))(const gs_sparam_t *,
									struct gs_shader_param_info *)
{
	return graphics->exports.gs_shader_get_param_info;
}
