#include "libobs/build/libobs.pch.h"

bool obs_is_core_module(obs_module_t *module)
{
	if (!module)
		return false;
	return obs_module_load_rules_is_core((uint32_t)module->module_type);
}

bool obs_is_legacy_module(obs_module_t *module)
{
	if (!module) {
		return false;
	}

	return obs_module_load_rules_is_legacy((uint32_t)module->module_type);
}
