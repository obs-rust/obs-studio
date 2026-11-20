/* Characterize the C traversal and the linked API before a separate Rust swap.
 * Registration rejects equal-version duplicates, so test-owned registry entries
 * pin that traversal edge directly. The extracted C is compiled verbatim. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "libobs/build/libobs.pch.h"

static struct obs_core *latest_input_test_core;
#define obs latest_input_test_core
#define obs_get_latest_input_type_id characterized_latest_input_type_id
#include "libobs/obs-get-latest-input-type-id.c.h"
#undef obs_get_latest_input_type_id
#undef obs

static void null_id_without_core(void **state)
{
	(void)state;
	latest_input_test_core = NULL;
	assert_null(characterized_latest_input_type_id(NULL));
	assert_null(obs_get_latest_input_type_id(NULL));
}

static void newest_and_borrowed_pointer(void **state)
{
	(void)state;
	char newest[] = "camera_v9";
	struct obs_source_info entries[] = {
		{.id = "camera", .unversioned_id = "camera", .version = 0},
		{.id = "other_v99", .unversioned_id = "other", .version = 99},
		{.id = newest, .unversioned_id = "camera", .version = 9},
		{.id = "camera_v3", .unversioned_id = "camera", .version = 3},
	};
	struct obs_core core = {0};
	core.source_types.array = entries;
	core.source_types.num = sizeof(entries) / sizeof(entries[0]);
	latest_input_test_core = &core;
	const char *result = characterized_latest_input_type_id("camera");
	assert_ptr_equal(result, newest);
	assert_ptr_equal(characterized_latest_input_type_id("camera"), result);
	newest[0] = 'C';
	assert_string_equal(result, "Camera_v9");
	assert_ptr_equal(core.source_types.array, entries);
	assert_int_equal(core.source_types.num, 4);
	assert_null(characterized_latest_input_type_id(NULL));
	latest_input_test_core = NULL;
}

static void ties_keep_first_registered(void **state)
{
	(void)state;
	char first[] = "first", second[] = "second";
	struct obs_source_info entries[] = {
		{.id = first, .unversioned_id = "camera", .version = 7},
		{.id = second, .unversioned_id = "camera", .version = 7},
	};
	struct obs_core core = {0};
	core.source_types.array = entries;
	core.source_types.num = sizeof(entries) / sizeof(entries[0]);
	latest_input_test_core = &core;
	assert_ptr_equal(characterized_latest_input_type_id("camera"), first);
	struct obs_source_info swap = entries[0];
	entries[0] = entries[1];
	entries[1] = swap;
	assert_ptr_equal(characterized_latest_input_type_id("camera"), second);
	latest_input_test_core = NULL;
}

static void signed_versions(void **state)
{
	(void)state;
	struct obs_source_info entries[] = {
		{.id = "negative", .unversioned_id = "camera", .version = UINT32_C(0x80000000)},
		{.id = "zero", .unversioned_id = "camera", .version = 0},
		{.id = "sentinel", .unversioned_id = "camera", .version = UINT32_MAX},
		{.id = "maximum", .unversioned_id = "camera", .version = INT32_MAX},
		{.id = "negative-last", .unversioned_id = "camera", .version = UINT32_C(0x80000001)},
	};
	struct obs_core core = {0};
	core.source_types.array = entries;
	core.source_types.num = 3;
	latest_input_test_core = &core;
	/* Characterized, not endorsed: high-bit versions compare as signed int. */
	assert_ptr_equal(characterized_latest_input_type_id("camera"), entries[1].id);
	core.source_types.num = sizeof(entries) / sizeof(entries[0]);
	assert_ptr_equal(characterized_latest_input_type_id("camera"), entries[3].id);
	latest_input_test_core = NULL;
}

#ifdef NDEBUG
static void missing_id_without_assertions(void **state)
{
	(void)state;
	struct obs_core core = {0};
	latest_input_test_core = &core;
	/* A missing non-NULL ID asserts in debug. Exercise only the release path. */
	assert_null(characterized_latest_input_type_id("missing"));
	struct obs_source_info entry = {.id = "camera", .unversioned_id = "camera", .version = 0};
	core.source_types.array = &entry;
	core.source_types.num = 1;
	assert_null(characterized_latest_input_type_id("missing"));
	entry.version = UINT32_MAX;
	assert_null(characterized_latest_input_type_id("camera"));
	latest_input_test_core = NULL;
}
#endif

static const char *get_name(void *data)
{
	(void)data;
	return "Latest input type fixture";
}

static int setup(void **state)
{
	(void)state;
	return obs_startup("en-US", NULL, NULL) ? 0 : -1;
}

static int teardown(void **state)
{
	(void)state;
	/* Registration owns the versioned ID copies; no instances are created. */
	obs_shutdown();
	return 0;
}

static void linked_registry_returns_borrowed_latest(void **state)
{
	(void)state;
	const char *id;
	size_t start = 0;
	while (obs_enum_input_types(start, &id))
		start++;
	const uint32_t versions[] = {0, 9, 3};
	for (size_t i = 0; i < sizeof(versions) / sizeof(versions[0]); i++) {
		struct obs_source_info info = {0};
		info.id = "latest_input_fixture";
		info.type = OBS_SOURCE_TYPE_INPUT;
		info.get_name = get_name;
		info.version = versions[i];
		obs_register_source(&info);
	}
	assert_true(obs_enum_input_types(start + 1, &id));
	assert_string_equal(id, "latest_input_fixture_v9");
	assert_ptr_equal(obs_get_latest_input_type_id("latest_input_fixture"), id);
	assert_ptr_equal(obs_get_latest_input_type_id("latest_input_fixture"), id);
	assert_null(obs_get_latest_input_type_id(NULL));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_id_without_core),
		cmocka_unit_test(newest_and_borrowed_pointer),
		cmocka_unit_test(ties_keep_first_registered),
		cmocka_unit_test(signed_versions),
#ifdef NDEBUG
		cmocka_unit_test(missing_id_without_assertions),
#endif
		cmocka_unit_test_setup_teardown(linked_registry_returns_borrowed_latest, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
