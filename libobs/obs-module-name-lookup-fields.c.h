#include "libobs/build/libobs.pch.h"

obs_module_t *const *obs_module_name_lookup_head_field(bool disabled);
obs_module_t *const *obs_module_name_lookup_head_field(bool disabled)
{
	if (!obs)
		return NULL;
	return disabled ? &obs->first_disabled_module : &obs->first_module;
}

char *const *obs_module_name_lookup_name_field(const obs_module_t *module);
char *const *obs_module_name_lookup_name_field(const obs_module_t *module)
{
	return module ? &module->mod_name : NULL;
}

obs_module_t *const *obs_module_name_lookup_next_field(const obs_module_t *module);
obs_module_t *const *obs_module_name_lookup_next_field(const obs_module_t *module)
{
	return module ? &module->next : NULL;
}
