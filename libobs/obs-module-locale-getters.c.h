#include "libobs/build/libobs.pch.h"

bool obs_module_get_locale_string(const obs_module_t *mod, const char *lookup_string, const char **translated_string)
{
	assert(mod);
	if (mod && mod->get_string) {
		return mod->get_string(lookup_string, translated_string);
	}

	return false;
}

const char *obs_module_get_locale_text(const obs_module_t *mod, const char *text)
{
	const char *str = text;
	obs_module_get_locale_string(mod, text, &str);
	return str;
}
