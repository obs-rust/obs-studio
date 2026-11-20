/* Characterize obs_apply_private_data before its separate Rust swap. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <obs.h>

static int setup(void **state)
{
	(void)state;
	assert_true(obs_startup("en-US", NULL, NULL));
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (obs_initialized())
		obs_shutdown();
	return 0;
}

static void null_settings_before_startup(void **state)
{
	(void)state;
	assert_false(obs_initialized());
	obs_apply_private_data(NULL);
	assert_false(obs_initialized());
}

static void apply_to_empty_storage_copies_values(void **state)
{
	(void)state;
	obs_data_t *held = obs_get_private_data();
	assert_non_null(held);
	assert_null(obs_data_first(held));
	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, "name", "original");
	obs_data_set_int(settings, "count", 42);
	obs_apply_private_data(settings);

	obs_data_t *read = obs_get_private_data();
	assert_ptr_equal(read, held);
	assert_ptr_not_equal(read, settings);
	assert_string_equal(obs_data_get_string(read, "name"), "original");
	assert_int_equal(obs_data_get_int(read, "count"), 42);
	obs_data_set_string(settings, "name", "caller changed");
	obs_data_set_int(held, "count", 99);
	assert_string_equal(obs_data_get_string(held, "name"), "original");
	assert_int_equal(obs_data_get_int(settings, "count"), 42);
	obs_data_release(settings);
	assert_string_equal(obs_data_get_string(held, "name"), "original");
	obs_data_release(read);
	obs_data_release(held);
}

static void merge_preserves_storage_and_unrelated_keys(void **state)
{
	(void)state;
	obs_data_t *held = obs_get_private_data();
	obs_data_set_string(held, "old-only", "keep");
	obs_data_set_string(held, "shared", "old");
	obs_data_set_default_int(held, "default-only", 7);
	obs_data_t *settings = obs_data_create();
	obs_data_set_int(settings, "shared", 42);
	obs_data_set_bool(settings, "new-only", true);
	obs_apply_private_data(settings);

	obs_data_t *read = obs_get_private_data();
	assert_ptr_equal(read, held);
	assert_ptr_not_equal(read, settings);
	assert_string_equal(obs_data_get_string(held, "old-only"), "keep");
	assert_int_equal(obs_data_get_int(held, "shared"), 42);
	assert_true(obs_data_get_bool(held, "new-only"));
	assert_int_equal(obs_data_get_int(held, "default-only"), 7);
	assert_false(obs_data_has_user_value(held, "default-only"));
	assert_false(obs_data_has_user_value(settings, "old-only"));
	assert_int_equal(obs_data_get_int(settings, "shared"), 42);
	obs_data_release(settings);
	obs_data_release(read);
	obs_data_release(held);
}

static void null_and_empty_settings_preserve_values(void **state)
{
	(void)state;
	obs_data_t *held = obs_get_private_data();
	obs_data_set_string(held, "keep", "unchanged");
	obs_data_set_default_int(held, "default", 8);
	obs_apply_private_data(NULL);
	assert_string_equal(obs_data_get_string(held, "keep"), "unchanged");
	assert_int_equal(obs_data_get_int(held, "default"), 8);
	obs_data_t *empty = obs_data_create();
	obs_apply_private_data(empty);
	obs_data_release(empty);
	obs_data_t *read = obs_get_private_data();
	assert_ptr_equal(read, held);
	assert_string_equal(obs_data_get_string(read, "keep"), "unchanged");
	assert_int_equal(obs_data_get_int(read, "default"), 8);
	obs_data_release(read);
	obs_data_release(held);
}

static void applied_values_survive_core_shutdown(void **state)
{
	(void)state;
	obs_data_t *held = obs_get_private_data();
	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, "retained", "after shutdown");
	obs_apply_private_data(settings);
	obs_data_release(settings);
	obs_shutdown();
	assert_false(obs_initialized());
	assert_string_equal(obs_data_get_string(held, "retained"), "after shutdown");
	obs_data_release(held);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_settings_before_startup),
		cmocka_unit_test_setup_teardown(apply_to_empty_storage_copies_values, setup, teardown),
		cmocka_unit_test_setup_teardown(merge_preserves_storage_and_unrelated_keys, setup, teardown),
		cmocka_unit_test_setup_teardown(null_and_empty_settings_preserve_values, setup, teardown),
		cmocka_unit_test_setup_teardown(applied_values_survive_core_shutdown, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
