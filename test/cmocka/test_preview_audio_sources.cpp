/* Studio Mode: which audio sources the Audio Mixer marks as Preview for the
 * scene in Preview (frontend/utility/preview-audio-sources.cpp, called by
 * AudioMixer::updatePreviewSources). Sources inside a group or a nested
 * scene play when the scene goes to Program, so they belong to the preview
 * too. Real libobs scenes, groups and a dummy audio input; no video and no
 * Qt GUI. obs_startup() needs a display on Linux (run under xvfb-run); the
 * cases skip when it cannot start. */

#include <obs.hpp>

#include "preview-audio-sources.hpp"

#include <unordered_set>

/* cmocka.h last: its fail() and inline macros break C++ standard headers. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
extern "C" {
#include <cmocka.h>
}

static bool obs_up;

static const char *dummy_name(void *)
{
	return "dummy audio";
}

static void *dummy_create(obs_data_t *, obs_source_t *source)
{
	return source;
}

static void dummy_destroy(void *) {}

static void register_dummy_audio(void)
{
	struct obs_source_info info = {};
	info.id = "dummy_audio";
	info.type = OBS_SOURCE_TYPE_INPUT;
	info.output_flags = OBS_SOURCE_AUDIO;
	info.get_name = dummy_name;
	info.create = dummy_create;
	info.destroy = dummy_destroy;
	obs_register_source(&info);
}

struct fixture {
	obs_source_t *direct = nullptr;
	obs_source_t *grouped = nullptr;
	obs_source_t *hidden_in_group = nullptr;
	obs_scene_t *music = nullptr;  /* direct */
	obs_scene_t *group = nullptr;  /* a group holding grouped + hidden_in_group */
	obs_scene_t *nested = nullptr; /* the music scene as a source */
	obs_scene_t *both = nullptr;   /* the group scene nested in another scene */
	obs_sceneitem_t *group_item = nullptr;
};

static fixture f;

static std::unordered_set<QString> collect(obs_scene_t *scene)
{
	std::unordered_set<QString> uuids;
	CollectPreviewAudioSources(scene, uuids);
	return uuids;
}

static bool has(const std::unordered_set<QString> &uuids, obs_source_t *source)
{
	return uuids.count(QString::fromUtf8(obs_source_get_uuid(source))) != 0;
}

static int setup(void **)
{
	if (!obs_up) {
		return 0;
	}

	f.direct = obs_source_create("dummy_audio", "music direct", nullptr, nullptr);
	f.grouped = obs_source_create("dummy_audio", "music grouped", nullptr, nullptr);
	f.hidden_in_group = obs_source_create("dummy_audio", "hidden in group", nullptr, nullptr);

	f.music = obs_scene_create("MUSIC");
	obs_scene_add(f.music, f.direct);

	f.group = obs_scene_create("GROUP");
	obs_sceneitem_t *items[2] = {obs_scene_add(f.group, f.grouped), obs_scene_add(f.group, f.hidden_in_group)};
	obs_sceneitem_set_visible(items[1], false);
	f.group_item = obs_scene_insert_group(f.group, "Group", items, 2);

	f.nested = obs_scene_create("NESTED");
	obs_scene_add(f.nested, obs_scene_get_source(f.music));

	f.both = obs_scene_create("BOTH");
	obs_scene_add(f.both, obs_scene_get_source(f.group));
	return 0;
}

static int teardown(void **)
{
	if (!obs_up) {
		return 0;
	}
	obs_scene_t *scenes[] = {f.both, f.nested, f.group, f.music};
	for (obs_scene_t *scene : scenes) {
		obs_source_remove(obs_scene_get_source(scene));
		obs_scene_release(scene);
	}
	obs_source_t *sources[] = {f.direct, f.grouped, f.hidden_in_group};
	for (obs_source_t *source : sources) {
		obs_source_remove(source);
		obs_source_release(source);
	}
	f = fixture();
	return 0;
}

/* Guard: an audio source directly in the previewed scene. */
static void direct_source_is_previewed(void **)
{
	if (!obs_up) {
		skip();
	}
	auto uuids = collect(f.music);
	assert_int_equal(uuids.size(), 1);
	assert_true(has(uuids, f.direct));
}

/* An audio source inside a group in the previewed scene. */
static void grouped_source_is_previewed(void **)
{
	if (!obs_up) {
		skip();
	}
	auto uuids = collect(f.group);
	assert_true(has(uuids, f.grouped));
	assert_false(has(uuids, f.hidden_in_group));
}

/* An audio source inside a scene nested in the previewed scene. */
static void nested_scene_source_is_previewed(void **)
{
	if (!obs_up) {
		skip();
	}
	auto uuids = collect(f.nested);
	assert_true(has(uuids, f.direct));
}

/* A group inside a nested scene. */
static void group_in_nested_scene_is_previewed(void **)
{
	if (!obs_up) {
		skip();
	}
	auto uuids = collect(f.both);
	assert_true(has(uuids, f.grouped));
	assert_false(has(uuids, f.hidden_in_group));
}

/* Guard: a hidden group hides everything in it. */
static void hidden_group_is_not_previewed(void **)
{
	if (!obs_up) {
		skip();
	}
	obs_sceneitem_set_visible(f.group_item, false);
	auto uuids = collect(f.group);
	assert_int_equal(uuids.size(), 0);
}

int main(void)
{
	obs_up = obs_startup("en-US", nullptr, nullptr);
	if (obs_up) {
		register_dummy_audio();
	} else {
		fprintf(stderr, "obs_startup failed (no display?), skipping\n");
	}

	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(direct_source_is_previewed, setup, teardown),
		cmocka_unit_test_setup_teardown(grouped_source_is_previewed, setup, teardown),
		cmocka_unit_test_setup_teardown(nested_scene_source_is_previewed, setup, teardown),
		cmocka_unit_test_setup_teardown(group_in_nested_scene_is_previewed, setup, teardown),
		cmocka_unit_test_setup_teardown(hidden_group_is_not_previewed, setup, teardown),
	};
	int failed = cmocka_run_group_tests(tests, nullptr, nullptr);

	if (obs_up) {
		obs_shutdown();
	}
	return failed;
}
