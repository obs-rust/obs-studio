/* Actual-library characterization of read-only module registry queries. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

static const char *loaded_fixture_path;

static void module_name_lookups(void **state)
{
	(void)state;
	assert_true(obs_startup("en-US", NULL, NULL));
	obs_module_t *nodes[4];
	struct obs_module before[4];
	char bytes[] = {'x', (char)0xff, 0};
	char *names[] = {"alpha", "", "alpha", bytes};
	char *original_names[4];
	assert_null(obs_get_module("alpha"));
	assert_null(obs_get_disabled_module("alpha"));
	for (size_t i = 4; i > 0; --i) {
		assert_true(obs_create_disabled_module(&nodes[i - 1], "lookup-fixture.so", "", OBS_MODULE_DISABLED));
		original_names[i - 1] = nodes[i - 1]->mod_name;
		nodes[i - 1]->mod_name = names[i - 1];
	}
	for (size_t i = 0; i < 4; ++i)
		memcpy(&before[i], nodes[i], sizeof(before[i]));
	assert_ptr_equal(obs_get_disabled_module("alpha"), nodes[0]);
	assert_ptr_equal(obs_get_disabled_module(""), nodes[1]);
	assert_ptr_equal(obs_get_disabled_module(bytes), nodes[3]);
	assert_null(obs_get_disabled_module("Alpha"));
	assert_null(obs_get_disabled_module("alph"));
	assert_null(obs_get_disabled_module("missing"));
	assert_ptr_equal(obs_get_disabled_module("alpha"), nodes[0]);
	/* The loaded and disabled registries must remain independent. */
	assert_null(obs_get_module("alpha"));
	assert_null(obs_get_module(""));
	assert_null(obs_get_module(bytes));
	for (size_t i = 0; i < 4; ++i) {
		assert_memory_equal(nodes[i], &before[i], sizeof(before[i]));
		nodes[i]->mod_name = original_names[i];
	}
	if (loaded_fixture_path) {
		for (size_t i = 4; i > 0; --i) {
			assert_int_equal(obs_open_module(&nodes[i - 1], loaded_fixture_path, ""), MODULE_SUCCESS);
			original_names[i - 1] = nodes[i - 1]->mod_name;
			nodes[i - 1]->mod_name = names[i - 1];
		}
		for (size_t i = 0; i < 4; ++i)
			memcpy(&before[i], nodes[i], sizeof(before[i]));
		assert_ptr_equal(obs_get_module("alpha"), nodes[0]);
		assert_ptr_equal(obs_get_module(""), nodes[1]);
		assert_ptr_equal(obs_get_module(bytes), nodes[3]);
		assert_null(obs_get_module("Alpha"));
		assert_null(obs_get_module("alph"));
		assert_null(obs_get_module("missing"));
		assert_ptr_equal(obs_get_module("alpha"), nodes[0]);
		for (size_t i = 0; i < 4; ++i) {
			assert_memory_equal(nodes[i], &before[i], sizeof(before[i]));
			nodes[i]->mod_name = original_names[i];
		}
	}
	obs_shutdown();
}

int main(int argc, char **argv)
{
	loaded_fixture_path = argc > 1 ? argv[1] : NULL;
	const struct CMUnitTest tests[] = {cmocka_unit_test(module_name_lookups)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
