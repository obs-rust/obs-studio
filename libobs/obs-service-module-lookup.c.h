#include "libobs/build/libobs.pch.h"

obs_module_t *obs_service_module_first(bool disabled)
{
	return disabled ? obs->first_disabled_module : obs->first_module;
}

obs_module_t *obs_service_module_next(obs_module_t *module)
{
	return module->next;
}

size_t obs_service_module_service_count(obs_module_t *module)
{
	return module->services.num;
}

const char *obs_service_module_service_id(obs_module_t *module, size_t i)
{
	return module->services.array[i];
}

enum obs_module_load_state obs_service_module_load_state(obs_module_t *module)
{
	return module->load_state;
}
