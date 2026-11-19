#include "libobs/build/libobs-graphics.pch.h"

/* Native layout and vtable dispatch only; Rust guards context and shader first. */
graphics_t *obs_graphics_shader_matrix_context(void);
graphics_t *obs_graphics_shader_matrix_context(void)
{
	return gs_get_context();
}
void obs_graphics_shader_matrix_debug(const char *function, const char *reason);
void obs_graphics_shader_matrix_debug(const char *function, const char *reason)
{
	blog(LOG_DEBUG, "%s: %s", function, reason);
}
gs_sparam_t *obs_graphics_shader_matrix_viewproj(graphics_t *graphics, const gs_shader_t *shader);
gs_sparam_t *obs_graphics_shader_matrix_viewproj(graphics_t *graphics, const gs_shader_t *shader)
{
	return graphics->exports.gs_shader_get_viewproj_matrix(shader);
}
gs_sparam_t *obs_graphics_shader_matrix_world(graphics_t *graphics, const gs_shader_t *shader);
gs_sparam_t *obs_graphics_shader_matrix_world(graphics_t *graphics, const gs_shader_t *shader)
{
	return graphics->exports.gs_shader_get_world_matrix(shader);
}
