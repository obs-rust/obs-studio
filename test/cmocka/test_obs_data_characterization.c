#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "cmocka-compat.h"

#include <obs-data.h>
#include <util/bmem.h>
#include <util/platform.h>

/* The deprecated calls are still exported; their behavior is pinned before
 * the port like every other export. */
#if defined(_MSC_VER)
#pragma warning(disable : 4996)
#else
#pragma GCC diagnostic ignored "-Wdeprecated-declarations"
#endif

/*
 * Characterization tests for libobs/obs-data.c.h, covering what
 * test_obs_data.c does not: double/bool/obj/array defaults and autoselects,
 * typed item getters, array enumeration, pretty JSON with defaults and the
 * pretty safe save. They pin CURRENT C behavior.
 */

#define TMP_DIR "test_obs_data_characterization.tmp"
#define PRETTY_PATH TMP_DIR "/pretty.json"
#define PRETTY_TMP TMP_DIR "/pretty.json.tmp"
#define PRETTY_BAK TMP_DIR "/pretty.json.bak"

static void remove_test_files(void)
{
	os_unlink(PRETTY_PATH);
	os_unlink(PRETTY_TMP);
	os_unlink(PRETTY_BAK);
}

static int group_setup(void **state)
{
	UNUSED_PARAMETER(state);

	remove_test_files();
	return os_mkdirs(TMP_DIR) == MKDIR_ERROR ? -1 : 0;
}

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);

	remove_test_files();
	os_rmdir(TMP_DIR);
	return 0;
}

static void test_typed_defaults(void **state)
{
	obs_data_t *data = obs_data_create();
	obs_data_t *obj = obs_data_create();
	obs_data_array_t *arr = obs_data_array_create();
	obs_data_t *got_obj;
	obs_data_array_t *got_arr;

	UNUSED_PARAMETER(state);

	obs_data_set_int(obj, "k", 7);
	obs_data_array_push_back(arr, obj);

	obs_data_set_default_double(data, "d", 2.5);
	obs_data_set_default_bool(data, "b", true);
	obs_data_set_default_obj(data, "o", obj);
	obs_data_set_default_array(data, "a", arr);
	obs_data_set_default_int(data, "i", 3);

	assert_true(obs_data_has_default_value(data, "d"));
	assert_true(obs_data_has_default_value(data, "b"));
	assert_true(obs_data_has_default_value(data, "o"));
	assert_true(obs_data_has_default_value(data, "a"));
	assert_false(obs_data_has_user_value(data, "d"));
	assert_false(obs_data_has_autoselect_value(data, "d"));
	assert_false(obs_data_has_default_value(data, "missing"));
	assert_false(obs_data_has_default_value(NULL, "d"));

	assert_true(obs_data_get_default_double(data, "d") == 2.5);
	assert_true(obs_data_get_default_bool(data, "b"));
	/* number defaults convert between int and double */
	assert_true(obs_data_get_default_double(data, "i") == 3.0);
	assert_int_equal(obs_data_get_default_int(data, "d"), 2);

	got_obj = obs_data_get_default_obj(data, "o");
	assert_ptr_equal(got_obj, obj);
	obs_data_release(got_obj);

	got_arr = obs_data_get_default_array(data, "a");
	assert_ptr_equal(got_arr, arr);
	assert_int_equal(obs_data_array_count(got_arr), 1);
	obs_data_array_release(got_arr);

	/* wrong-type and missing getters fall back to zero values */
	assert_true(obs_data_get_default_double(data, "b") == 0.0);
	assert_false(obs_data_get_default_bool(data, "d"));
	assert_null(obs_data_get_default_obj(data, "a"));
	assert_null(obs_data_get_default_array(data, "o"));
	assert_string_equal(obs_data_get_default_string(data, "d"), "");
	assert_true(obs_data_get_default_double(data, "missing") == 0.0);
	assert_false(obs_data_get_default_bool(NULL, "b"));

	/* a user value overrides the default for the user getter only */
	obs_data_set_double(data, "d", 9.0);
	assert_true(obs_data_get_double(data, "d") == 9.0);
	assert_true(obs_data_get_default_double(data, "d") == 2.5);

	obs_data_array_release(arr);
	obs_data_release(obj);
	obs_data_release(data);
}

static void test_typed_autoselect(void **state)
{
	obs_data_t *data = obs_data_create();
	obs_data_t *obj = obs_data_create();
	obs_data_array_t *arr = obs_data_array_create();
	obs_data_t *got_obj;
	obs_data_array_t *got_arr;

	UNUSED_PARAMETER(state);

	obs_data_set_autoselect_double(data, "d", 1.25);
	obs_data_set_autoselect_bool(data, "b", true);
	obs_data_set_autoselect_obj(data, "o", obj);
	obs_data_set_autoselect_array(data, "a", arr);

	assert_true(obs_data_has_autoselect_value(data, "d"));
	assert_true(obs_data_has_autoselect_value(data, "b"));
	assert_true(obs_data_has_autoselect_value(data, "o"));
	assert_true(obs_data_has_autoselect_value(data, "a"));
	assert_false(obs_data_has_default_value(data, "d"));
	assert_false(obs_data_has_autoselect_value(data, "missing"));
	assert_false(obs_data_has_autoselect_value(NULL, "d"));

	assert_true(obs_data_get_autoselect_double(data, "d") == 1.25);
	assert_int_equal(obs_data_get_autoselect_int(data, "d"), 1);
	assert_true(obs_data_get_autoselect_bool(data, "b"));

	got_obj = obs_data_get_autoselect_obj(data, "o");
	assert_ptr_equal(got_obj, obj);
	obs_data_release(got_obj);

	got_arr = obs_data_get_autoselect_array(data, "a");
	assert_ptr_equal(got_arr, arr);
	obs_data_array_release(got_arr);

	assert_false(obs_data_get_autoselect_bool(data, "d"));
	assert_null(obs_data_get_autoselect_obj(data, "a"));
	assert_null(obs_data_get_autoselect_array(data, "o"));
	assert_true(obs_data_get_autoselect_double(data, "missing") == 0.0);

	/* default and autoselect stay separate on one item */
	obs_data_set_default_double(data, "d", 4.0);
	assert_true(obs_data_get_default_double(data, "d") == 4.0);
	assert_true(obs_data_get_autoselect_double(data, "d") == 1.25);

	obs_data_array_release(arr);
	obs_data_release(obj);
	obs_data_release(data);
}

static void test_item_getters(void **state)
{
	obs_data_t *data = obs_data_create();
	obs_data_t *obj = obs_data_create();
	obs_data_array_t *arr = obs_data_array_create();
	obs_data_item_t *item;
	obs_data_t *got_obj;
	obs_data_array_t *got_arr;

	UNUSED_PARAMETER(state);

	obs_data_set_double(data, "d", 1.5);
	obs_data_set_bool(data, "b", true);
	obs_data_set_obj(data, "o", obj);
	obs_data_set_array(data, "a", arr);
	obs_data_set_default_bool(data, "b", false);
	obs_data_set_autoselect_bool(data, "b", true);

	item = obs_data_item_byname(data, "d");
	assert_non_null(item);
	assert_int_equal(obs_data_item_gettype(item), OBS_DATA_NUMBER);
	assert_int_equal(obs_data_item_numtype(item), OBS_DATA_NUM_DOUBLE);
	assert_true(obs_data_item_get_double(item) == 1.5);
	assert_int_equal(obs_data_item_get_int(item), 1);
	assert_false(obs_data_item_get_bool(item));
	assert_string_equal(obs_data_item_get_string(item), "");
	assert_null(obs_data_item_get_obj(item));
	assert_null(obs_data_item_get_array(item));
	assert_false(obs_data_item_has_default_value(item));
	assert_false(obs_data_item_has_autoselect_value(item));
	obs_data_item_release(&item);

	item = obs_data_item_byname(data, "b");
	assert_int_equal(obs_data_item_gettype(item), OBS_DATA_BOOLEAN);
	assert_true(obs_data_item_get_bool(item));
	assert_false(obs_data_item_get_default_bool(item));
	assert_true(obs_data_item_get_autoselect_bool(item));
	assert_true(obs_data_item_has_default_value(item));
	assert_true(obs_data_item_has_autoselect_value(item));
	assert_true(obs_data_item_get_double(item) == 0.0);
	obs_data_item_release(&item);

	item = obs_data_item_byname(data, "o");
	assert_int_equal(obs_data_item_gettype(item), OBS_DATA_OBJECT);
	got_obj = obs_data_item_get_obj(item);
	assert_ptr_equal(got_obj, obj);
	obs_data_release(got_obj);
	assert_null(obs_data_item_get_default_obj(item));
	assert_null(obs_data_item_get_autoselect_obj(item));
	obs_data_item_release(&item);

	item = obs_data_item_byname(data, "a");
	assert_int_equal(obs_data_item_gettype(item), OBS_DATA_ARRAY);
	got_arr = obs_data_item_get_array(item);
	assert_ptr_equal(got_arr, arr);
	obs_data_array_release(got_arr);
	assert_null(obs_data_item_get_default_array(item));
	assert_null(obs_data_item_get_autoselect_array(item));
	obs_data_item_release(&item);

	/* NULL items read as zero values */
	assert_string_equal(obs_data_item_get_string(NULL), "");
	assert_int_equal(obs_data_item_get_int(NULL), 0);
	assert_true(obs_data_item_get_double(NULL) == 0.0);
	assert_false(obs_data_item_get_bool(NULL));
	assert_null(obs_data_item_get_obj(NULL));
	assert_null(obs_data_item_get_array(NULL));
	assert_false(obs_data_item_has_user_value(NULL));
	assert_false(obs_data_item_has_default_value(NULL));
	assert_false(obs_data_item_has_autoselect_value(NULL));

	obs_data_array_release(arr);
	obs_data_release(obj);
	obs_data_release(data);
}

struct enum_ctx {
	long long seen[4];
	size_t count;
};

static void enum_cb(obs_data_t *data, void *param)
{
	struct enum_ctx *ctx = param;

	if (ctx->count < 4)
		ctx->seen[ctx->count] = obs_data_get_int(data, "v");
	ctx->count++;
}

static void test_array_enum(void **state)
{
	obs_data_array_t *arr = obs_data_array_create();
	struct enum_ctx ctx = {{0}, 0};

	UNUSED_PARAMETER(state);

	for (long long i = 0; i < 3; i++) {
		obs_data_t *obj = obs_data_create();
		obs_data_set_int(obj, "v", i * 10);
		obs_data_array_push_back(arr, obj);
		obs_data_release(obj);
	}

	obs_data_array_enum(arr, enum_cb, &ctx);
	assert_int_equal(ctx.count, 3);
	assert_int_equal(ctx.seen[0], 0);
	assert_int_equal(ctx.seen[1], 10);
	assert_int_equal(ctx.seen[2], 20);

	/* NULL array or callback is a no-op */
	ctx.count = 0;
	obs_data_array_enum(NULL, enum_cb, &ctx);
	obs_data_array_enum(arr, NULL, &ctx);
	assert_int_equal(ctx.count, 0);

	obs_data_array_release(arr);
}

static void test_pretty_json_with_defaults(void **state)
{
	obs_data_t *data = obs_data_create();
	const char *json;
	obs_data_t *parsed;

	UNUSED_PARAMETER(state);

	assert_null(obs_data_get_json_pretty_with_defaults(NULL));

	obs_data_set_int(data, "user", 1);
	obs_data_set_default_int(data, "def", 2);

	json = obs_data_get_json_pretty(data);
	assert_non_null(json);
	assert_null(strstr(json, "\"def\""));

	json = obs_data_get_json_pretty_with_defaults(data);
	assert_non_null(json);
	assert_ptr_equal(obs_data_get_last_json(data), json);
	assert_non_null(strchr(json, '\n'));
	assert_non_null(strstr(json, "    \"user\""));
	assert_non_null(strstr(json, "\"def\""));

	parsed = obs_data_create_from_json(json);
	assert_non_null(parsed);
	assert_int_equal(obs_data_get_int(parsed, "user"), 1);
	assert_int_equal(obs_data_get_int(parsed, "def"), 2);
	assert_true(obs_data_has_user_value(parsed, "def"));
	obs_data_release(parsed);

	obs_data_release(data);
}

static void test_save_json_pretty_safe(void **state)
{
	obs_data_t *data = obs_data_create();
	obs_data_t *loaded;
	char *text;

	UNUSED_PARAMETER(state);

	assert_false(obs_data_save_json_pretty_safe(NULL, PRETTY_PATH, "tmp", "bak"));

	obs_data_set_string(data, "name", "first");
	if (!obs_data_save_json_pretty_safe(data, PRETTY_PATH, "tmp", "bak")) {
		fprintf(stderr, "first pretty safe save failed\n");
		fail();
	}

	text = os_quick_read_utf8_file(PRETTY_PATH);
	assert_non_null(text);
	assert_non_null(strchr(text, '\n'));
	assert_non_null(strstr(text, "    \"name\""));
	bfree(text);

	obs_data_set_string(data, "name", "second");
	assert_true(obs_data_save_json_pretty_safe(data, PRETTY_PATH, "tmp", "bak"));

	loaded = obs_data_create_from_json_file(PRETTY_PATH);
	assert_non_null(loaded);
	assert_string_equal(obs_data_get_string(loaded, "name"), "second");
	obs_data_release(loaded);

	/* the previous file is kept as the backup */
	loaded = obs_data_create_from_json_file(PRETTY_BAK);
	assert_non_null(loaded);
	assert_string_equal(obs_data_get_string(loaded, "name"), "first");
	obs_data_release(loaded);

	assert_false(os_file_exists(PRETTY_TMP));

	obs_data_release(data);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_typed_defaults),
		cmocka_unit_test(test_typed_autoselect),
		cmocka_unit_test(test_item_getters),
		cmocka_unit_test(test_array_enum),
		cmocka_unit_test(test_pretty_json_with_defaults),
		cmocka_unit_test(test_save_json_pretty_safe),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
