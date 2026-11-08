#include "libobs/build/libobs.pch.h"

const char *const *obs_module_metadata_file_field(obs_module_t *module)
{
	return module ? &module->file : NULL;
}

char *const *obs_module_metadata_binary_path_field(obs_module_t *module)
{
	return module ? &module->bin_path : NULL;
}

char *const *obs_module_metadata_data_path_field(obs_module_t *module)
{
	return module ? &module->data_path : NULL;
}

void *const *obs_module_metadata_lib_field(obs_module_t *module)
{
	return module ? &module->module : NULL;
}

char *const *obs_module_metadata_id_field(obs_module_t *module)
{
	return module && module->metadata ? &module->metadata->id : NULL;
}

char *const *obs_module_metadata_version_field(obs_module_t *module)
{
	return module && module->metadata ? &module->metadata->version : NULL;
}

char *const *obs_module_metadata_display_field(obs_module_t *module)
{
	return module && module->metadata ? &module->metadata->display_name : NULL;
}

obs_module_metadata_callback_t const *obs_module_metadata_name_field(obs_module_t *module)
{
	return module ? &module->name : NULL;
}

obs_module_metadata_callback_t const *obs_module_metadata_author_field(obs_module_t *module)
{
	return module ? &module->author : NULL;
}

obs_module_metadata_callback_t const *obs_module_metadata_description_field(obs_module_t *module)
{
	return module ? &module->description : NULL;
}
