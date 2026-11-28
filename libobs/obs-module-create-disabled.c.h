#include "libobs/build/libobs.pch.h"

bool obs_create_disabled_module(obs_module_t **module, const char *path, const char *data_path,
				enum obs_module_load_state state)
{
	struct obs_module mod = {0};

	mod.bin_path = bstrdup(path);
	mod.file = obs_module_names_file_name(mod.bin_path);
	mod.mod_name = get_module_name(mod.file);
	mod.data_path = bstrdup(data_path);
	mod.next = obs->first_disabled_module;
	mod.load_state = state;

	da_init(mod.sources);
	da_init(mod.outputs);
	da_init(mod.encoders);
	da_init(mod.services);

	obs_module_load_metadata(&mod);

	*module = bmemdup(&mod, sizeof(mod));
	obs->first_disabled_module = (*module);

	return true;
}
