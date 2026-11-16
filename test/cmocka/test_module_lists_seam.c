/*
 * The module-list seam (libobs/obs-module-lists-seam.h,
 * obs-rust/obs-studio#4219), which the port of the module list calls
 * (libobs/obs-module-lists.c) calls in place of the struct reads and the
 * da_push_back() targets it wraps:
 * - the core: whether obs is set, first_module, module_config_path, and the
 *   darrays of module_paths, safe_modules and disabled_modules;
 * - a module: next, file, data_path and mod_name, and the darrays of
 *   sources, outputs, encoders and services.
 *
 * Each read is checked against the field it wraps, and each darray address
 * against the typed list behind it: an element pushed through the address
 * with darray_push_back() shows up in the typed list, and one pushed with
 * da_push_back() on the typed list shows up through the address. The
 * module's lists are also checked against libobs's own obs_module_add_*()
 * calls, which take the module they push onto.
 *
 * The accessors are hidden in libobs, so this test compiles the seam into
 * itself and runs it on a stand-in core (`obs`, with only the fields the seam
 * reads) and stand-in modules: zeroed structs.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdint.h>
#include <string.h>

#include "obs-internal.h"
#include "obs-module-lists-seam.h"

/* libobs has this one in obs.c, hidden; the seam in this test reads this
 * stand-in. */
struct obs_core *obs;

static int setup(void **state)
{
	obs = bzalloc(sizeof(*obs));
	(void)state;
	return 0;
}

static void free_strings(struct darray *list)
{
	char **items = list->array;
	for (size_t i = 0; i < list->num; i++)
		bfree(items[i]);
	darray_free(list);
}

static int teardown(void **state)
{
	for (size_t i = 0; i < obs->module_paths.num; i++)
		free_module_path(&obs->module_paths.array[i]);
	da_free(obs->module_paths);
	free_strings(&obs->safe_modules.da);
	free_strings(&obs->disabled_modules.da);
	bfree(obs);
	obs = NULL;
	(void)state;
	return 0;
}

static void test_has_core(void **state)
{
	(void)state;
	assert_true(obs_module_lists_seam_has_core());

	struct obs_core *core = obs;
	obs = NULL;
	assert_false(obs_module_lists_seam_has_core());
	obs = core;
	assert_true(obs_module_lists_seam_has_core());
}

static void test_first_module(void **state)
{
	(void)state;
	struct obs_module module = {0};

	assert_null(obs_module_lists_seam_first_module());
	obs->first_module = &module;
	assert_ptr_equal(obs_module_lists_seam_first_module(), &module);
	obs->first_module = NULL;
	assert_null(obs_module_lists_seam_first_module());
}

static void test_module_config_path(void **state)
{
	(void)state;
	char path[] = "config/plugin_config";

	assert_null(obs_module_lists_seam_module_config_path());
	obs->module_config_path = path;
	/* The field itself, not a copy. */
	assert_ptr_equal(obs_module_lists_seam_module_config_path(), path);
	obs->module_config_path = NULL;
	assert_null(obs_module_lists_seam_module_config_path());
}

static void test_module_paths(void **state)
{
	(void)state;
	struct darray *list = obs_module_lists_seam_module_paths();
	assert_ptr_equal(list, &obs->module_paths.da);

	/* Through the address, as the port pushes: the typed list has it. */
	struct obs_module_path first = {
		.bin = bstrdup("bin/a"),
		.data = bstrdup("data/a"),
		.type = OBS_MODULE_TYPE_UNDEFINED,
	};
	darray_push_back(sizeof(struct obs_module_path), list, &first);
	assert_int_equal(obs->module_paths.num, 1);
	assert_string_equal(obs->module_paths.array[0].bin, "bin/a");
	assert_string_equal(obs->module_paths.array[0].data, "data/a");
	assert_int_equal(obs->module_paths.array[0].type, OBS_MODULE_TYPE_UNDEFINED);

	/* On the typed list, as obs_add_module_path() pushes: the address sees
	 * it. */
	struct obs_module_path second = {
		.bin = bstrdup("bin/b"),
		.data = bstrdup("data/b"),
		.type = OBS_MODULE_TYPE_UNDEFINED,
	};
	da_push_back(obs->module_paths, &second);
	assert_ptr_equal(obs_module_lists_seam_module_paths(), &obs->module_paths.da);
	assert_int_equal(list->num, 2);
	struct obs_module_path *paths = list->array;
	assert_string_equal(paths[1].bin, "bin/b");
	assert_string_equal(paths[1].data, "data/b");
}

static void check_string_list(struct darray *list, size_t *typed_num, char ***typed_array)
{
	char *first = bstrdup("first");
	darray_push_back(sizeof(char *), list, &first);
	assert_int_equal(*typed_num, 1);
	assert_ptr_equal((*typed_array)[0], first);

	char *second = bstrdup("second");
	darray_push_back(sizeof(char *), list, &second);
	assert_int_equal(*typed_num, 2);
	assert_ptr_equal((*typed_array)[1], second);
	assert_string_equal((*typed_array)[1], "second");
}

static void test_safe_modules(void **state)
{
	(void)state;
	struct darray *list = obs_module_lists_seam_safe_modules();
	assert_ptr_equal(list, &obs->safe_modules.da);
	check_string_list(list, &obs->safe_modules.num, &obs->safe_modules.array);

	char *typed = bstrdup("typed");
	da_push_back(obs->safe_modules, &typed);
	assert_int_equal(obs_module_lists_seam_safe_modules()->num, 3);
	assert_ptr_equal(((char **)list->array)[2], typed);
}

static void test_disabled_modules(void **state)
{
	(void)state;
	struct darray *list = obs_module_lists_seam_disabled_modules();
	assert_ptr_equal(list, &obs->disabled_modules.da);
	check_string_list(list, &obs->disabled_modules.num, &obs->disabled_modules.array);

	char *typed = bstrdup("typed");
	da_push_back(obs->disabled_modules, &typed);
	assert_int_equal(obs_module_lists_seam_disabled_modules()->num, 3);
	assert_ptr_equal(((char **)list->array)[2], typed);
}

/* The core lists are three distinct arrays. */
static void test_core_lists_distinct(void **state)
{
	(void)state;
	assert_ptr_not_equal(obs_module_lists_seam_module_paths(), obs_module_lists_seam_safe_modules());
	assert_ptr_not_equal(obs_module_lists_seam_safe_modules(), obs_module_lists_seam_disabled_modules());
	assert_ptr_not_equal(obs_module_lists_seam_module_paths(), obs_module_lists_seam_disabled_modules());
}

static void test_module_next(void **state)
{
	(void)state;
	struct obs_module second = {0};
	struct obs_module first = {.next = &second};

	assert_ptr_equal(obs_module_lists_seam_module_next(&first), &second);
	assert_null(obs_module_lists_seam_module_next(&second));
}

static void test_module_strings(void **state)
{
	(void)state;
	char name[] = "test-module";
	char file[] = "plugins/test-module.so";
	char data[] = "data/plugins/test-module";
	struct obs_module module = {
		.mod_name = name,
		.file = file,
		.data_path = data,
	};

	/* The fields themselves, not copies. */
	assert_ptr_equal(obs_module_lists_seam_module_file(&module), file);
	assert_ptr_equal(obs_module_lists_seam_module_data_path(&module), data);
	assert_ptr_equal(obs_module_lists_seam_module_name(&module), name);

	struct obs_module empty = {0};
	assert_null(obs_module_lists_seam_module_file(&empty));
	assert_null(obs_module_lists_seam_module_data_path(&empty));
	assert_null(obs_module_lists_seam_module_name(&empty));
}

typedef struct darray *(*module_list_fn)(obs_module_t *module);
typedef void (*module_add_fn)(obs_module_t *module, const char *id);

/* One module list: the address is the typed list's darray, a push through it
 * shows in the typed list, and libobs's own obs_module_add_*() pushes where
 * the address points. */
static void check_module_list(struct obs_module *module, module_list_fn seam, module_add_fn add, struct darray *field)
{
	struct darray *list = seam(module);
	assert_ptr_equal(list, field);

	char *id = bstrdup("through_seam");
	darray_push_back(sizeof(char *), list, &id);
	assert_int_equal(field->num, 1);
	assert_ptr_equal(((char **)field->array)[0], id);

	add(module, "through_libobs");
	assert_ptr_equal(seam(module), field);
	assert_int_equal(list->num, 2);
	assert_string_equal(((char **)list->array)[1], "through_libobs");

	/* obs_module_add_*(module, NULL) pushes a NULL. */
	add(module, NULL);
	assert_int_equal(list->num, 3);
	assert_null(((char **)list->array)[2]);

	free_strings(list);
}

static void test_module_sources(void **state)
{
	(void)state;
	struct obs_module module = {0};
	check_module_list(&module, obs_module_lists_seam_module_sources, obs_module_add_source, &module.sources.da);
}

static void test_module_outputs(void **state)
{
	(void)state;
	struct obs_module module = {0};
	check_module_list(&module, obs_module_lists_seam_module_outputs, obs_module_add_output, &module.outputs.da);
}

static void test_module_encoders(void **state)
{
	(void)state;
	struct obs_module module = {0};
	check_module_list(&module, obs_module_lists_seam_module_encoders, obs_module_add_encoder, &module.encoders.da);
}

static void test_module_services(void **state)
{
	(void)state;
	struct obs_module module = {0};
	check_module_list(&module, obs_module_lists_seam_module_services, obs_module_add_service, &module.services.da);
}

/* The module lists are four distinct arrays. */
static void test_module_lists_distinct(void **state)
{
	(void)state;
	struct obs_module module = {0};
	struct darray *lists[] = {
		obs_module_lists_seam_module_sources(&module),
		obs_module_lists_seam_module_outputs(&module),
		obs_module_lists_seam_module_encoders(&module),
		obs_module_lists_seam_module_services(&module),
	};
	for (size_t i = 0; i < 4; i++) {
		assert_non_null(lists[i]);
		for (size_t j = i + 1; j < 4; j++)
			assert_ptr_not_equal(lists[i], lists[j]);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(test_has_core, setup, teardown),
		cmocka_unit_test_setup_teardown(test_first_module, setup, teardown),
		cmocka_unit_test_setup_teardown(test_module_config_path, setup, teardown),
		cmocka_unit_test_setup_teardown(test_module_paths, setup, teardown),
		cmocka_unit_test_setup_teardown(test_safe_modules, setup, teardown),
		cmocka_unit_test_setup_teardown(test_disabled_modules, setup, teardown),
		cmocka_unit_test_setup_teardown(test_core_lists_distinct, setup, teardown),
		cmocka_unit_test_setup_teardown(test_module_next, setup, teardown),
		cmocka_unit_test_setup_teardown(test_module_strings, setup, teardown),
		cmocka_unit_test_setup_teardown(test_module_sources, setup, teardown),
		cmocka_unit_test_setup_teardown(test_module_outputs, setup, teardown),
		cmocka_unit_test_setup_teardown(test_module_encoders, setup, teardown),
		cmocka_unit_test_setup_teardown(test_module_services, setup, teardown),
		cmocka_unit_test_setup_teardown(test_module_lists_distinct, setup, teardown),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
