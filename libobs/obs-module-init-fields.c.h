#include "libobs/build/libobs.pch.h"

bool *obs_module_init_loaded_field(obs_module_t *module);
bool *obs_module_init_loaded_field(obs_module_t *module)
{
	return &module->loaded;
}

bool obs_module_init_load(obs_module_t *module);
bool obs_module_init_load(obs_module_t *module)
{
	return module->load();
}

const char *obs_module_init_profile_start(obs_module_t *module);
const char *obs_module_init_profile_start(obs_module_t *module)
{
	const char *name = profile_store_name(obs_get_profiler_name_store(), "obs_init_module(%s)", module->file);
	profile_start(name);
	return name;
}

void obs_module_init_warning(obs_module_t *module);
void obs_module_init_warning(obs_module_t *module)
{
	blog(LOG_WARNING, "Failed to initialize module '%s'", module->file);
}

void obs_module_init_profile_end(const char *name);
void obs_module_init_profile_end(const char *name)
{
	profile_end(name);
}
