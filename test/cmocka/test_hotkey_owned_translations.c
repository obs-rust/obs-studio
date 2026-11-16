/* Public C characterization of owned audio/scene-item hotkey descriptions.
 * Keep unchanged when these setters are replaced by Rust. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <obs.h>
#include <obs-hotkey.h>
#include <string.h>
#include <stdio.h>

#define PROBE_ID "owned_translation_probe"

static const char *probe_name(void *unused)
{
	(void)unused;
	return "translation probe";
}

static void *probe_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void probe_destroy(void *data)
{
	(void)data;
}

static int setup(void **state)
{
	(void)state;
	assert_true(obs_startup("en-US", NULL, NULL));
	struct obs_source_info info = {
		.id = PROBE_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_AUDIO,
		.get_name = probe_name,
		.create = probe_create,
		.destroy = probe_destroy,
	};
	obs_register_source(&info);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	while (obs_wait_for_destroy_queue())
		;
	obs_shutdown();
	return 0;
}

struct expected_hotkey {
	obs_weak_source_t *owner;
	const char *name;
	const char *description;
	size_t found;
};

static bool check_hotkey(void *data, obs_hotkey_id id, obs_hotkey_t *key)
{
	(void)id;
	struct expected_hotkey *expected = data;
	if (obs_hotkey_get_registerer_type(key) != OBS_HOTKEY_REGISTERER_SOURCE ||
	    obs_hotkey_get_registerer(key) != expected->owner || strcmp(obs_hotkey_get_name(key), expected->name) != 0)
		return true;
	const char *description = obs_hotkey_get_description(key);
	if (expected->description)
		assert_string_equal(description, expected->description);
	else
		assert_null(description);
	expected->found++;
	return true;
}

static void expect_description(obs_source_t *source, const char *name, const char *description)
{
	struct expected_hotkey expected = {obs_source_get_weak_source(source), name, description, 0};
	obs_enum_hotkeys(check_hotkey, &expected);
	obs_weak_source_release(expected.owner);
	assert_int_equal(expected.found, 1);
}

static obs_source_t *new_audio(void)
{
	obs_source_t *source = obs_source_create(PROBE_ID, "probe", NULL, NULL);
	assert_non_null(source);
	return source;
}

static void expect_audio(obs_source_t *source, const char *const descriptions[6])
{
	static const char *names[] = {"libobs.mute",         "libobs.unmute",     "libobs.push-to-mute",
				      "libobs.push-to-talk", "libobs.monitor-on", "libobs.monitor-off"};
	for (size_t i = 0; i < 6; i++)
		expect_description(source, names[i], descriptions[i]);
}

static void expect_scene(const char *show, const char *hide)
{
	obs_source_t *source = new_audio();
	obs_source_set_name(source, "item");
	obs_scene_t *scene = obs_scene_create("translations scene");
	assert_non_null(scene);
	obs_sceneitem_t *item = obs_scene_add(scene, source);
	assert_non_null(item);
	char name[80];
	snprintf(name, sizeof(name), "libobs.show_scene_item.%lld", (long long)obs_sceneitem_get_id(item));
	expect_description(obs_scene_get_source(scene), name, show);
	snprintf(name, sizeof(name), "libobs.hide_scene_item.%lld", (long long)obs_sceneitem_get_id(item));
	expect_description(obs_scene_get_source(scene), name, hide);
	obs_source_remove(obs_scene_get_source(scene));
	obs_scene_release(scene);
	obs_source_release(source);
	while (obs_wait_for_destroy_queue())
		;
}

static void audio_copies_and_replaces(void **state)
{
	(void)state;
	char labels[6][32] = {"Mute", "Unmute", "Push mute", "Push talk", "Monitor on", "Monitor off"};
	const char *first[] = {"Mute", "Unmute", "Push mute", "Push talk", "Monitor on", "Monitor off"};
	obs_hotkeys_set_audio_hotkeys_translations(labels[0], labels[1], labels[2], labels[3], labels[4], labels[5]);
	memset(labels, 'x', sizeof(labels));
	obs_source_t *old = new_audio();
	expect_audio(old, first);
	obs_hotkeys_set_audio_hotkeys_translations("M2", "U2", "PM2", "PT2", "ON2", "OFF2");
	obs_source_t *replacement = new_audio();
	const char *second[] = {"M2", "U2", "PM2", "PT2", "ON2", "OFF2"};
	expect_audio(replacement, second);
	/* Existing registrations retain their descriptions. */
	expect_audio(old, first);
	obs_source_release(replacement);
	obs_source_release(old);
}

static void scene_copies_replaces_and_formats(void **state)
{
	(void)state;
	char show[] = "Show %1 / %1";
	char hide[] = "Hide %1";
	obs_hotkeys_set_sceneitem_hotkeys_translations(show, hide);
	memset(show, 'x', sizeof(show));
	memset(hide, 'y', sizeof(hide));
	expect_scene("Show item / item", "Hide item");
	obs_hotkeys_set_sceneitem_hotkeys_translations("Visible %1", "Hidden %1");
	expect_scene("Visible item", "Hidden item");
}

static void null_empty_and_group_isolation(void **state)
{
	(void)state;
	obs_hotkeys_set_audio_hotkeys_translations("M", "U", "PM", "PT", "ON", "OFF");
	obs_hotkeys_set_sceneitem_hotkeys_translations("Show %1", "Hide %1");
	obs_hotkeys_set_audio_hotkeys_translations(NULL, "", NULL, "", NULL, "");
	obs_source_t *source = new_audio();
	const char *mixed[] = {NULL, "", NULL, "", NULL, ""};
	expect_audio(source, mixed);
	expect_scene("Show item", "Hide item");
	obs_hotkeys_set_sceneitem_hotkeys_translations(NULL, "");
	expect_scene(NULL, NULL);
	obs_source_t *second = new_audio();
	expect_audio(second, mixed);
	obs_hotkeys_set_audio_hotkeys_translations(NULL, NULL, NULL, NULL, NULL, NULL);
	const char *cleared[] = {NULL, NULL, NULL, NULL, NULL, NULL};
	obs_source_t *nulls = new_audio();
	expect_audio(nulls, cleared);
	obs_hotkeys_set_audio_hotkeys_translations("", "", "", "", "", "");
	const char *empty[] = {"", "", "", "", "", ""};
	obs_source_t *empties = new_audio();
	expect_audio(empties, empty);
	obs_source_release(empties);
	obs_source_release(nulls);
	obs_hotkeys_set_audio_hotkeys_translations("M", "U", "PM", "PT", "ON", "OFF");
	const char *restored[] = {"M", "U", "PM", "PT", "ON", "OFF"};
	obs_source_t *third = new_audio();
	expect_audio(third, restored);
	obs_hotkeys_set_sceneitem_hotkeys_translations("Restored %1", "Hidden %1");
	expect_scene("Restored item", "Hidden item");
	expect_audio(third, restored);
	obs_source_release(third);
	obs_source_release(second);
	obs_source_release(source);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(audio_copies_and_replaces, setup, teardown),
		cmocka_unit_test_setup_teardown(scene_copies_replaces_and_formats, setup, teardown),
		cmocka_unit_test_setup_teardown(null_empty_and_group_isolation, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
