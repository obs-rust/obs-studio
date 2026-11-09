#include "libobs/build/libobs-graphics.pch.h"

/* This unit follows graphics.c.h in the graphics unity group, retaining its
 * private context and validation helpers without exporting a new C API.
 * graphics.c.h retires its macros before the next unity member. */
#define gs_valid_p(function, param) (gs_valid(function) && gs_obj_valid(param, function, #param))

static inline struct gs_effect *find_cached_effect(const char *filename)
{
	struct gs_effect *effect = thread_graphics->first_effect;

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

	if (!gs_valid_p("gs_effect_create_from_file", file))
		return NULL;

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

#undef gs_valid_p
