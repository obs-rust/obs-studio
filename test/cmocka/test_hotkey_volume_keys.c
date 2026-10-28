/* Saved hotkey bindings to Volume Up and Down (obs-rust/obs-studio#2635).
 *
 * The X11 and Wayland key tables gave the Volume Up key the name
 * OBS_KEY_VK_VOLUME_DOWN and Volume Down OBS_KEY_VK_VOLUME_UP, so every
 * binding to them that a Linux build saved names the other key. Once the
 * tables are right, such a binding in a scene collection's hotkey data (a
 * source's, as obs_source_create and obs_hotkeys_load_source read it) is
 * swapped once when it loads, so it keeps firing on the key the user
 * pressed, and libobs saves these two keys with "volume_keys_fixed" there.
 * Windows and macOS always had the right names and swap nothing.
 *
 * obs_hotkey_load() and obs_hotkey_save() are not a scene collection's:
 * a plugin or script passes them today's names, so they neither swap nor
 * mark.
 *
 * Links the real libobs (obs_startup). Run under xvfb-run on Linux:
 * obs_startup needs a display there. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <util/dstr.h>

#if defined(__linux__) || defined(__FreeBSD__)
#define SWAPPED_HOST true
#else
#define SWAPPED_HOST false
#endif

#define SOURCE_ID "test_hotkey_volume_keys_input"

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

static const char *source_name(void *type_data)
{
	(void)type_data;
	return "volume keys";
}

static void *source_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	obs_hotkey_register_source(source, "vk.single", "single", noop_hotkey, NULL);
	obs_hotkey_pair_register_source(source, "vk.on", "on", "vk.off", "off", noop_pair, noop_pair, NULL, NULL);
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
		.id = SOURCE_ID,
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

static void assert_array_json(obs_data_array_t *array, const char *expected)
{
	obs_data_t *data = obs_data_create();
	obs_data_set_array(data, "b", array);
	struct dstr json = {0};
	dstr_printf(&json, "{\"b\":%s}", expected);
	const char *saved = obs_data_get_json(data);
	if (strcmp(saved, json.array) != 0)
		fprintf(stderr, "saved:    %s\nexpected: %s\n", saved, json.array);
	assert_string_equal(saved, json.array);
	dstr_free(&json);
	obs_data_release(data);
}

static void assert_source_saves(obs_source_t *source, const char *expected)
{
	obs_data_t *out = obs_hotkeys_save_source(source);
	assert_non_null(out);
	const char *json = obs_data_get_json(out);
	if (strcmp(json, expected) != 0)
		fprintf(stderr, "saved:    %s\nexpected: %s\n", json, expected);
	assert_string_equal(json, expected);
	obs_data_release(out);
}

static obs_source_t *source_from(const char *name, const char *hotkeys_json)
{
	obs_data_t *in = obs_data_create_from_json(hotkeys_json);
	assert_non_null(in);
	obs_source_t *source = obs_source_create(SOURCE_ID, name, NULL, in);
	obs_data_release(in);
	assert_non_null(source);
	return source;
}

/* A source's hotkeys as a Linux build saved them before the fix: the user
 * pressed Volume Up for "single" and "on", and Volume Down (with Shift) for
 * "single" and "off". */
static const char *old_hotkeys =
	"{\"vk.single\":[{\"key\":\"OBS_KEY_VK_VOLUME_DOWN\"},"
	"{\"shift\":true,\"key\":\"OBS_KEY_VK_VOLUME_UP\"},{\"key\":\"OBS_KEY_VK_VOLUME_MUTE\"}],"
	"\"vk.on\":[{\"key\":\"OBS_KEY_VK_VOLUME_DOWN\"}],"
	"\"vk.off\":[{\"key\":\"OBS_KEY_VK_VOLUME_UP\"}]}";

#if SWAPPED_HOST
static const char *migrated_hotkeys = "{\"vk.single\":[{\"key\":\"OBS_KEY_VK_VOLUME_UP\",\"volume_keys_fixed\":true},"
				      "{\"shift\":true,\"key\":\"OBS_KEY_VK_VOLUME_DOWN\",\"volume_keys_fixed\":true},"
				      "{\"key\":\"OBS_KEY_VK_VOLUME_MUTE\"}],"
				      "\"vk.on\":[{\"key\":\"OBS_KEY_VK_VOLUME_UP\",\"volume_keys_fixed\":true}],"
				      "\"vk.off\":[{\"key\":\"OBS_KEY_VK_VOLUME_DOWN\",\"volume_keys_fixed\":true}]}";
#else
static const char *migrated_hotkeys = "{\"vk.single\":[{\"key\":\"OBS_KEY_VK_VOLUME_DOWN\",\"volume_keys_fixed\":true},"
				      "{\"shift\":true,\"key\":\"OBS_KEY_VK_VOLUME_UP\",\"volume_keys_fixed\":true},"
				      "{\"key\":\"OBS_KEY_VK_VOLUME_MUTE\"}],"
				      "\"vk.on\":[{\"key\":\"OBS_KEY_VK_VOLUME_DOWN\",\"volume_keys_fixed\":true}],"
				      "\"vk.off\":[{\"key\":\"OBS_KEY_VK_VOLUME_UP\",\"volume_keys_fixed\":true}]}";
#endif

/* Swapped once on a Linux host (single hotkeys and pairs alike), other keys
 * left alone, and saved with the marker. */
static void source_saved_before_fix(void **state)
{
	(void)state;
	obs_source_t *source = source_from("volume keys 1", old_hotkeys);
	assert_source_saves(source, migrated_hotkeys);
	obs_source_release(source);
}

/* What was saved since the fix loads as it is: the migration runs once. */
static void source_saved_after_fix(void **state)
{
	(void)state;
	obs_source_t *source = source_from("volume keys 2", migrated_hotkeys);
	assert_source_saves(source, migrated_hotkeys);
	obs_source_release(source);
}

/* obs_hotkeys_load_source() reads a scene collection's data too. */
static void source_load_after_create(void **state)
{
	(void)state;
	obs_source_t *source = source_from("volume keys 3", "{}");
	obs_data_t *hotkeys = obs_data_create_from_json(old_hotkeys);
	obs_hotkeys_load_source(source, hotkeys);
	obs_data_release(hotkeys);
	assert_source_saves(source, migrated_hotkeys);
	obs_source_release(source);
}

/* A plugin or script that loads bindings through the API uses today's
 * names: nothing is swapped or marked, on any host. */
static void api_load_and_save_unchanged(void **state)
{
	(void)state;
	const char *json = "[{\"key\":\"OBS_KEY_VK_VOLUME_UP\"},{\"shift\":true,\"key\":\"OBS_KEY_VK_VOLUME_DOWN\"}]";
	obs_hotkey_id id = obs_hotkey_register_frontend("vk.api", "api", noop_hotkey, NULL);
	assert_int_not_equal(id, OBS_INVALID_HOTKEY_ID);

	obs_data_array_t *in = array_from_json(json);
	obs_hotkey_load(id, in);
	obs_data_array_release(in);
	obs_data_array_t *out = obs_hotkey_save(id);
	assert_array_json(out, json);
	obs_data_array_release(out);
	obs_hotkey_unregister(id);

	obs_hotkey_pair_id pair = obs_hotkey_pair_register_frontend("vk.api.on", "on", "vk.api.off", "off", noop_pair,
								    noop_pair, NULL, NULL);
	assert_int_not_equal(pair, OBS_INVALID_HOTKEY_PAIR_ID);
	obs_data_array_t *in0 = array_from_json("[{\"key\":\"OBS_KEY_VK_VOLUME_UP\"}]");
	obs_data_array_t *in1 = array_from_json("[{\"key\":\"OBS_KEY_VK_VOLUME_DOWN\"}]");
	obs_hotkey_pair_load(pair, in0, in1);
	obs_data_array_release(in0);
	obs_data_array_release(in1);
	obs_data_array_t *out0 = NULL, *out1 = NULL;
	obs_hotkey_pair_save(pair, &out0, &out1);
	assert_array_json(out0, "[{\"key\":\"OBS_KEY_VK_VOLUME_UP\"}]");
	assert_array_json(out1, "[{\"key\":\"OBS_KEY_VK_VOLUME_DOWN\"}]");
	obs_data_array_release(out0);
	obs_data_array_release(out1);
	obs_hotkey_pair_unregister(pair);
}

static void no_task(void *param)
{
	(void)param;
}

int main(void)
{
	if (!obs_startup("en-US", NULL, NULL)) {
		puts("obs_startup failed, skipping");
		return 0;
	}
	register_source();

	const struct CMUnitTest tests[] = {
		cmocka_unit_test(source_saved_before_fix),
		cmocka_unit_test(source_saved_after_fix),
		cmocka_unit_test(source_load_after_create),
		cmocka_unit_test(api_load_and_save_unchanged),
	};
	int rc = cmocka_run_group_tests(tests, NULL, NULL);

	/* Sources are destroyed on the destroy task thread. */
	obs_queue_task(OBS_TASK_DESTROY, no_task, NULL, true);
	obs_shutdown();
	return rc;
}
