/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 2 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.
******************************************************************************/

#include "libobs/build/libobs.pch.h"

extern const char *get_module_extension(void);

obs_module_t *loadingModule = NULL;

static inline int req_func_not_found(const char *name, const char *path)
{
	blog(LOG_DEBUG,
	     "Required module function '%s' in module '%s' not "
	     "found, loading of module failed",
	     name, path);
	return MODULE_MISSING_EXPORTS;
}

static int load_module_exports(struct obs_module *mod, const char *path)
{
	mod->load = os_dlsym(mod->module, "obs_module_load");
	if (!mod->load)
		return req_func_not_found("obs_module_load", path);

	mod->set_pointer = os_dlsym(mod->module, "obs_module_set_pointer");
	if (!mod->set_pointer)
		return req_func_not_found("obs_module_set_pointer", path);

	mod->ver = os_dlsym(mod->module, "obs_module_ver");
	if (!mod->ver)
		return req_func_not_found("obs_module_ver", path);

	/* optional exports */
	mod->unload = os_dlsym(mod->module, "obs_module_unload");
	mod->post_load = os_dlsym(mod->module, "obs_module_post_load");
	mod->set_locale = os_dlsym(mod->module, "obs_module_set_locale");
	mod->free_locale = os_dlsym(mod->module, "obs_module_free_locale");
	mod->name = os_dlsym(mod->module, "obs_module_name");
	mod->description = os_dlsym(mod->module, "obs_module_description");
	mod->author = os_dlsym(mod->module, "obs_module_author");
	mod->get_string = os_dlsym(mod->module, "obs_module_get_string");
	return MODULE_SUCCESS;
}

bool obs_module_get_locale_string(const obs_module_t *mod, const char *lookup_string, const char **translated_string)
{
	assert(mod);
	if (mod && mod->get_string) {
		return mod->get_string(lookup_string, translated_string);
	}

	return false;
}

const char *obs_module_get_locale_text(const obs_module_t *mod, const char *text)
{
	const char *str = text;
	obs_module_get_locale_string(mod, text, &str);
	return str;
}

static inline char *get_module_name(const char *file)
{
	return obs_module_names_module_name(file, get_module_extension());
}

int obs_module_load_metadata(struct obs_module *mod)
{
	struct obs_module_metadata *md = NULL;

	/* Check if the metadata file exists */
	char *path = obs_module_names_manifest_path(mod->data_path);

	if (os_file_exists(path)) {
		/* If we find a metadata file, allocate a new metadata. */
		md = bmalloc(sizeof(obs_module_metadata_t));
		obs_data_t *metadata = obs_data_create_from_json_file(path);

		md->display_name = bstrdup(obs_data_get_string(metadata, "display_name"));
		md->id = bstrdup(obs_data_get_string(metadata, "id"));
		md->version = bstrdup(obs_data_get_string(metadata, "version"));
		md->os_arch = bstrdup(obs_data_get_string(metadata, "os_arch"));
		md->name = bstrdup(obs_data_get_string(metadata, "name"));
		md->description = bstrdup(obs_data_get_string(metadata, "description"));
		md->long_description = bstrdup(obs_data_get_string(metadata, "long_description"));

		obs_data_t *urls = obs_data_get_obj(metadata, "urls");
		md->repository_url = bstrdup(obs_data_get_string(urls, "repository"));
		md->website_url = bstrdup(obs_data_get_string(urls, "website"));
		md->support_url = bstrdup(obs_data_get_string(urls, "support"));
		obs_data_release(urls);

		md->has_banner = obs_data_get_bool(metadata, "has_banner");
		md->has_icon = obs_data_get_bool(metadata, "has_icon");
		obs_data_release(metadata);
	}
	bfree(path);
	mod->metadata = md;
	return MODULE_SUCCESS;
}

int obs_open_module(obs_module_t **module, const char *path, const char *data_path)
{
	struct obs_module mod = {0};
	int errorcode;

	if (!module || !path || !obs)
		return MODULE_ERROR;

	if (obs_host_skip_module(path))
		return MODULE_HARDCODED_SKIP;

	blog(LOG_DEBUG, "---------------------------------");

	mod.module = os_dlopen(path);
	if (!mod.module) {
		blog(LOG_WARNING, "Module '%s' not loaded", path);
		return MODULE_FAILED_TO_OPEN;
	}

	errorcode = load_module_exports(&mod, path);
	if (errorcode != MODULE_SUCCESS)
		return errorcode;

	/* Reject plugins compiled with a newer libobs. Patch version (lower 16-bit) is ignored. */
	uint32_t ver = mod.ver ? mod.ver() & 0xFFFF0000 : 0;
	if (ver > LIBOBS_API_VER) {
		blog(LOG_WARNING, "Module '%s' compiled with newer libobs %d.%d", path, (ver >> 24) & 0xFF,
		     (ver >> 16) & 0xFF);
		return MODULE_INCOMPATIBLE_VER;
	}

	mod.bin_path = bstrdup(path);
	mod.file = obs_module_names_file_name(mod.bin_path);
	mod.mod_name = get_module_name(mod.file);
	mod.data_path = bstrdup(data_path);
	mod.next = obs->first_module;
	mod.load_state = OBS_MODULE_ENABLED;

	da_init(mod.sources);
	da_init(mod.outputs);
	da_init(mod.encoders);
	da_init(mod.services);

	if (mod.file) {
		blog(LOG_DEBUG, "Loading module: %s", mod.file);
	}

	obs_module_load_metadata(&mod);

	*module = bmemdup(&mod, sizeof(mod));
	obs->first_module = (*module);
	mod.set_pointer(*module);

	if (mod.set_locale)
		mod.set_locale(obs->locale);

	return MODULE_SUCCESS;
}

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

void obs_log_loaded_modules(void)
{
	blog(LOG_INFO, "  Loaded Modules:");

	for (obs_module_t *mod = obs->first_module; !!mod; mod = mod->next)
		blog(LOG_INFO, "    %s", mod->file);
}

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

obs_module_t *obs_get_module(const char *name)
{
	obs_module_t *module = obs->first_module;
	while (module) {
		if (strcmp(module->mod_name, name) == 0) {
			return module;
		}

		module = module->next;
	}

	return NULL;
}

obs_module_t *obs_get_disabled_module(const char *name)
{
	obs_module_t *module = obs->first_disabled_module;
	while (module) {
		if (strcmp(module->mod_name, name) == 0) {
			return module;
		}

		module = module->next;
	}

	return NULL;
}

void *obs_get_module_lib(obs_module_t *module)
{
	return module ? module->module : NULL;
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

extern void get_plugin_info(const char *path, bool *is_obs_plugin);

struct fail_info {
	struct dstr fail_modules;
	size_t fail_count;
};

static bool is_safe_module(const char *name)
{
	if (!obs->safe_modules.num)
		return true;

	return obs_module_names_listed(obs->safe_modules.array, obs->safe_modules.num, name);
}

static bool is_core_module(const char *name)
{
	return obs_in_core_module_list(name);
}

static bool is_disabled_module(const char *name)
{
	if (obs->disabled_modules.num == 0)
		return false;

	return obs_module_names_listed(obs->disabled_modules.array, obs->disabled_modules.num, name);
}

bool obs_get_module_allow_disable(const char *name)
{
	return !is_core_module(name);
}

static void load_all_callback(void *param, const struct obs_module_info2 *info)
{
	struct fail_info *fail_info = param;
	obs_module_t *module;
	obs_module_t *disabled_module;

	bool is_obs_plugin;

	get_plugin_info(info->bin_path, &is_obs_plugin);

	if (!is_obs_plugin) {
		blog(LOG_WARNING, "Skipping module '%s', not an OBS plugin", info->bin_path);
		return;
	}

	if (!is_safe_module(info->name)) {
		obs_create_disabled_module(&disabled_module, info->bin_path, info->data_path, OBS_MODULE_DISABLED_SAFE);
		blog(LOG_WARNING, "Skipping module '%s', not on safe list", info->name);
		return;
	}

	if (info->type != OBS_MODULE_TYPE_CORE && is_disabled_module(info->name)) {
		obs_create_disabled_module(&disabled_module, info->bin_path, info->data_path, OBS_MODULE_DISABLED);
		blog(LOG_WARNING, "Skipping module '%s', is disabled", info->name);
		return;
	}

	/* This is a necessary check until legacy plugin loading is removed in a future update.
	 * It covers portable installs where the old core modules still live in `obs-plugins`.
	 */
	if (info->type == OBS_MODULE_TYPE_LEGACY_PLUGIN && is_core_module(info->name)) {
		blog(LOG_WARNING, "Skipping module '%s', is a legacy core module", info->name);
		return;
	}

	obs_module_t *existing_module = obs_get_module(info->name);
	if (info->type != OBS_MODULE_TYPE_CORE && existing_module != NULL) {
		if (info->type == OBS_MODULE_TYPE_LEGACY_PLUGIN) {
			blog(LOG_WARNING, "Skipping legacy plugin '%s' at '%s', already loaded from '%s'", info->name,
			     info->bin_path, existing_module->bin_path);
		} else {
			blog(LOG_WARNING, "Skipping plugin '%s' at '%s', already loaded from '%s'", info->name,
			     info->bin_path, existing_module->bin_path);
		}
		return;
	}

	int code = obs_open_module(&module, info->bin_path, info->data_path);
	switch (code) {
	case MODULE_MISSING_EXPORTS:
		blog(LOG_DEBUG, "Failed to load module file '%s', not an OBS plugin", info->bin_path);
		return;
	case MODULE_FAILED_TO_OPEN:
		blog(LOG_DEBUG, "Failed to load module file '%s', module failed to open", info->bin_path);
		obs_create_disabled_module(&disabled_module, info->bin_path, info->data_path,
					   OBS_MODULE_FAILED_TO_OPEN);
		if (info->type == OBS_MODULE_TYPE_CORE) {
			/* Most often a library the module needs is not
			 * installed (libcef.so for obs-browser). Carry on
			 * without it, as for a plugin, instead of failing
			 * the core module load and with it OBS startup. */
			disabled_module->module_type = info->type;
			blog(LOG_WARNING, "Core module '%s' could not be opened, continuing without it", info->name);
			return;
		}
		goto load_failure;
	case MODULE_ERROR:
		blog(LOG_DEBUG, "Failed to load module file '%s' (unknown error)", info->bin_path);
		goto load_failure;
	case MODULE_INCOMPATIBLE_VER:
		blog(LOG_DEBUG, "Failed to load module file '%s', incompatible version", info->bin_path);
		obs_create_disabled_module(&disabled_module, info->bin_path, info->data_path,
					   OBS_MODULE_FAILED_TO_OPEN);
		goto load_failure;
	case MODULE_HARDCODED_SKIP:
		return;
	}

	if (!obs_init_module(module)) {
		free_module(module);
		obs_create_disabled_module(&disabled_module, info->bin_path, info->data_path,
					   OBS_MODULE_FAILED_TO_INITIALIZE);
		disabled_module->module_type = info->type;
	} else {
		module->module_type = info->type;
	}

	UNUSED_PARAMETER(param);
	return;

load_failure:
	if (fail_info) {
		dstr_cat(&fail_info->fail_modules, info->name);
		dstr_cat(&fail_info->fail_modules, ";");
		fail_info->fail_count++;
	}
}

void obs_load_all_modules(void) {}

void obs_load_all_modules2(struct obs_module_failure_info *mfi)
{
	UNUSED_PARAMETER(mfi);
}

bool obs_load_core_modules(void)
{
	if (obs->core_modules_loaded) {
		return true;
	}

	struct fail_info error = {0};

	load_core_modules(load_all_callback, &error);

	obs->core_modules_loaded = error.fail_count == 0;

	dstr_free(&error.fail_modules);

	return obs->core_modules_loaded;
}

void obs_load_plugins(struct obs_runtime_module_info *info, struct obs_module_failure_info *error)
{
	struct fail_info failure = {0};

	obs_module_paths_find(info->path_info.binary, info->path_info.data, info->type, get_module_extension(),
			      obs_host_add_module_extension, load_all_callback, &failure);

	error->count = failure.fail_count;
	error->failed_modules = strlist_split(failure.fail_modules.array, ';', false);
	dstr_free(&failure.fail_modules);
}

void obs_module_failure_info_free(struct obs_module_failure_info *mfi)
{
	if (mfi->failed_modules) {
		bfree(mfi->failed_modules);
		mfi->failed_modules = NULL;
	}
}

void obs_post_load_modules(void)
{
	obs_host_post_load_modules();
	for (obs_module_t *mod = obs->first_module; !!mod; mod = mod->next)
		if (mod->post_load)
			mod->post_load();
}

/* Function to find a specifically defined module. Used for loading core modules by name.
 * info->path_info.binary should be path to the module binary, without its file extension.
 * info->path_info.data should be path to the module data directory including the module name.
 */
bool find_core_module(struct obs_runtime_module_info *info, obs_find_module_callback2_t callback, void *data)
{
	const char *name = obs_module_names_file_name(info->path_info.binary);
	info->name = name;

	if (info->type != OBS_MODULE_TYPE_CORE) {
		blog(LOG_WARNING, "Module %s is not a core module, and must be loaded with the plugin loader.", name);
		return false;
	}

	return obs_module_names_find_core(info->path_info.binary, name, info->path_info.data, info->type,
					  get_module_extension(), callback, data);
}

bool obs_is_core_module(obs_module_t *module)
{
	if (!module)
		return false;
	return module->module_type == OBS_MODULE_TYPE_CORE;
}

bool obs_is_legacy_module(obs_module_t *module)
{
	if (!module) {
		return false;
	}

	return module->module_type == OBS_MODULE_TYPE_LEGACY_PLUGIN;
}

void obs_find_modules2(obs_find_module_callback2_t callback, void *param)
{
	if (!obs)
		return;

	for (size_t i = 0; i < obs->module_paths.num; i++) {
		struct obs_module_path *omp = obs->module_paths.array + i;
		obs_module_paths_find(omp->bin, omp->data, omp->type, get_module_extension(),
				      obs_host_add_module_extension, callback, param);
	}
}

void obs_find_modules(obs_find_module_callback_t callback, void *param)
{
	/* the structure is ABI compatible so we can just cast the callback */
	obs_find_modules2((obs_find_module_callback2_t)callback, param);
}

void obs_enum_modules(obs_enum_module_callback_t callback, void *param)
{
	struct obs_module *module;
	if (!obs)
		return;

	module = obs->first_module;
	while (module) {
		callback(param, module);
		module = module->next;
	}
}

void free_module(struct obs_module *mod)
{
	if (!mod)
		return;

	if (mod->module) {
		if (mod->free_locale)
			mod->free_locale();

		if (mod->loaded && mod->unload)
			mod->unload();

		/* there is no real reason to close the dynamic libraries,
		 * and sometimes this can cause issues. */
		/* os_dlclose(mod->module); */
	}

	/* Is this module an active / loaded module, or a disabled module? */
	if (mod->load_state == OBS_MODULE_ENABLED) {
		for (obs_module_t *m = obs->first_module; !!m; m = m->next) {
			if (m->next == mod) {
				m->next = mod->next;
				break;
			}
		}

		if (obs->first_module == mod)
			obs->first_module = mod->next;
	} else {
		for (obs_module_t *m = obs->first_disabled_module; !!m; m = m->next) {
			if (m->next == mod) {
				m->next = mod->next;
				break;
			}
		}

		if (obs->first_disabled_module == mod)
			obs->first_disabled_module = mod->next;
	}

	bfree(mod->mod_name);
	bfree(mod->bin_path);
	bfree(mod->data_path);

	for (size_t i = 0; i < mod->sources.num; i++) {
		bfree(mod->sources.array[i]);
	}
	da_free(mod->sources);

	for (size_t i = 0; i < mod->outputs.num; i++) {
		bfree(mod->outputs.array[i]);
	}
	da_free(mod->outputs);

	for (size_t i = 0; i < mod->encoders.num; i++) {
		bfree(mod->encoders.array[i]);
	}
	da_free(mod->encoders);

	for (size_t i = 0; i < mod->services.num; i++) {
		bfree(mod->services.array[i]);
	}
	da_free(mod->services);

	if (mod->metadata) {
		free_module_metadata(mod->metadata);
		bfree(mod->metadata);
	}
	bfree(mod);
}

lookup_t *obs_module_load_locale(obs_module_t *module, const char *default_locale, const char *locale)
{
	char *name = NULL;
	lookup_t *lookup = NULL;

	if (!module || !default_locale || !locale) {
		blog(LOG_WARNING, "obs_module_load_locale: Invalid parameters");
		return NULL;
	}

	name = obs_module_locale_default_file(default_locale);

	char *file = obs_find_module_file(module, name);
	if (file)
		lookup = text_lookup_create(file);

	bfree(file);

	if (!lookup) {
		blog(LOG_WARNING, "Failed to load '%s' text for module: '%s'", default_locale, module->file);
		goto cleanup;
	}

	if (obs_module_locale_is_default(locale, default_locale))
		goto cleanup;

	bfree(name);
	name = obs_module_locale_file(locale);

	file = obs_find_module_file(module, name);

	if (!text_lookup_add(lookup, file))
		blog(LOG_WARNING, "Failed to load '%s' text for module: '%s'", locale, module->file);

	bfree(file);
cleanup:
	bfree(name);
	return lookup;
}

/* A registration copies only the fields of the caller's structure that lie
 * wholly within its size, the rule HAS_VAL() applies to the callbacks. A
 * field the size ends inside stays zero instead of keeping some of the
 * caller's bytes: a partly copied pointer would be called later, an
 * optional callback when OBS needs it and free_type_data at shutdown.
 *
 * Every field of the four info structures is a pointer, aligned to and as
 * large as a pointer, except the 32-bit fields each type lists here. So the
 * copy ends at the size rounded down to a pointer, or at the end of a 32-bit
 * field that starts there and lies wholly within the size. A size of
 * sizeof() is a multiple of a pointer, so such a copy is whole, as before. */
static size_t whole_fields_size(size_t size, const size_t *u32_fields, size_t count)
{
	size_t whole = size - size % sizeof(void *);

	for (size_t i = 0; i < count; i++) {
		if (u32_fields[i] >= whole && u32_fields[i] + sizeof(uint32_t) <= size)
			whole = u32_fields[i] + sizeof(uint32_t);
	}
	return whole;
}

/* The 32-bit fields. */
_Static_assert(sizeof(((struct obs_source_info *)NULL)->type) == sizeof(uint32_t), "obs_source_info.type");
_Static_assert(sizeof(((struct obs_source_info *)NULL)->output_flags) == sizeof(uint32_t),
	       "obs_source_info.output_flags");
_Static_assert(sizeof(((struct obs_source_info *)NULL)->icon_type) == sizeof(uint32_t), "obs_source_info.icon_type");
_Static_assert(sizeof(((struct obs_source_info *)NULL)->version) == sizeof(uint32_t), "obs_source_info.version");
_Static_assert(sizeof(((struct obs_output_info *)NULL)->flags) == sizeof(uint32_t), "obs_output_info.flags");
_Static_assert(sizeof(((struct obs_encoder_info *)NULL)->type) == sizeof(uint32_t), "obs_encoder_info.type");
_Static_assert(sizeof(((struct obs_encoder_info *)NULL)->caps) == sizeof(uint32_t), "obs_encoder_info.caps");

static size_t copy_size_obs_source_info(size_t size)
{
	const size_t u32_fields[] = {
		offsetof(struct obs_source_info, type),
		offsetof(struct obs_source_info, output_flags),
		offsetof(struct obs_source_info, icon_type),
		offsetof(struct obs_source_info, version),
	};
	return whole_fields_size(size, u32_fields, sizeof(u32_fields) / sizeof(u32_fields[0]));
}

static size_t copy_size_obs_output_info(size_t size)
{
	const size_t u32_fields[] = {offsetof(struct obs_output_info, flags)};
	return whole_fields_size(size, u32_fields, sizeof(u32_fields) / sizeof(u32_fields[0]));
}

static size_t copy_size_obs_encoder_info(size_t size)
{
	const size_t u32_fields[] = {
		offsetof(struct obs_encoder_info, type),
		offsetof(struct obs_encoder_info, caps),
	};
	return whole_fields_size(size, u32_fields, sizeof(u32_fields) / sizeof(u32_fields[0]));
}

static size_t copy_size_obs_service_info(size_t size)
{
	return whole_fields_size(size, NULL, 0);
}

#define REGISTER_OBS_DEF(size_var, structure, dest, info)                                               \
	do {                                                                                            \
		struct structure data = {0};                                                            \
		if (!size_var) {                                                                        \
			blog(LOG_ERROR, "Tried to register " #structure " outside of obs_module_load"); \
			return;                                                                         \
		}                                                                                       \
                                                                                                        \
		if (!obs_module_source_rules_size_supported(size_var, sizeof(data))) {                  \
			blog(LOG_ERROR,                                                                 \
			     "Tried to register " #structure " with size %llu which is more "           \
			     "than libobs currently supports "                                          \
			     "(%llu)",                                                                  \
			     (long long unsigned)size_var, (long long unsigned)sizeof(data));           \
			goto error;                                                                     \
		}                                                                                       \
                                                                                                        \
		memcpy(&data, info, copy_size_##structure(size_var));                                   \
		da_push_back(dest, &data);                                                              \
	} while (false)

#define HAS_VAL(type, info, val) ((offsetof(type, val) + sizeof(info->val) <= size) && info->val)

#define HANDLE_ERROR(size_var, structure, info)                                         \
	do {                                                                            \
		/* Free the caller's type data only when type_data and          \
		 * free_type_data each lie wholly within the size, as in        \
		 * HAS_VAL(): a size that ends inside a pointer would leave it  \
		 * partly copied, and calling that jumps to a wrong address.    \
		 */                                                             \
		if (offsetof(struct structure, type_data) + sizeof(info->type_data) <= (size_var) &&       \
		    offsetof(struct structure, free_type_data) + sizeof(info->free_type_data) <= (size_var) && \
		    info->type_data && info->free_type_data)                                           \
			info->free_type_data(info->type_data);                                     \
	} while (false)

#define source_warn(format, ...) blog(LOG_WARNING, "obs_register_source: " format, ##__VA_ARGS__)
#define output_warn(format, ...) blog(LOG_WARNING, "obs_register_output: " format, ##__VA_ARGS__)
#define encoder_warn(format, ...) blog(LOG_WARNING, "obs_register_encoder: " format, ##__VA_ARGS__)
#define service_warn(format, ...) blog(LOG_WARNING, "obs_register_service: " format, ##__VA_ARGS__)

void obs_register_source_s(const struct obs_source_info *info, size_t size)
{
	struct obs_source_info data = {0};
	obs_source_info_array_t *array = NULL;
	enum obs_module_source_rules_error err;
	uint32_t callbacks = 0;

	switch (obs_module_source_rules_list_of(info->type)) {
	case OBS_MODULE_SOURCE_RULES_LIST_INPUT:
		array = &obs->input_types;
		break;
	case OBS_MODULE_SOURCE_RULES_LIST_FILTER:
		array = &obs->filter_types;
		break;
	case OBS_MODULE_SOURCE_RULES_LIST_TRANSITION:
		array = &obs->transition_types;
		break;
	case OBS_MODULE_SOURCE_RULES_LIST_NONE:
		break;
	case OBS_MODULE_SOURCE_RULES_LIST_UNKNOWN:
	default:
		source_warn("Tried to register unknown source type: %u", info->type);
		goto error;
	}

	if (get_source_info2(info->id, info->version)) {
		source_warn("Source '%s' already exists!  "
			    "Duplicate library?",
			    info->id);
		goto error;
	}

	if (!obs_module_source_rules_size_supported(size, sizeof(data))) {
		source_warn("Tried to register obs_source_info with size "
			    "%llu which is more than libobs currently "
			    "supports (%llu)",
			    (long long unsigned)size, (long long unsigned)sizeof(data));
		goto error;
	}

	memcpy(&data, info, copy_size_obs_source_info(size));

	if (data.type == OBS_SOURCE_TYPE_TRANSITION) {
		if (data.get_width)
			source_warn("get_width ignored registering "
				    "transition '%s'",
				    data.id);
		if (data.get_height)
			source_warn("get_height ignored registering "
				    "transition '%s'",
				    data.id);
	}

	data.output_flags = obs_module_source_rules_output_flags(data.type, data.output_flags);

	if (HAS_VAL(struct obs_source_info, info, get_name))
		callbacks |= OBS_MODULE_SOURCE_RULES_HAS_GET_NAME;
	if (HAS_VAL(struct obs_source_info, info, get_width))
		callbacks |= OBS_MODULE_SOURCE_RULES_HAS_GET_WIDTH;
	if (HAS_VAL(struct obs_source_info, info, get_height))
		callbacks |= OBS_MODULE_SOURCE_RULES_HAS_GET_HEIGHT;
	if (HAS_VAL(struct obs_source_info, info, audio_render))
		callbacks |= OBS_MODULE_SOURCE_RULES_HAS_AUDIO_RENDER;

	/* The required callbacks are checked against the caller's type and
	 * flags, the composite rules against the normalized flags. */
#define source_required(val)                                                                 \
	blog(LOG_ERROR,                                                                       \
	     "Required value '" #val "' for "                                                 \
	     "'%s' not found.  obs_register_source failed.",                                  \
	     info->id)
	err = obs_module_source_rules_check(info->type, info->output_flags, data.output_flags, callbacks);
	switch (err) {
	case OBS_MODULE_SOURCE_RULES_OK:
		break;
	case OBS_MODULE_SOURCE_RULES_COMPOSITE_AUDIO:
		source_warn("Source '%s': Composite sources "
			    "cannot be audio sources",
			    info->id);
		break;
	case OBS_MODULE_SOURCE_RULES_COMPOSITE_ASYNC:
		source_warn("Source '%s': Composite sources "
			    "cannot be async sources",
			    info->id);
		break;
	case OBS_MODULE_SOURCE_RULES_NO_GET_NAME:
		source_required(get_name);
		break;
	case OBS_MODULE_SOURCE_RULES_NO_GET_WIDTH:
		source_required(get_width);
		break;
	case OBS_MODULE_SOURCE_RULES_NO_GET_HEIGHT:
		source_required(get_height);
		break;
	case OBS_MODULE_SOURCE_RULES_NO_AUDIO_RENDER:
		source_required(audio_render);
		break;
	}
	if (err != OBS_MODULE_SOURCE_RULES_OK)
		goto error;
#undef source_required

	/* version-related stuff */
	data.unversioned_id = data.id;
	data.id = obs_module_source_rules_versioned_id(data.id, data.version);

	if (array)
		da_push_back(*array, &data);
	da_push_back(obs->source_types, &data);

	/* The loading module names the type only once it is registered. */
	if (loadingModule) {
		char *source_id = bstrdup(info->id);
		da_push_back(loadingModule->sources, &source_id);
	}
	return;

error:
	HANDLE_ERROR(size, obs_source_info, info);
}

/* The constants obs-module-type-rules.h copies. */
_Static_assert(OBS_MODULE_TYPE_RULES_OUTPUT_VIDEO == OBS_OUTPUT_VIDEO, "OBS_OUTPUT_VIDEO");
_Static_assert(OBS_MODULE_TYPE_RULES_OUTPUT_AUDIO == OBS_OUTPUT_AUDIO, "OBS_OUTPUT_AUDIO");
_Static_assert(OBS_MODULE_TYPE_RULES_OUTPUT_ENCODED == OBS_OUTPUT_ENCODED, "OBS_OUTPUT_ENCODED");
_Static_assert(OBS_MODULE_TYPE_RULES_OUTPUT_SERVICE == OBS_OUTPUT_SERVICE, "OBS_OUTPUT_SERVICE");
_Static_assert(OBS_MODULE_TYPE_RULES_OUTPUT_MULTI_TRACK == OBS_OUTPUT_MULTI_TRACK, "OBS_OUTPUT_MULTI_TRACK");
_Static_assert(OBS_MODULE_TYPE_RULES_ENCODER_AUDIO == OBS_ENCODER_AUDIO, "OBS_ENCODER_AUDIO");
_Static_assert(OBS_MODULE_TYPE_RULES_CAP_PASS_TEXTURE == OBS_ENCODER_CAP_PASS_TEXTURE, "OBS_ENCODER_CAP_PASS_TEXTURE");
_Static_assert(OBS_MODULE_TYPE_RULES_CAP_SCALING == OBS_ENCODER_CAP_SCALING, "OBS_ENCODER_CAP_SCALING");

/* The message of a missing callback, as CHECK_REQUIRED_VAL logged it. */
#define type_required(val, func)                             \
	blog(LOG_ERROR,                                      \
	     "Required value '" #val "' for "                \
	     "'%s' not found.  " #func " failed.",           \
	     info->id)
#define has_val(type, val, bit)                   \
	do {                                      \
		if (HAS_VAL(type, info, val))     \
			callbacks |= OBS_MODULE_TYPE_RULES_HAS_##bit; \
	} while (false)

void obs_register_output_s(const struct obs_output_info *info, size_t size)
{
	enum obs_module_type_rules_error err;
	uint32_t callbacks = 0;

	if (find_output(info->id)) {
		output_warn("Output id '%s' already exists!  "
			    "Duplicate library?",
			    info->id);
		goto error;
	}

	has_val(struct obs_output_info, get_name, GET_NAME);
	has_val(struct obs_output_info, create, CREATE);
	has_val(struct obs_output_info, destroy, DESTROY);
	has_val(struct obs_output_info, start, START);
	has_val(struct obs_output_info, stop, STOP);
	has_val(struct obs_output_info, protocols, PROTOCOLS);
	has_val(struct obs_output_info, encoded_packet, ENCODED_PACKET);
	has_val(struct obs_output_info, raw_video, RAW_VIDEO);
	has_val(struct obs_output_info, raw_audio, RAW_AUDIO);
	has_val(struct obs_output_info, raw_audio2, RAW_AUDIO2);

	err = obs_module_type_rules_output_check(info->flags, callbacks);
	switch (err) {
	case OBS_MODULE_TYPE_RULES_OK:
		break;
	case OBS_MODULE_TYPE_RULES_NO_GET_NAME:
		type_required(get_name, obs_register_output);
		break;
	case OBS_MODULE_TYPE_RULES_NO_CREATE:
		type_required(create, obs_register_output);
		break;
	case OBS_MODULE_TYPE_RULES_NO_DESTROY:
		type_required(destroy, obs_register_output);
		break;
	case OBS_MODULE_TYPE_RULES_NO_START:
		type_required(start, obs_register_output);
		break;
	case OBS_MODULE_TYPE_RULES_NO_STOP:
		type_required(stop, obs_register_output);
		break;
	case OBS_MODULE_TYPE_RULES_NO_PROTOCOLS:
		type_required(protocols, obs_register_output);
		break;
	case OBS_MODULE_TYPE_RULES_NO_ENCODED_PACKET:
		type_required(encoded_packet, obs_register_output);
		break;
	case OBS_MODULE_TYPE_RULES_NO_RAW_VIDEO:
		type_required(raw_video, obs_register_output);
		break;
	case OBS_MODULE_TYPE_RULES_NO_RAW_AUDIO:
		type_required(raw_audio, obs_register_output);
		break;
	case OBS_MODULE_TYPE_RULES_NO_RAW_AUDIO2:
		type_required(raw_audio2, obs_register_output);
		break;
	default:
		break;
	}
	if (err != OBS_MODULE_TYPE_RULES_OK)
		goto error;

	REGISTER_OBS_DEF(size, obs_output_info, obs->output_types, info);

	if (info->flags & OBS_OUTPUT_SERVICE) {
		char **protocols = strlist_split(info->protocols, ';', false);
		for (char **protocol = protocols; *protocol; ++protocol) {
			if (obs_module_type_rules_protocol_listed(*protocol, obs->data.protocols.array,
								  obs->data.protocols.num))
				continue;
			char *new_prtcl = bstrdup(*protocol);
			da_push_back(obs->data.protocols, &new_prtcl);
		}
		strlist_free(protocols);
	}

	if (loadingModule) {
		char *output_id = bstrdup(info->id);
		da_push_back(loadingModule->outputs, &output_id);
	}

	return;

error:
	HANDLE_ERROR(size, obs_output_info, info);
}

void obs_register_encoder_s(const struct obs_encoder_info *info, size_t size)
{
	enum obs_module_type_rules_error err;
	uint32_t callbacks = 0;

	if (find_encoder(info->id)) {
		encoder_warn("Encoder id '%s' already exists!  "
			     "Duplicate library?",
			     info->id);
		goto error;
	}

	has_val(struct obs_encoder_info, get_name, GET_NAME);
	has_val(struct obs_encoder_info, create, CREATE);
	has_val(struct obs_encoder_info, destroy, DESTROY);
	has_val(struct obs_encoder_info, encode, ENCODE);
	has_val(struct obs_encoder_info, encode_texture, ENCODE_TEXTURE);
	has_val(struct obs_encoder_info, encode_texture2, ENCODE_TEXTURE2);
	has_val(struct obs_encoder_info, get_frame_size, GET_FRAME_SIZE);

	/* The type and the capabilities are the caller's, read whatever the
	 * size. */
	err = obs_module_type_rules_encoder_check((uint32_t)info->type, info->caps, callbacks);
	switch (err) {
	case OBS_MODULE_TYPE_RULES_OK:
		break;
	case OBS_MODULE_TYPE_RULES_TEXTURE_SCALING:
		encoder_warn("Texture encoders cannot self-scale. Encoder id '%s' not registered.", info->id);
		break;
	case OBS_MODULE_TYPE_RULES_NO_GET_NAME:
		type_required(get_name, obs_register_encoder);
		break;
	case OBS_MODULE_TYPE_RULES_NO_CREATE:
		type_required(create, obs_register_encoder);
		break;
	case OBS_MODULE_TYPE_RULES_NO_DESTROY:
		type_required(destroy, obs_register_encoder);
		break;
	case OBS_MODULE_TYPE_RULES_NO_ENCODE_TEXTURE:
		blog(LOG_ERROR,
		     "Neither 'encode_texture' nor 'encode_texture2' "
		     "for '%s' found.  obs_register_encoder failed.",
		     info->id);
		break;
	case OBS_MODULE_TYPE_RULES_NO_ENCODE:
		type_required(encode, obs_register_encoder);
		break;
	case OBS_MODULE_TYPE_RULES_NO_GET_FRAME_SIZE:
		type_required(get_frame_size, obs_register_encoder);
		break;
	default:
		break;
	}
	if (err != OBS_MODULE_TYPE_RULES_OK)
		goto error;

	REGISTER_OBS_DEF(size, obs_encoder_info, obs->encoder_types, info);

	if (loadingModule) {
		char *encoder_id = bstrdup(info->id);
		da_push_back(loadingModule->encoders, &encoder_id);
	}

	return;

error:
	HANDLE_ERROR(size, obs_encoder_info, info);
}

void obs_register_service_s(const struct obs_service_info *info, size_t size)
{
	enum obs_module_type_rules_error err;
	uint32_t callbacks = 0;

	if (find_service(info->id)) {
		service_warn("Service id '%s' already exists!  "
			     "Duplicate library?",
			     info->id);
		goto error;
	}

	has_val(struct obs_service_info, get_name, GET_NAME);
	has_val(struct obs_service_info, create, CREATE);
	has_val(struct obs_service_info, destroy, DESTROY);
	has_val(struct obs_service_info, get_protocol, GET_PROTOCOL);

	err = obs_module_type_rules_service_check(callbacks);
	switch (err) {
	case OBS_MODULE_TYPE_RULES_OK:
		break;
	case OBS_MODULE_TYPE_RULES_NO_GET_NAME:
		type_required(get_name, obs_register_service);
		break;
	case OBS_MODULE_TYPE_RULES_NO_CREATE:
		type_required(create, obs_register_service);
		break;
	case OBS_MODULE_TYPE_RULES_NO_DESTROY:
		type_required(destroy, obs_register_service);
		break;
	case OBS_MODULE_TYPE_RULES_NO_GET_PROTOCOL:
		type_required(get_protocol, obs_register_service);
		break;
	default:
		break;
	}
	if (err != OBS_MODULE_TYPE_RULES_OK)
		goto error;

	REGISTER_OBS_DEF(size, obs_service_info, obs->service_types, info);

	if (loadingModule) {
		char *service_id = bstrdup(info->id);
		da_push_back(loadingModule->services, &service_id);
	}

	return;

error:
	HANDLE_ERROR(size, obs_service_info, info);
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef type_required
#undef has_val
#undef REGISTER_OBS_DEF
#undef HAS_VAL
#undef HANDLE_ERROR
#undef source_warn
#undef output_warn
#undef encoder_warn
#undef service_warn
