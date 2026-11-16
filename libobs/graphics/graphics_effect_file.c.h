#include "libobs/build/libobs-graphics.pch.h"

static inline struct gs_effect *find_cached_effect(const char *filename)
{
	struct gs_effect *effect = gs_get_context()->first_effect;

	while (effect) {
		if (strcmp(effect->effect_path, filename) == 0)
			break;
		effect = effect->next;
	}

	return effect;
}

gs_effect_t *gs_effect_create_from_file(const char *file, char **error_string)
{
	char *file_string;
	gs_effect_t *effect = NULL;

	if (!gs_get_context()) {
		blog(LOG_DEBUG, "%s: called while not in a graphics context", "gs_effect_create_from_file");
		return NULL;
	}
	if (!file) {
		blog(LOG_DEBUG, "%s: Null '%s' parameter", "gs_effect_create_from_file", "file");
		return NULL;
	}

	effect = find_cached_effect(file);
	if (effect)
		return effect;

	file_string = os_quick_read_utf8_file(file);
	if (!file_string) {
		blog(LOG_ERROR, "Could not load effect file '%s'", file);
		return NULL;
	}

	effect = gs_effect_create(file_string, file, error_string);
	bfree(file_string);

	return effect;
}
