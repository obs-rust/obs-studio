#include "libobs/build/libobs.pch.h"

void obs_log_loaded_modules(void)
{
	blog(LOG_INFO, "  Loaded Modules:");

	for (obs_module_t *mod = obs->first_module; !!mod; mod = mod->next)
		blog(LOG_INFO, "    %s", mod->file);
}

void obs_module_add_source(obs_module_t *module, const char *id)
{
	if (module) {
		char *source_id = bstrdup(id);
		da_push_back(module->sources, &source_id);
	}
}

void obs_module_add_output(obs_module_t *module, const char *id)
{
	if (module) {
		char *output_id = bstrdup(id);
		da_push_back(module->outputs, &output_id);
	}
}

void obs_module_add_encoder(obs_module_t *module, const char *id)
{
	if (module) {
		char *encoder_id = bstrdup(id);
		da_push_back(module->encoders, &encoder_id);
	}
}

void obs_module_add_service(obs_module_t *module, const char *id)
{
	if (module) {
		char *service_id = bstrdup(id);
		da_push_back(module->services, &service_id);
	}
}

char *obs_find_module_file(obs_module_t *module, const char *file)
{
	if (!module)
		return NULL;

	return obs_module_paths_find_file(module->data_path, file);
}

char *obs_module_get_config_path(obs_module_t *module, const char *file)
{
	if (!module)
		return NULL;

	return obs_module_names_config_path(obs->module_config_path, module->mod_name, file);
}

void obs_add_module_path(const char *bin, const char *data)
{
	if (!obs || !bin || !data)
		return;

	/* Paths from this API carry no type (they predate module types). */
	struct obs_module_path omp = {
		.bin = bstrdup(bin),
		.data = bstrdup(data),
		.type = OBS_MODULE_TYPE_UNDEFINED,
	};
	da_push_back(obs->module_paths, &omp);
}

void obs_add_safe_module(const char *name)
{
	if (!obs || !name)
		return;

	char *item = bstrdup(name);
	da_push_back(obs->safe_modules, &item);
}

void obs_add_disabled_module(const char *name)
{
	if (!obs || !name)
		return;

	char *item = bstrdup(name);
	da_push_back(obs->disabled_modules, &item);
}
