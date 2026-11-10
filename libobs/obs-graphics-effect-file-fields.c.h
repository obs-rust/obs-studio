#include "libobs/build/libobs-graphics.pch.h"

/* Private collaborators for the Rust wrapper. Keep allocator and file calls in
 * libobs when a consumer links the Rust archive beside the shared library. */
bool obs_graphics_effect_file_context(void);
gs_effect_t *obs_graphics_effect_file_first(void);
char *obs_graphics_effect_file_read(const char *file);
gs_effect_t *obs_graphics_effect_file_create(const char *text, const char *file, char **error);
void obs_graphics_effect_file_free(void *text);
void obs_graphics_effect_file_log(int which, const char *file);

bool obs_graphics_effect_file_context(void)
{
	return gs_get_context() != NULL;
}
gs_effect_t *obs_graphics_effect_file_first(void)
{
	return gs_get_context()->first_effect;
}
char *obs_graphics_effect_file_read(const char *file)
{
	return os_quick_read_utf8_file(file);
}
gs_effect_t *obs_graphics_effect_file_create(const char *text, const char *file, char **error)
{
	return gs_effect_create(text, file, error);
}
void obs_graphics_effect_file_free(void *text)
{
	bfree(text);
}
void obs_graphics_effect_file_log(int which, const char *file)
{
	switch (which) {
	case 0:
		blog(LOG_DEBUG, "%s: called while not in a graphics context", "gs_effect_create_from_file");
		break;
	case 1:
		blog(LOG_DEBUG, "%s: Null '%s' parameter", "gs_effect_create_from_file", "file");
		break;
	case 2:
		blog(LOG_ERROR, "Could not load effect file '%s'", file);
		break;
	}
}
