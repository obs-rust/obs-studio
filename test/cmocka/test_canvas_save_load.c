/* Characterize obs_save_canvas() and obs_load_canvas() before the Rust swap
 * (Refs #5127). test_canvas_rules_callers::constructor_flags/save_rules pin
 * the flag masks, the private roundtrip and the ephemeral/removed refusal;
 * this adds the key order, missing keys, the uuid passthrough and the int64
 * truncation of high and negative saved flags. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <string.h>
#include "obs-internal.h"

#if !__has_include("obs-canvas-save-load.c.h")
#error "canvas save/load extraction seam is missing"
#endif

#define LOADED_UUID "5127c0de-0b5d-4c41-9a00-5a7e10adca01"
#define MAIN_UUID "6c69626f-6273-4c00-9d88-c5136d61696e"

static void expect_keys(obs_data_t *data)
{
	static const char *const keys[] = {"name", "uuid", "private", "flags"};
	obs_data_item_t *item = obs_data_first(data);
	for (size_t i = 0; i < sizeof(keys) / sizeof(keys[0]); i++) {
		assert_non_null(item);
		assert_string_equal(obs_data_item_get_name(item), keys[i]);
		obs_data_item_next(&item);
	}
	assert_null(item);
}

static void save_key_order(void **unused)
{
	(void)unused;
	obs_canvas_t *canvas = obs_canvas_create("save load order", NULL, ACTIVATE | SCENE_REF);
	assert_non_null(canvas);
	obs_data_t *data = obs_save_canvas(canvas);
	assert_non_null(data);
	expect_keys(data);
	assert_string_equal(obs_data_get_string(data, "name"), "save load order");
	assert_string_equal(obs_data_get_string(data, "uuid"), obs_canvas_get_uuid(canvas));
	assert_false(obs_data_get_bool(data, "private"));
	assert_int_equal(obs_data_get_int(data, "flags"), ACTIVATE | SCENE_REF);
	obs_data_release(data);
	obs_canvas_release(canvas);

	/* characterized, not endorsed: the main canvas saves, MAIN bit included */
	obs_canvas_t *main_canvas = obs_get_main_canvas();
	data = obs_save_canvas(main_canvas);
	assert_non_null(data);
	expect_keys(data);
	assert_string_equal(obs_data_get_string(data, "uuid"), MAIN_UUID);
	assert_int_equal(obs_data_get_int(data, "flags"), MAIN | PROGRAM);
	obs_data_release(data);
	obs_canvas_release(main_canvas);
	obs_wait_for_destroy_queue();
}

static void save_refusals(void **unused)
{
	(void)unused;
	obs_canvas_t *ephemeral = obs_canvas_create("save load ephemeral", NULL, EPHEMERAL | ACTIVATE);
	obs_canvas_t *removed = obs_canvas_create_private("save load removed", NULL, SCENE_REF);
	assert_non_null(ephemeral);
	assert_non_null(removed);
	assert_null(obs_save_canvas(ephemeral));
	obs_canvas_remove(removed);
	assert_null(obs_save_canvas(removed));
	obs_canvas_remove(ephemeral);
	assert_null(obs_save_canvas(ephemeral));
	obs_canvas_release(ephemeral);
	obs_canvas_release(removed);
	obs_wait_for_destroy_queue();
}

static void uuid_passthrough(void **unused)
{
	(void)unused;
	obs_data_t *data = obs_data_create();
	obs_data_set_string(data, "name", "save load uuid");
	obs_data_set_string(data, "uuid", LOADED_UUID);
	obs_data_set_bool(data, "private", true);
	obs_data_set_int(data, "flags", SCENE_REF);
	obs_canvas_t *canvas = obs_load_canvas(data);
	obs_data_release(data);
	assert_non_null(canvas);
	assert_string_equal(obs_canvas_get_uuid(canvas), LOADED_UUID);
	assert_string_equal(obs_canvas_get_name(canvas), "save load uuid");
	assert_true(canvas->context.private);
	assert_null(canvas->mix);
	assert_int_equal(obs_canvas_get_flags(canvas), SCENE_REF);

	/* The roundtrip writes the same uuid back. */
	data = obs_save_canvas(canvas);
	assert_non_null(data);
	assert_string_equal(obs_data_get_string(data, "uuid"), LOADED_UUID);
	assert_true(obs_data_get_bool(data, "private"));
	obs_data_release(data);
	obs_canvas_release(canvas);
	obs_wait_for_destroy_queue();
}

static void missing_keys(void **unused)
{
	(void)unused;
	obs_data_t *data = obs_data_create();
	obs_canvas_t *canvas = obs_load_canvas(data);
	obs_data_release(data);
	assert_non_null(canvas);
	/* The empty name gets a generated one, then a fresh uuid, public and no
	 * flags. */
	assert_int_equal(strncmp(obs_canvas_get_name(canvas), "__unnamed", 9), 0);
	const char *uuid = obs_canvas_get_uuid(canvas);
	assert_non_null(uuid);
	assert_int_equal(strlen(uuid), 36);
	assert_false(canvas->context.private);
	assert_null(canvas->mix);
	assert_int_equal(obs_canvas_get_flags(canvas), 0);
	obs_canvas_remove(canvas);
	obs_canvas_release(canvas);
	obs_wait_for_destroy_queue();
}

static void saved_flags_truncate(void **unused)
{
	(void)unused;
	/* characterized, not endorsed: the int64 is cut to uint32 before the
	 * public mask, so high bits vanish and a negative value keeps its low
	 * word */
	const long long saved[] = {
		(1LL << 32) | EPHEMERAL,
		INT64_MIN | MAIN | ACTIVATE,
		-(long long)(1u << 31),
		(long long)(0xffffffff00000000ULL | SCENE_REF | (1u << 31)),
	};
	const uint32_t kept[] = {EPHEMERAL, ACTIVATE, 0, SCENE_REF};
	for (size_t i = 0; i < sizeof(saved) / sizeof(saved[0]); i++) {
		obs_data_t *data = obs_data_create();
		obs_data_set_string(data, "name", "save load flags");
		obs_data_set_bool(data, "private", true);
		obs_data_set_int(data, "flags", saved[i]);
		obs_canvas_t *canvas = obs_load_canvas(data);
		obs_data_release(data);
		assert_non_null(canvas);
		assert_int_equal(obs_canvas_get_flags(canvas), kept[i]);
		assert_false(obs_canvas_removed(canvas));
		obs_canvas_release(canvas);
	}
	obs_wait_for_destroy_queue();
}

int main(void)
{
	if (!obs_startup("en-US", NULL, NULL))
		return 77;
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(save_key_order),       cmocka_unit_test(save_refusals),
		cmocka_unit_test(uuid_passthrough),     cmocka_unit_test(missing_keys),
		cmocka_unit_test(saved_flags_truncate),
	};
	int result = cmocka_run_group_tests(tests, NULL, NULL);
	obs_wait_for_destroy_queue();
	obs_shutdown();
	return result;
}
