/* Tearing down scenes that nest a group, then removing the sources the group
 * holds, used to free a scene item after its source: obs_sceneitem_destroy()
 * disconnected "rename" from a freed source's signal handler (use after free,
 * a crash on Windows where freed memory is poisoned). Mirrors the fixture of
 * test_preview_audio_sources: MUSIC, GROUP (a "Group" of two sources), NESTED
 * (MUSIC) and BOTH (GROUP), torn down with obs_source_remove + release.
 *
 * No graphics are initialised. obs_startup() may fail without a display on
 * Linux; the case then skips. Run under ASan or valgrind to see the report. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <util/base.h>
#include <util/platform.h>
#include <util/threading.h>

static bool obs_up;
static volatile long destroyed;

static const char *dummy_name(void *type_data)
{
	(void)type_data;
	return "dummy audio";
}

static void *dummy_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void dummy_destroy(void *data)
{
	(void)data;
	os_sleep_ms(100);
	os_atomic_inc_long(&destroyed);
}

static void busy_destroy_thread(void *param)
{
	(void)param;
	os_sleep_ms(300);
}

static void register_dummy_audio(void)
{
	struct obs_source_info info;
	memset(&info, 0, sizeof(info));
	info.id = "group_teardown_dummy_audio";
	info.type = OBS_SOURCE_TYPE_INPUT;
	info.output_flags = OBS_SOURCE_AUDIO;
	info.get_name = dummy_name;
	info.create = dummy_create;
	info.destroy = dummy_destroy;
	obs_register_source(&info);
}

static void teardown_after_group_fixture(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_source_t *direct = obs_source_create("group_teardown_dummy_audio", "music direct", NULL, NULL);
	obs_source_t *grouped = obs_source_create("group_teardown_dummy_audio", "music grouped", NULL, NULL);
	obs_source_t *hidden = obs_source_create("group_teardown_dummy_audio", "hidden in group", NULL, NULL);

	obs_scene_t *music = obs_scene_create("MUSIC");
	obs_scene_add(music, direct);

	obs_scene_t *group = obs_scene_create("GROUP");
	obs_sceneitem_t *items[2] = {obs_scene_add(group, grouped), obs_scene_add(group, hidden)};
	obs_sceneitem_set_visible(items[1], false);
	obs_scene_insert_group(group, "Group", items, 2);

	obs_scene_t *nested = obs_scene_create("NESTED");
	obs_scene_add(nested, obs_scene_get_source(music));

	obs_scene_t *both = obs_scene_create("BOTH");
	obs_scene_add(both, obs_scene_get_source(group));

	/* Keep the destruction thread busy so every scene's destroy task is queued
	 * before obs_shutdown() queues its wait marker; the sources the scenes
	 * release are then destroyed by tasks queued behind that marker. */
	obs_queue_task(OBS_TASK_DESTROY, busy_destroy_thread, NULL, false);

	obs_scene_t *scenes[] = {both, nested, group, music};
	for (size_t i = 0; i < 4; i++) {
		obs_source_remove(obs_scene_get_source(scenes[i]));
		obs_scene_release(scenes[i]);
	}
	obs_source_t *sources[] = {direct, grouped, hidden};
	for (size_t i = 0; i < 3; i++) {
		obs_source_remove(sources[i]);
		obs_source_release(sources[i]);
	}

	obs_shutdown();
	obs_up = false;
	assert_int_equal(os_atomic_load_long(&destroyed), 3);
}

int main(void)
{
	obs_up = obs_startup("en-US", NULL, NULL);
	if (obs_up)
		register_dummy_audio();
	else
		fprintf(stderr, "obs_startup failed, skipping\n");

	const struct CMUnitTest tests[] = {
		cmocka_unit_test(teardown_after_group_fixture),
	};
	int failed = cmocka_run_group_tests(tests, NULL, NULL);
	if (obs_up)
		obs_shutdown();
	return failed;
}
