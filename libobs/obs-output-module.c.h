#include "libobs/build/libobs.pch.h"

obs_module_t *obs_output_get_module(const char *id)
{
	obs_module_t *module = obs->first_module;
	while (module) {
		for (size_t i = 0; i < module->outputs.num; i++) {
			if (strcmp(module->outputs.array[i], id) == 0) {
				return module;
			}
		}
		module = module->next;
	}

	module = obs->first_disabled_module;
	while (module) {
		for (size_t i = 0; i < module->outputs.num; i++) {
			if (strcmp(module->outputs.array[i], id) == 0) {
				return module;
			}
		}
		module = module->next;
	}

	return NULL;
}

enum obs_module_load_state obs_output_load_state(const char *id)
{
	obs_module_t *module = obs_output_get_module(id);
	if (!module) {
		return OBS_MODULE_MISSING;
	}
	return module->load_state;
}
