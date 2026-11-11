#include "libobs/build/libobs.pch.h"

obs_module_t *obs_source_get_module(const char *id)
{
	obs_module_t *module = obs->first_module;
	while (module) {
		for (size_t i = 0; i < module->sources.num; i++) {
			if (strcmp(module->sources.array[i], id) == 0) {
				return module;
			}
		}
		module = module->next;
	}

	module = obs->first_disabled_module;
	while (module) {
		for (size_t i = 0; i < module->sources.num; i++) {
			if (strcmp(module->sources.array[i], id) == 0) {
				return module;
			}
		}
		module = module->next;
	}

	return NULL;
}

enum obs_module_load_state obs_source_load_state(const char *id)
{
	if (!id)
		return OBS_MODULE_INVALID;

	if (obs_source_type_is_scene(id) || obs_source_type_is_group(id))
		return OBS_MODULE_ENABLED;

	obs_module_t *module = obs_source_get_module(id);
	if (!module) {
		return OBS_MODULE_MISSING;
	}
	return module->load_state;
}
