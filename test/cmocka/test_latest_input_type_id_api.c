/* External API edges for the Rust swap; original characterization stays intact. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <obs.h>
#include <stdint.h>
#include <string.h>

static const char *name(void *data)
{
	(void)data;
	return "Latest source catalog fixture";
}

static int setup(void **state)
{
	(void)state;
	return obs_startup("en-US", NULL, NULL) ? 0 : -1;
}

static int teardown(void **state)
{
	(void)state;
	obs_shutdown();
	return 0;
}

static void full_catalog_and_signed_order(void **state)
{
	(void)state;
	const uint32_t versions[] = {UINT32_C(0x80000000), 0, UINT32_MAX, INT32_MAX, 3};
	for (size_t i = 0; i < sizeof(versions) / sizeof(versions[0]); ++i) {
		struct obs_source_info info = {0};
		info.id = "latest_api_filter";
		info.type = OBS_SOURCE_TYPE_FILTER;
		info.get_name = name;
		info.version = versions[i];
		obs_register_source(&info);
	}
	char query[] = "latest_api_filter";
	const char *expected = NULL, *id;
	for (size_t i = 0; obs_enum_filter_types(i, &id); ++i)
		if (strcmp(id, "latest_api_filter_v2147483647") == 0)
			expected = id;
	assert_non_null(expected);
	assert_ptr_equal(obs_get_latest_input_type_id(query), expected);
	assert_ptr_equal(obs_get_latest_input_type_id("latest_api_filter"), expected);
	assert_null(obs_get_latest_input_type_id(NULL));
#ifdef NDEBUG
	struct obs_source_info high_only = {0};
	high_only.id = "latest_api_high_only";
	high_only.type = OBS_SOURCE_TYPE_FILTER;
	high_only.get_name = name;
	high_only.version = UINT32_MAX;
	obs_register_source(&high_only);
	assert_null(obs_get_latest_input_type_id("latest_api_high_only"));
	assert_null(obs_get_latest_input_type_id("missing_latest_api_type"));
#endif
}

static void null_without_core(void **state)
{
	(void)state;
	assert_null(obs_get_latest_input_type_id(NULL));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_without_core),
		cmocka_unit_test_setup_teardown(full_catalog_and_signed_order, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
