/* Direct characterization of obs_create_disabled_module on actual libobs.
 * Public calls establish ownership, lookup and metadata behavior; private
 * obs_module fields pin zero initialization and list linkage, not a new ABI.
 * No-core/NULL output/NULL binary-path calls are outside the valid domain.
 * OBS_MODULE_ENABLED is not used: the constructor always puts its result on
 * the disabled list, while shutdown uses the state to choose which to unlink.
 * Characterized, not endorsed: duplicate names allocate distinct modules and
 * the newest one wins lookup. No binary is opened or initialized here. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "cmocka-compat.h"
#include <stdio.h>
#include <string.h>
#include <obs-internal.h>
#include <obs-module.h>
#include <util/platform.h>

#if defined(_WIN32)
#define MODULE_EXT ".dll"
#elif defined(__APPLE__)
#define MODULE_EXT ""
#else
#define MODULE_EXT ".so"
#endif

static char data_dir[128];
static char manifest_path[160];

static int setup(void **state)
{
	(void)state;
	snprintf(data_dir, sizeof(data_dir), "disabled-module-%llu", (unsigned long long)os_gettime_ns());
	snprintf(manifest_path, sizeof(manifest_path), "%s/manifest.json", data_dir);
	assert_int_equal(os_mkdirs(data_dir), 0);
	return obs_startup("en-US", NULL, NULL) ? 0 : -1;
}

static int teardown(void **state)
{
	(void)state;
	obs_shutdown();
	if (os_file_exists(manifest_path))
		assert_int_equal(os_unlink(manifest_path), 0);
	assert_int_equal(os_rmdir(data_dir), 0);
	return 0;
}

static obs_module_t *create(const char *path, const char *data, enum obs_module_load_state load_state)
{
	obs_module_t *module = NULL;
	assert_true(obs_create_disabled_module(&module, path, data, load_state));
	assert_non_null(module);
	return module;
}

static void count_loaded(void *param, obs_module_t *module)
{
	(void)module;
	++*(size_t *)param;
}

static void initialized_owned_fields(void **state)
{
	(void)state;
	char path[] = "absent/owned" MODULE_EXT;
	char data[128];
	strcpy(data, data_dir);
	obs_module_t *module = create(path, data, OBS_MODULE_DISABLED_SAFE);
	assert_ptr_not_equal(module->bin_path, path);
	assert_ptr_not_equal(module->data_path, data);
	assert_ptr_equal(module->file, module->bin_path + strlen("absent/"));
	memset(path, 'x', sizeof(path) - 1);
	memset(data, 'y', strlen(data));
	assert_string_equal(obs_get_module_binary_path(module), "absent/owned" MODULE_EXT);
	assert_string_equal(obs_get_module_file_name(module), "owned" MODULE_EXT);
	assert_string_equal(obs_get_module_data_path(module), data_dir);
	assert_string_equal(module->mod_name, "owned");
	assert_ptr_equal(obs_get_disabled_module("owned"), module);
	assert_null(obs_get_module("owned"));
	assert_int_equal(module->load_state, OBS_MODULE_DISABLED_SAFE);
	assert_int_equal(module->module_type, OBS_MODULE_TYPE_UNDEFINED);
	assert_false(module->loaded);
	assert_null(obs_get_module_lib(module));
	assert_null(module->load);
	assert_null(module->unload);
	assert_null(module->post_load);
	assert_null(module->set_locale);
	assert_null(module->get_string);
	assert_null(module->free_locale);
	assert_null(module->ver);
	assert_null(module->set_pointer);
	assert_null(module->name);
	assert_null(module->description);
	assert_null(module->author);
	assert_null(module->metadata);
	assert_null(module->next);
	assert_null(module->sources.array);
	assert_int_equal(module->sources.num, 0);
	assert_int_equal(module->sources.capacity, 0);
	assert_null(module->outputs.array);
	assert_int_equal(module->outputs.num, 0);
	assert_int_equal(module->outputs.capacity, 0);
	assert_null(module->encoders.array);
	assert_int_equal(module->encoders.num, 0);
	assert_int_equal(module->encoders.capacity, 0);
	assert_null(module->services.array);
	assert_int_equal(module->services.num, 0);
	assert_int_equal(module->services.capacity, 0);
}

static void states_duplicates_and_prepend(void **state)
{
	(void)state;
	const enum obs_module_load_state states[] = {OBS_MODULE_INVALID,        OBS_MODULE_MISSING,
						     OBS_MODULE_DISABLED,       OBS_MODULE_DISABLED_SAFE,
						     OBS_MODULE_FAILED_TO_OPEN, OBS_MODULE_FAILED_TO_INITIALIZE};
	obs_module_t *previous = NULL;
	for (size_t i = 0; i < sizeof(states) / sizeof(states[0]); ++i) {
		obs_module_t *module = create("duplicate" MODULE_EXT, data_dir, states[i]);
		assert_ptr_not_equal(module, previous);
		assert_ptr_equal(module->next, previous);
		assert_int_equal(module->load_state, states[i]);
		assert_ptr_equal(obs_get_disabled_module("duplicate"), module);
		assert_null(obs_get_module("duplicate"));
		previous = module;
	}
	size_t loaded = 0;
	obs_enum_modules(count_loaded, &loaded);
	assert_int_equal(loaded, 0);
}

static void metadata_is_loaded_at_creation(void **state)
{
	(void)state;
	FILE *file = fopen(manifest_path, "wb");
	assert_non_null(file);
	assert_true(fputs("{\"display_name\":\"Shown\",\"id\":\"disabled.id\",\"version\":\"1.2\",\"urls\":{}}",
			  file) >= 0);
	assert_int_equal(fclose(file), 0);
	obs_module_t *module = create("manifest" MODULE_EXT, data_dir, OBS_MODULE_DISABLED);
	assert_non_null(module->metadata);
	assert_string_equal(obs_get_module_name(module), "Shown");
	assert_string_equal(obs_get_module_id(module), "disabled.id");
	assert_string_equal(obs_get_module_version(module), "1.2");
	assert_string_equal(module->mod_name, "manifest");
	assert_ptr_equal(obs_get_disabled_module("manifest"), module);
	assert_null(obs_get_disabled_module("Shown"));
	assert_int_equal(os_unlink(manifest_path), 0);
	/* Metadata belongs to the module, independent of the original file. */
	assert_string_equal(obs_get_module_id(module), "disabled.id");
	obs_module_t *missing = create("manifest" MODULE_EXT, data_dir, OBS_MODULE_FAILED_TO_OPEN);
	assert_null(missing->metadata);
	assert_null(obs_get_module_name(missing));
	assert_null(obs_get_module_id(missing));
	assert_null(obs_get_module_version(missing));
	assert_ptr_equal(missing->next, module);
	assert_ptr_equal(obs_get_disabled_module("manifest"), missing);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(initialized_owned_fields, setup, teardown),
		cmocka_unit_test_setup_teardown(states_duplicates_and_prepend, setup, teardown),
		cmocka_unit_test_setup_teardown(metadata_is_loaded_at_creation, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
