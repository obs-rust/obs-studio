#include "libobs/build/libobs.pch.h"

obs_module_t *loadingModule = NULL;

bool obs_init_module(obs_module_t *module)
{
	if (!module || !obs)
		return false;
	if (module->loaded)
		return true;

	const char *profile_name =
		profile_store_name(obs_get_profiler_name_store(), "obs_init_module(%s)", module->file);
	profile_start(profile_name);

	loadingModule = module;
	module->loaded = module->load();
	loadingModule = NULL;

	if (!module->loaded)
		blog(LOG_WARNING, "Failed to initialize module '%s'", module->file);

	profile_end(profile_name);
	return module->loaded;
}
