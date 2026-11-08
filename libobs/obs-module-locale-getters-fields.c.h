#include "libobs/build/libobs.pch.h"

typedef bool (*obs_module_locale_getters_callback_t)(const char *, const char **);
obs_module_locale_getters_callback_t const *obs_module_locale_getters_callback_field(const obs_module_t *module);
obs_module_locale_getters_callback_t const *obs_module_locale_getters_callback_field(const obs_module_t *module)
{
	return module ? &module->get_string : NULL;
}
