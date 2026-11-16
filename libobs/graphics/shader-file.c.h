#include "libobs/build/libobs-graphics.pch.h"

/* The validators are private to the graphics unity group. */
#define ptr_valid(ptr, func) shader_file_obj_valid(ptr, func, #ptr)
#define gs_valid_p(func, param1) (shader_file_valid(func) && ptr_valid(param1, func))

gs_shader_t *gs_vertexshader_create_from_file(const char *file, char **error_string)
{
	if (!gs_valid_p("gs_vertexshader_create_from_file", file))
		return NULL;

	char *file_string;
	gs_shader_t *shader = NULL;

	file_string = os_quick_read_utf8_file(file);
	if (!file_string) {
		blog(LOG_ERROR, "Could not load vertex shader file '%s'", file);
		return NULL;
	}

	shader = gs_vertexshader_create(file_string, file, error_string);
	bfree(file_string);

	return shader;
}

gs_shader_t *gs_pixelshader_create_from_file(const char *file, char **error_string)
{
	char *file_string;
	gs_shader_t *shader = NULL;

	if (!gs_valid_p("gs_pixelshader_create_from_file", file))
		return NULL;

	file_string = os_quick_read_utf8_file(file);
	if (!file_string) {
		blog(LOG_ERROR, "Could not load pixel shader file '%s'", file);
		return NULL;
	}

	shader = gs_pixelshader_create(file_string, file, error_string);
	bfree(file_string);

	return shader;
}

#undef gs_valid_p
#undef ptr_valid
