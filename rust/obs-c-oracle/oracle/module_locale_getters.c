/* Original C dispatches only caller-supplied callbacks, never production functions. */
#define obs_module_get_locale_string oracle_obs_module_get_locale_string
#define obs_module_get_locale_text oracle_obs_module_get_locale_text
#include "libobs/obs-module-locale-getters.c.h"
#undef obs_module_get_locale_string
#undef obs_module_get_locale_text
#include "module-locale-getters-fields.inc"
#include <stdlib.h>

void *oracle_module_locale_getters_new(obs_module_locale_getters_callback_t callback);
void *oracle_module_locale_getters_new(obs_module_locale_getters_callback_t callback)
{
	struct obs_module *module = calloc(1, sizeof(*module));
	if (!module)
		abort();
	module->get_string = callback;
	return module;
}
void oracle_module_locale_getters_free(void *module);
void oracle_module_locale_getters_free(void *module)
{
	free(module);
}
bool oracle_module_locale_getters_field_address(const void *opaque);
bool oracle_module_locale_getters_field_address(const void *opaque)
{
	const struct obs_module *module = opaque;
	return obs_module_locale_getters_callback_field(module) == &module->get_string &&
	       obs_module_locale_getters_callback_field(NULL) == NULL &&
	       sizeof(*obs_module_locale_getters_callback_field(module)) == sizeof(module->get_string) &&
	       _Alignof(obs_module_locale_getters_callback_t) == _Alignof(obs_module_locale_getters_callback_t);
}

void oracle_module_locale_getters_callback_layout(size_t *size, size_t *align);
void oracle_module_locale_getters_callback_layout(size_t *size, size_t *align)
{
	*size = sizeof(((struct obs_module *)0)->get_string);
	*align = _Alignof(obs_module_locale_getters_callback_t);
}
