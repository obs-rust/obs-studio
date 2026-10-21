/* Characterization of the hotkey binding codec in
 * libobs/obs-hotkey-bindings.c before its port to the obs-hotkey crate.
 *
 * The codec turns one item of a hotkey's saved bindings array into an
 * obs_key_combination_t and back: the "shift", "control", "alt" and
 * "command" bools and the "key" name. It is the on-disk hotkey format of
 * every profile and scene collection, so the expectations below compare the
 * saved JSON byte for byte.
 *
 * test_hotkey_load compiles the C into itself, so it cannot see a Rust swap.
 * This test links the real libobs instead (obs_startup) and goes only
 * through the public API: obs_hotkey_load / obs_hotkey_save,
 * obs_hotkey_load_bindings, obs_hotkey_pair_load / obs_hotkey_pair_save,
 * and a source's hotkey data (obs_source_create, obs_hotkeys_save_source).
 * Run under xvfb-run on Linux: obs_startup needs a display there.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <util/dstr.h>

static int obs_up;
static obs_hotkey_id hotkey;
static size_t changed;

/* ------------------------------------------------------------------------- */

static void noop_hotkey(void *data, obs_hotkey_id id, obs_hotkey_t *key, bool pressed)
{
	(void)data;
	(void)id;
	(void)key;
	(void)pressed;
}

static bool noop_pair(void *data, obs_hotkey_pair_id id, obs_hotkey_t *key, bool pressed)
{
	(void)data;
	(void)id;
	(void)key;
	(void)pressed;
	return false;
}

static void bindings_changed(void *data, calldata_t *cd)
{
	obs_hotkey_t *key = calldata_ptr(cd, "key");
	if (key && obs_hotkey_get_id(key) == *(obs_hotkey_id *)data)
		changed++;
}

/* "[...]" -> an obs_data_array_t, through obs_data's JSON parser as a scene
 * collection or profile is read. */
static obs_data_array_t *array_from_json(const char *array_json)
{
	struct dstr json = {0};
	dstr_printf(&json, "{\"b\":%s}", array_json);
	obs_data_t *data = obs_data_create_from_json(json.array);
	dstr_free(&json);
	assert_non_null(data);

	obs_data_array_t *array = obs_data_get_array(data, "b");
	obs_data_release(data);
	assert_non_null(array);
	return array;
}

/* An obs_data_array_t -> "[...]", byte for byte as obs_data writes it. */
static char *array_to_json(obs_data_array_t *array)
{
	obs_data_t *data = obs_data_create();
	obs_data_set_array(data, "b", array);
	const char *json = obs_data_get_json(data);
	assert_non_null(json);
	assert_true(strncmp(json, "{\"b\":", 5) == 0);
	size_t len = strlen(json);
	assert_true(len >= 6 && json[len - 1] == '}');

	char *result = bstrdup_n(json + 5, len - 6);
	obs_data_release(data);
	return result;
}

static void load(obs_hotkey_id id, const char *array_json)
{
	obs_data_array_t *array = array_from_json(array_json);
	obs_hotkey_load(id, array);
	obs_data_array_release(array);
}

static char *save(obs_hotkey_id id)
{
	obs_data_array_t *array = obs_hotkey_save(id);
	assert_non_null(array);
	char *json = array_to_json(array);
	obs_data_array_release(array);
	return json;
}

static void assert_saved(obs_hotkey_id id, const char *expected)
{
	char *json = save(id);
	if (strcmp(json, expected) != 0)
		fprintf(stderr, "saved:    %s\nexpected: %s\n", json, expected);
	assert_string_equal(json, expected);
	bfree(json);
}

/* load(input), then the saved JSON is `expected`, and loading that saves it
 * again unchanged (save -> load -> save is a fixpoint). */
static void assert_load_saves(const char *input, const char *expected)
{
	load(hotkey, input);
	assert_saved(hotkey, expected);
	load(hotkey, expected);
	assert_saved(hotkey, expected);
}

static int setup(void **state)
{
	(void)state;
	hotkey = obs_hotkey_register_frontend("test.codec", "codec", noop_hotkey, NULL);
	assert_int_not_equal(hotkey, OBS_INVALID_HOTKEY_ID);
	changed = 0;
	signal_handler_connect(obs_get_signal_handler(), "hotkey_bindings_changed", bindings_changed, &hotkey);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	signal_handler_disconnect(obs_get_signal_handler(), "hotkey_bindings_changed", bindings_changed, &hotkey);
	obs_hotkey_unregister(hotkey);
	return 0;
}

/* ------------------------------------------------------------------------- */
/* Modifiers */

static void each_modifier_alone(void **state)
{
	(void)state;
	assert_load_saves("[{\"shift\":true,\"key\":\"OBS_KEY_A\"}]", "[{\"shift\":true,\"key\":\"OBS_KEY_A\"}]");
	assert_load_saves("[{\"control\":true,\"key\":\"OBS_KEY_A\"}]", "[{\"control\":true,\"key\":\"OBS_KEY_A\"}]");
	assert_load_saves("[{\"alt\":true,\"key\":\"OBS_KEY_A\"}]", "[{\"alt\":true,\"key\":\"OBS_KEY_A\"}]");
	assert_load_saves("[{\"command\":true,\"key\":\"OBS_KEY_A\"}]", "[{\"command\":true,\"key\":\"OBS_KEY_A\"}]");
	assert_load_saves("[{\"key\":\"OBS_KEY_A\"}]", "[{\"key\":\"OBS_KEY_A\"}]");
}

/* Saved keys come in a fixed order (shift, control, alt, command, key)
 * whatever order the input had; a false modifier is not written. */
static void modifier_order_and_false(void **state)
{
	(void)state;
	assert_load_saves("[{\"key\":\"OBS_KEY_F1\",\"command\":true,\"alt\":true,\"control\":true,\"shift\":true}]",
			  "[{\"shift\":true,\"control\":true,\"alt\":true,\"command\":true,\"key\":\"OBS_KEY_F1\"}]");
	assert_load_saves("[{\"key\":\"OBS_KEY_F1\",\"alt\":true,\"shift\":true}]",
			  "[{\"shift\":true,\"alt\":true,\"key\":\"OBS_KEY_F1\"}]");
	assert_load_saves(
		"[{\"shift\":false,\"control\":false,\"alt\":false,\"command\":false,\"key\":\"OBS_KEY_F1\"}]",
		"[{\"key\":\"OBS_KEY_F1\"}]");
}

/* Only a JSON true sets a modifier: a string, a number, null, an object or
 * an array does not. Unknown keys in an item are dropped. */
static void modifier_not_a_bool(void **state)
{
	(void)state;
	assert_load_saves("[{\"shift\":\"true\",\"control\":1,\"alt\":1.0,\"command\":null,\"key\":\"OBS_KEY_B\"}]",
			  "[{\"key\":\"OBS_KEY_B\"}]");
	assert_load_saves("[{\"shift\":{},\"control\":[],\"Shift\":true,\"ctrl\":true,\"key\":\"OBS_KEY_B\"}]",
			  "[{\"key\":\"OBS_KEY_B\"}]");
}

/* All 16 modifier sets on one key, in one array, in order. */
static void all_modifier_sets(void **state)
{
	(void)state;
	static const char *names[] = {"shift", "control", "alt", "command"};
	struct dstr json = {0};
	dstr_copy(&json, "[");
	for (int set = 0; set < 16; set++) {
		dstr_cat(&json, set ? ",{" : "{");
		for (int m = 0; m < 4; m++) {
			if (set & (1 << m))
				dstr_catf(&json, "\"%s\":true,", names[m]);
		}
		dstr_cat(&json, "\"key\":\"OBS_KEY_Z\"}");
	}
	dstr_cat(&json, "]");
	assert_load_saves(json.array, json.array);
	dstr_free(&json);
}

/* ------------------------------------------------------------------------- */
/* Keys */

/* Every key name round-trips, MOUSE1 and MOUSE2 aside (rejected below), and
 * OBS_KEY_NONE, which alone is no binding (dropped below). The array is built
 * the way the codec writes it, so it must load and save to the same bytes. */
static void every_key_name(void **state)
{
	(void)state;
	struct dstr json = {0};
	size_t count = 0;
	dstr_copy(&json, "[");
	for (int key = OBS_KEY_NONE; key < OBS_KEY_LAST_VALUE; key++) {
		if (key == OBS_KEY_NONE || key == OBS_KEY_MOUSE1 || key == OBS_KEY_MOUSE2)
			continue;
		const char *name = obs_key_to_name((obs_key_t)key);
		assert_non_null(name);
		assert_true(*name != '\0');
		assert_int_equal(obs_key_from_name(name), key);
		dstr_catf(&json, "%s{\"%skey\":\"%s\"}", count ? "," : "", (key % 2) ? "shift\":true,\"" : "", name);
		count++;
	}
	dstr_cat(&json, "]");
	assert_load_saves(json.array, json.array);
	dstr_free(&json);
}

/* An item with no modifiers and no known key is no binding: it could never
 * fire (handle_binding() resets a modifier-less OBS_KEY_NONE), so
 * load_binding() drops it. An unknown, empty, missing or non-string "key",
 * {}, and the {"key":"OBS_KEY_NONE"} that older builds saved for these, load
 * as nothing and are not saved back. */
static void unknown_or_missing_key_dropped(void **state)
{
	(void)state;
	const char *none = "[]";
	assert_load_saves("[{\"key\":\"OBS_KEY_NOT_A_KEY\"}]", none);
	assert_load_saves("[{\"key\":\"obs_key_a\"}]", none);
	assert_load_saves("[{\"key\":\" OBS_KEY_A\"}]", none);
	assert_load_saves("[{\"key\":\"OBS_KEY_A \"}]", none);
	assert_load_saves("[{\"key\":\"A\"}]", none);
	assert_load_saves("[{\"key\":\"\"}]", none);
	assert_load_saves("[{\"key\":65}]", none);
	assert_load_saves("[{\"key\":true}]", none);
	assert_load_saves("[{\"key\":null}]", none);
	assert_load_saves("[{\"key\":{}}]", none);
	assert_load_saves("[{\"key\":\"OBS_KEY_NONE\"}]", none);
	assert_load_saves("[{}]", none);
	assert_load_saves("[{\"unrelated\":1}]", none);
}

/* A modifier-only binding (key OBS_KEY_NONE) is a real binding: an unknown
 * or missing key with modifiers keeps the modifiers. */
static void modifiers_only(void **state)
{
	(void)state;
	assert_load_saves("[{\"shift\":true}]", "[{\"shift\":true,\"key\":\"OBS_KEY_NONE\"}]");
	assert_load_saves("[{\"control\":true,\"alt\":true,\"key\":\"bogus\"}]",
			  "[{\"control\":true,\"alt\":true,\"key\":\"OBS_KEY_NONE\"}]");
	assert_load_saves("[{\"command\":true,\"key\":\"OBS_KEY_NONE\"}]",
			  "[{\"command\":true,\"key\":\"OBS_KEY_NONE\"}]");
}

/* ------------------------------------------------------------------------- */
/* Mouse buttons (ours): primary buttons are reserved for UI interaction. */

static void primary_mouse_buttons_rejected(void **state)
{
	(void)state;
	load(hotkey, "[{\"key\":\"OBS_KEY_MOUSE1\"},"
		     "{\"key\":\"OBS_KEY_A\"},"
		     "{\"shift\":true,\"key\":\"OBS_KEY_MOUSE2\"},"
		     "{\"shift\":true,\"control\":true,\"alt\":true,\"command\":true,\"key\":\"OBS_KEY_MOUSE1\"},"
		     "{\"key\":\"OBS_KEY_MOUSE3\"},"
		     "{\"control\":true,\"key\":\"OBS_KEY_MOUSE29\"}]");
	assert_saved(hotkey, "[{\"key\":\"OBS_KEY_A\"},"
			     "{\"key\":\"OBS_KEY_MOUSE3\"},"
			     "{\"control\":true,\"key\":\"OBS_KEY_MOUSE29\"}]");
}

/* A load whose every item is rejected still replaces the old bindings. */
static void rejected_load_clears(void **state)
{
	(void)state;
	load(hotkey, "[{\"key\":\"OBS_KEY_A\"}]");
	load(hotkey, "[{\"key\":\"OBS_KEY_MOUSE1\"},{\"alt\":true,\"key\":\"OBS_KEY_MOUSE2\"}]");
	assert_saved(hotkey, "[]");
}

/* The same rule for API bindings (obs_hotkey_load_bindings). */
static void api_primary_mouse_buttons_rejected(void **state)
{
	(void)state;
	obs_key_combination_t combos[] = {
		{0, OBS_KEY_MOUSE1},
		{INTERACT_SHIFT_KEY, OBS_KEY_MOUSE2},
		{INTERACT_CONTROL_KEY | INTERACT_COMMAND_KEY, OBS_KEY_C},
		{0, OBS_KEY_MOUSE3},
	};
	obs_hotkey_load_bindings(hotkey, combos, 4);
	assert_saved(hotkey,
		     "[{\"control\":true,\"command\":true,\"key\":\"OBS_KEY_C\"},{\"key\":\"OBS_KEY_MOUSE3\"}]");
}

/* Only the four modifier bits are saved; other bits of an API binding are
 * dropped. */
static void api_other_modifier_bits_dropped(void **state)
{
	(void)state;
	obs_key_combination_t combos[] = {
		{0xFFFFFFFFu, OBS_KEY_D},
		{INTERACT_CAPS_KEY | INTERACT_MOUSE_LEFT, OBS_KEY_E},
	};
	obs_hotkey_load_bindings(hotkey, combos, 2);
	assert_saved(
		hotkey,
		"[{\"shift\":true,\"control\":true,\"alt\":true,\"command\":true,\"key\":\"OBS_KEY_D\"},{\"key\":\"OBS_KEY_E\"}]");
}

/* ------------------------------------------------------------------------- */
/* Arrays */

/* Duplicates are kept and the order is the load order. */
static void duplicates_and_order(void **state)
{
	(void)state;
	const char *json = "[{\"key\":\"OBS_KEY_A\"},{\"key\":\"OBS_KEY_A\"},{\"shift\":true,\"key\":\"OBS_KEY_B\"},"
			   "{\"key\":\"OBS_KEY_A\"},{\"key\":\"OBS_KEY_NUM9\"},{\"key\":\"OBS_KEY_1\"},"
			   "{\"shift\":true,\"key\":\"OBS_KEY_B\"}]";
	assert_load_saves(json, json);
}

/* Items that are not objects are dropped by the parser before the codec;
 * the objects keep their order. */
static void non_object_items(void **state)
{
	(void)state;
	assert_load_saves(
		"[1,\"OBS_KEY_A\",null,true,[],{\"key\":\"OBS_KEY_G\"},[{\"key\":\"OBS_KEY_H\"}],{\"key\":\"OBS_KEY_I\"}]",
		"[{\"key\":\"OBS_KEY_G\"},{\"key\":\"OBS_KEY_I\"}]");
}

/* A load replaces the hotkey's bindings, and other hotkeys keep theirs. */
static void load_replaces_only_its_hotkey(void **state)
{
	(void)state;
	obs_hotkey_id other = obs_hotkey_register_frontend("test.codec.other", "other", noop_hotkey, NULL);
	load(other, "[{\"key\":\"OBS_KEY_O\"}]");
	load(hotkey, "[{\"key\":\"OBS_KEY_A\"},{\"key\":\"OBS_KEY_B\"}]");
	load(hotkey, "[{\"key\":\"OBS_KEY_C\"}]");
	assert_saved(hotkey, "[{\"key\":\"OBS_KEY_C\"}]");
	assert_saved(other, "[{\"key\":\"OBS_KEY_O\"}]");
	obs_hotkey_unregister(other);
}

/* A hotkey with no bindings saves an empty array; an unknown id saves
 * nothing. */
static void empty_save(void **state)
{
	(void)state;
	assert_saved(hotkey, "[]");
	assert_null(obs_hotkey_save(OBS_INVALID_HOTKEY_ID));
}

/* An empty (or missing) array clears the bindings and sends
 * hotkey_bindings_changed when it removed any, as obs_hotkey_load_bindings()
 * does; clearing a hotkey with no bindings sends nothing. A non-empty array
 * signals even when every item is rejected. */
static void empty_array_clears_with_signal(void **state)
{
	(void)state;
	load(hotkey, "[{\"key\":\"OBS_KEY_A\"}]");
	assert_int_equal(changed, 1);

	load(hotkey, "[]");
	assert_saved(hotkey, "[]");
	assert_int_equal(changed, 2);
	load(hotkey, "[]");
	assert_int_equal(changed, 2);

	load(hotkey, "[{\"key\":\"OBS_KEY_A\"}]");
	assert_int_equal(changed, 3);
	obs_hotkey_load(hotkey, NULL);
	assert_saved(hotkey, "[]");
	assert_int_equal(changed, 4);
	obs_hotkey_load(hotkey, NULL);
	assert_int_equal(changed, 4);

	load(hotkey, "[{\"key\":\"OBS_KEY_MOUSE1\"}]");
	assert_int_equal(changed, 5);

	load(hotkey, "[{\"key\":\"OBS_KEY_A\"}]");
	assert_int_equal(changed, 6);
	obs_hotkey_load_bindings(hotkey, NULL, 0);
	assert_saved(hotkey, "[]");
	assert_int_equal(changed, 7);
	obs_hotkey_load_bindings(hotkey, NULL, 0);
	assert_int_equal(changed, 7);
}

/* ------------------------------------------------------------------------- */
/* Pairs and sources: the other public entry points into the codec. */

static void pair_round_trip(void **state)
{
	(void)state;
	obs_hotkey_pair_id pair = obs_hotkey_pair_register_frontend("test.codec.on", "on", "test.codec.off", "off",
								    noop_pair, noop_pair, NULL, NULL);
	assert_int_not_equal(pair, OBS_INVALID_HOTKEY_PAIR_ID);

	obs_data_array_t *in0 = array_from_json("[{\"alt\":true,\"key\":\"OBS_KEY_1\"},{\"key\":\"OBS_KEY_MOUSE2\"}]");
	obs_data_array_t *in1 = array_from_json("[{\"command\":true,\"key\":\"OBS_KEY_2\"},{}]");
	obs_hotkey_pair_load(pair, in0, in1);
	obs_data_array_release(in0);
	obs_data_array_release(in1);

	obs_data_array_t *out0 = NULL, *out1 = NULL;
	obs_hotkey_pair_save(pair, &out0, &out1);
	char *json0 = array_to_json(out0);
	char *json1 = array_to_json(out1);
	assert_string_equal(json0, "[{\"alt\":true,\"key\":\"OBS_KEY_1\"}]");
	assert_string_equal(json1, "[{\"command\":true,\"key\":\"OBS_KEY_2\"}]");
	bfree(json0);
	bfree(json1);
	obs_data_array_release(out0);
	obs_data_array_release(out1);

	obs_hotkey_pair_unregister(pair);
}

static const char *source_name(void *type_data)
{
	(void)type_data;
	return "hotkey codec";
}

static void *source_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	obs_hotkey_register_source(source, "codec.first", "first", noop_hotkey, NULL);
	obs_hotkey_register_source(source, "codec.second", "second", noop_hotkey, NULL);
	return source;
}

static void source_destroy(void *data)
{
	(void)data;
}

static uint32_t source_size(void *data)
{
	(void)data;
	return 16;
}

static void register_source(void)
{
	struct obs_source_info info = {
		.id = "test_hotkey_bindings_codec_input",
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = source_name,
		.create = source_create,
		.destroy = source_destroy,
		.get_width = source_size,
		.get_height = source_size,
	};
	obs_register_source(&info);
}

static char *source_saved(const char *name, const char *hotkeys_json)
{
	obs_data_t *in = obs_data_create_from_json(hotkeys_json);
	assert_non_null(in);
	obs_source_t *source = obs_source_create("test_hotkey_bindings_codec_input", name, NULL, in);
	obs_data_release(in);
	assert_non_null(source);

	obs_data_t *out = obs_hotkeys_save_source(source);
	assert_non_null(out);
	char *json = bstrdup(obs_data_get_json(out));
	obs_data_release(out);
	obs_source_release(source);
	return json;
}

/* A source's hotkeys, as a scene collection stores them: an object of one
 * array per hotkey name, loaded when the source registers each hotkey
 * (obs_source_create's hotkey data) and saved in registration order. A name
 * the source has no hotkey for is dropped, and a missing name saves an empty
 * array. A source made from the saved data saves the same bytes. */
static void source_round_trip(void **state)
{
	(void)state;
	const char *expected = "{\"codec.first\":[{\"shift\":true,\"key\":\"OBS_KEY_F5\"},{\"key\":\"OBS_KEY_F6\"}],"
			       "\"codec.second\":[{\"control\":true,\"command\":true,\"key\":\"OBS_KEY_NONE\"}]}";

	char *first = source_saved(
		"codec source 1",
		"{\"codec.second\":[{\"command\":true,\"control\":true,\"key\":\"nope\"}],"
		"\"codec.unknown\":[{\"key\":\"OBS_KEY_U\"}],"
		"\"codec.first\":[{\"shift\":true,\"key\":\"OBS_KEY_F5\"},{\"key\":\"OBS_KEY_MOUSE1\"},{\"key\":\"OBS_KEY_F6\"}]}");
	assert_string_equal(first, expected);

	char *second = source_saved("codec source 2", first);
	assert_string_equal(second, expected);

	char *partial = source_saved("codec source 3", "{\"codec.second\":[{\"key\":\"OBS_KEY_S\"}]}");
	assert_string_equal(partial, "{\"codec.first\":[],\"codec.second\":[{\"key\":\"OBS_KEY_S\"}]}");

	bfree(first);
	bfree(second);
	bfree(partial);
}

/* ------------------------------------------------------------------------- */

static void no_task(void *param)
{
	(void)param;
}

int main(void)
{
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		puts("obs_startup failed, skipping");
		return 0;
	}
	register_source();

	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(each_modifier_alone, setup, teardown),
		cmocka_unit_test_setup_teardown(modifier_order_and_false, setup, teardown),
		cmocka_unit_test_setup_teardown(modifier_not_a_bool, setup, teardown),
		cmocka_unit_test_setup_teardown(all_modifier_sets, setup, teardown),
		cmocka_unit_test_setup_teardown(every_key_name, setup, teardown),
		cmocka_unit_test_setup_teardown(unknown_or_missing_key_dropped, setup, teardown),
		cmocka_unit_test_setup_teardown(modifiers_only, setup, teardown),
		cmocka_unit_test_setup_teardown(primary_mouse_buttons_rejected, setup, teardown),
		cmocka_unit_test_setup_teardown(rejected_load_clears, setup, teardown),
		cmocka_unit_test_setup_teardown(api_primary_mouse_buttons_rejected, setup, teardown),
		cmocka_unit_test_setup_teardown(api_other_modifier_bits_dropped, setup, teardown),
		cmocka_unit_test_setup_teardown(duplicates_and_order, setup, teardown),
		cmocka_unit_test_setup_teardown(non_object_items, setup, teardown),
		cmocka_unit_test_setup_teardown(load_replaces_only_its_hotkey, setup, teardown),
		cmocka_unit_test_setup_teardown(empty_save, setup, teardown),
		cmocka_unit_test_setup_teardown(empty_array_clears_with_signal, setup, teardown),
		cmocka_unit_test_setup_teardown(pair_round_trip, setup, teardown),
		cmocka_unit_test_setup_teardown(source_round_trip, setup, teardown),
	};
	int rc = cmocka_run_group_tests(tests, NULL, NULL);

	/* Private sources are destroyed on the destroy task thread. */
	obs_queue_task(OBS_TASK_DESTROY, no_task, NULL, true);
	obs_shutdown();
	return rc;
}
