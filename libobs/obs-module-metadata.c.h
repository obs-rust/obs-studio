#include "libobs/build/libobs.pch.h"

const char *obs_get_module_file_name(obs_module_t *module)
{
	return module ? module->file : NULL;
}

const char *obs_get_module_name(obs_module_t *module)
{
	if (module && module->metadata && module->metadata->display_name) {
		return module->metadata->display_name;
	}

	return (module && module->name) ? module->name() : NULL;
}

const char *obs_get_module_author(obs_module_t *module)
{
	return (module && module->author) ? module->author() : NULL;
}

const char *obs_get_module_description(obs_module_t *module)
{
	return (module && module->description) ? module->description() : NULL;
}

const char *obs_get_module_binary_path(obs_module_t *module)
{
	return module ? module->bin_path : NULL;
}

const char *obs_get_module_data_path(obs_module_t *module)
{
	return module ? module->data_path : NULL;
}

const char *obs_get_module_id(obs_module_t *module)
{
	return module && module->metadata ? module->metadata->id : NULL;
}

const char *obs_get_module_version(obs_module_t *module)
{
	return module && module->metadata ? module->metadata->version : NULL;
}

void *obs_get_module_lib(obs_module_t *module)
{
	return module ? module->module : NULL;
}
