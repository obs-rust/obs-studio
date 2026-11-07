/* Actual-library characterization of the read-only obs_module getters (#3866).
 * Private stack fixtures pin borrowed identity and callback behavior without
 * loading a plugin. No fixture memory is owned by libobs. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <obs-internal.h>
#include <obs-module.h>

static unsigned calls;
static const char *result;

static const char *metadata_callback(void)
{
	++calls;
	return result;
}

static void null_and_empty(void **state)
{
	(void)state;
	struct obs_module empty = {0};
	obs_module_t *modules[] = {NULL, &empty};
	for (size_t i = 0; i < 2; ++i) {
		assert_null(obs_get_module_file_name(modules[i]));
		assert_null(obs_get_module_name(modules[i]));
		assert_null(obs_get_module_author(modules[i]));
		assert_null(obs_get_module_description(modules[i]));
		assert_null(obs_get_module_binary_path(modules[i]));
		assert_null(obs_get_module_data_path(modules[i]));
		assert_null(obs_get_module_id(modules[i]));
		assert_null(obs_get_module_version(modules[i]));
		assert_null(obs_get_module_lib(modules[i]));
	}
}

static void borrowed_fields(void **state)
{
	(void)state;
	char bytes[] = {'x', (char)0xff, 0};
	char empty[] = "";
	struct obs_module_metadata metadata = {.id = bytes, .version = empty};
	struct obs_module module = {.file = bytes,
				    .bin_path = bytes,
				    .data_path = empty,
				    .module = &metadata,
				    .metadata = &metadata};
	assert_ptr_equal(obs_get_module_file_name(&module), bytes);
	assert_ptr_equal(obs_get_module_binary_path(&module), bytes);
	assert_ptr_equal(obs_get_module_data_path(&module), empty);
	assert_ptr_equal(obs_get_module_id(&module), bytes);
	assert_ptr_equal(obs_get_module_version(&module), empty);
	assert_ptr_equal(obs_get_module_lib(&module), &metadata);
	metadata.id = NULL;
	metadata.version = NULL;
	assert_null(obs_get_module_id(&module));
	assert_null(obs_get_module_version(&module));
}

static void callbacks_and_precedence(void **state)
{
	(void)state;
	char display[] = "";
	char value[] = "callback";
	struct obs_module_metadata metadata = {.display_name = display, .description = "ignored"};
	struct obs_module module = {.name = metadata_callback,
				    .author = metadata_callback,
				    .description = metadata_callback};
	calls = 0;
	result = value;
	assert_ptr_equal(obs_get_module_name(&module), value);
	assert_ptr_equal(obs_get_module_author(&module), value);
	assert_ptr_equal(obs_get_module_description(&module), value);
	assert_int_equal(calls, 3);
	module.metadata = &metadata;
	assert_ptr_equal(obs_get_module_name(&module), display);
	assert_int_equal(calls, 3);
	/* Characterized: even an empty display name suppresses the callback. */
	metadata.display_name = NULL;
	assert_ptr_equal(obs_get_module_name(&module), value);
	assert_int_equal(calls, 4);
	result = NULL;
	assert_null(obs_get_module_name(&module));
	assert_null(obs_get_module_author(&module));
	assert_null(obs_get_module_description(&module));
	assert_int_equal(calls, 7);
	module.name = NULL;
	module.author = NULL;
	module.description = NULL;
	assert_null(obs_get_module_name(&module));
	assert_null(obs_get_module_author(&module));
	assert_null(obs_get_module_description(&module));
	assert_int_equal(calls, 7);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(null_and_empty), cmocka_unit_test(borrowed_fields),
					   cmocka_unit_test(callbacks_and_precedence)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
