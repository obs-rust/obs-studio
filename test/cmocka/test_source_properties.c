/* Characterize the public source property constructors before their Rust port.
 * No video/audio is started; each property callback observes applied settings. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdbool.h>
#include <cmocka.h>
#include "cmocka-compat.h"
#include <obs.h>

static int type_token;
static void *seen_data;
static void *seen_type;
static unsigned legacy_calls, modern_calls, defaults_calls, applied_calls;
static long long applied_value;
static bool return_null;

static const char *source_name(void *type_data)
{
	(void)type_data;
	return "source properties characterization";
}

static void *source_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	(void)source;
	return bzalloc(1);
}

static void source_destroy(void *data)
{
	bfree(data);
}

static void defaults(obs_data_t *settings)
{
	defaults_calls++;
	obs_data_set_default_int(settings, "value", 17);
}

static void defaults2(void *type_data, obs_data_t *settings)
{
	assert_ptr_equal(type_data, &type_token);
	defaults_calls++;
	obs_data_set_default_int(settings, "value", 29);
}

static bool modified(obs_properties_t *props, obs_property_t *property, obs_data_t *settings)
{
	(void)props;
	(void)property;
	applied_calls++;
	applied_value = obs_data_get_int(settings, "value");
	return false;
}

static obs_properties_t *make_properties(void)
{
	if (return_null)
		return NULL;
	obs_properties_t *props = obs_properties_create();
	obs_property_t *property = obs_properties_add_int(props, "value", "Value", 0, 100, 1);
	obs_property_set_modified_callback(property, modified);
	return props;
}

static obs_properties_t *legacy(void *data)
{
	legacy_calls++;
	seen_data = data;
	return make_properties();
}

static obs_properties_t *modern(void *data, void *type_data)
{
	modern_calls++;
	seen_data = data;
	seen_type = type_data;
	return make_properties();
}

static void reset_observations(void)
{
	seen_data = &type_token;
	seen_type = NULL;
	legacy_calls = modern_calls = defaults_calls = applied_calls = 0;
	applied_value = -1;
}

static int setup(void **state)
{
	(void)state;
	if (!obs_startup("en-US", NULL, NULL))
		return -1;
	struct obs_source_info info = {
		.id = "source_properties_legacy",
		.type = OBS_SOURCE_TYPE_INPUT,
		.get_name = source_name,
		.create = source_create,
		.destroy = source_destroy,
		.get_defaults = defaults,
		.get_properties = legacy,
		.type_data = &type_token,
	};
	obs_register_source(&info);
	info.id = "source_properties_modern";
	info.get_properties2 = modern;
	info.get_defaults2 = defaults2;
	obs_register_source(&info);
	info.id = "source_properties_modern_only";
	info.get_properties = NULL;
	obs_register_source(&info);
	info.id = "source_properties_none";
	info.get_properties2 = NULL;
	obs_register_source(&info);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	obs_shutdown();
	return 0;
}

static void test_type_properties(void **state)
{
	(void)state;
	const char *ids[] = {"source_properties_legacy", "source_properties_modern", "source_properties_modern_only"};
	for (size_t i = 0; i < 3; i++) {
		reset_observations();
		obs_properties_t *props = obs_get_source_properties(ids[i]);
		assert_non_null(props);
		assert_non_null(obs_properties_get(props, "value"));
		assert_null(seen_data);
		assert_int_equal(legacy_calls, i == 0);
		assert_int_equal(modern_calls, i != 0);
		if (i != 0)
			assert_ptr_equal(seen_type, &type_token);
		assert_int_equal(defaults_calls, 1);
		assert_int_equal(applied_calls, 1);
		assert_int_equal(applied_value, i == 0 ? 17 : 29);
		obs_properties_destroy(props);
	}
}

static void test_instance_properties(void **state)
{
	(void)state;
	const char *ids[] = {"source_properties_legacy", "source_properties_modern", "source_properties_modern_only",
			     "source_properties_none"};
	for (size_t i = 0; i < 4; i++) {
		obs_data_t *settings = obs_data_create();
		obs_data_set_int(settings, "value", 83);
		obs_source_t *source = obs_source_create_private(ids[i], "properties", settings);
		assert_non_null(source);
		reset_observations();
		obs_properties_t *props = obs_source_properties(source);
		assert_int_equal(defaults_calls, 0);
		assert_int_equal(legacy_calls, i == 0);
		assert_int_equal(modern_calls, i == 1 || i == 2);
		assert_int_equal(applied_calls, i != 3);
		if (i != 3) {
			assert_non_null(props);
			assert_ptr_equal(seen_data, obs_obj_get_data(source));
			if (i != 0)
				assert_ptr_equal(seen_type, &type_token);
			assert_int_equal(applied_value, 83);
		} else {
			assert_null(props);
		}
		obs_properties_destroy(props);
		obs_source_release(source);
		obs_data_release(settings);
	}
}

static void test_absent_properties(void **state)
{
	(void)state;
	reset_observations();
	assert_null(obs_get_source_properties("source_properties_unknown"));
	assert_null(obs_get_source_properties("source_properties_none"));
	assert_null(obs_source_properties(NULL));
	assert_int_equal(legacy_calls + modern_calls + defaults_calls + applied_calls, 0);
}

static void test_null_callback_result(void **state)
{
	(void)state;
	const char *ids[] = {"source_properties_legacy", "source_properties_modern"};
	return_null = true;
	for (size_t i = 0; i < 2; i++) {
		reset_observations();
		assert_null(obs_get_source_properties(ids[i]));
		assert_int_equal(defaults_calls, 1);
		assert_int_equal(applied_calls, 0);
		assert_int_equal(legacy_calls, i == 0);
		assert_int_equal(modern_calls, i != 0);
		assert_null(seen_data);
		obs_source_t *source = obs_source_create_private(ids[i], "null-properties", NULL);
		assert_non_null(source);
		reset_observations();
		assert_null(obs_source_properties(source));
		assert_int_equal(defaults_calls + applied_calls, 0);
		assert_int_equal(legacy_calls, i == 0);
		assert_int_equal(modern_calls, i != 0);
		assert_ptr_equal(seen_data, obs_obj_get_data(source));
		obs_source_release(source);
	}
	return_null = false;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_type_properties),
		cmocka_unit_test(test_instance_properties),
		cmocka_unit_test(test_absent_properties),
		cmocka_unit_test(test_null_callback_result),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
