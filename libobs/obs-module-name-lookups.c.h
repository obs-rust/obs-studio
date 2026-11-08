#include "libobs/build/libobs.pch.h"

obs_module_t *obs_get_module(const char *name)
{
	obs_module_t *module = obs->first_module;
	while (module) {
		if (strcmp(module->mod_name, name) == 0) {
			return module;
		}

		module = module->next;
	}

	return NULL;
}

obs_module_t *obs_get_disabled_module(const char *name)
{
	obs_module_t *module = obs->first_disabled_module;
	while (module) {
		if (strcmp(module->mod_name, name) == 0) {
			return module;
		}

		module = module->next;
	}

	return NULL;
}
