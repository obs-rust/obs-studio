/* Direct characterization of the extracted C lookups, before the Rust swap.
 * Stack fixtures use the real private layouts; no source startup is needed. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "obs-internal.h"
#include "obs-scene.h"

static void scene_lookup(void **state)
{
	(void)state;
	struct obs_scene scenes[3] = {0};
	struct obs_scene_item items[7] = {0};
	struct obs_source sources[7] = {0};
	char *names[] = {"group", "same", "same", "same", "nested", "deep", ""};
	for (size_t i = 0; i < 3; i++) {
		assert_int_equal(pthread_mutex_init(&scenes[i].video_mutex, NULL), 0);
		assert_int_equal(pthread_mutex_init(&scenes[i].audio_mutex, NULL), 0);
	}
	for (size_t i = 0; i < 7; i++) {
		items[i].source = &sources[i];
		sources[i].context.name = names[i];
		items[i].id = (int64_t)i;
	}
	/* Top: group -> same -> same -> empty-name group. Group: same -> nested.
	 * Nested: deep. Recursive means one group-child level, characterized, not endorsed. */
	scenes[0].first_item = &items[0];
	items[0].next = &items[1];
	items[1].next = &items[2];
	items[2].next = &items[6];
	items[0].is_group = items[4].is_group = items[6].is_group = true;
	sources[0].context.data = &scenes[1];
	sources[4].context.data = &scenes[2];
	scenes[1].first_item = &items[3];
	items[3].next = &items[4];
	scenes[2].first_item = &items[5];
	items[2].id = items[1].id;
	assert_null(obs_scene_find_source(NULL, "same"));
	assert_null(obs_scene_find_source_recursive(NULL, "same"));
	assert_null(obs_scene_find_sceneitem_by_id(NULL, 1));
	assert_null(obs_scene_get_group(NULL, "group"));
	assert_null(obs_scene_get_group(&scenes[0], NULL));
	assert_null(obs_scene_get_group(&scenes[0], ""));
	assert_ptr_equal(obs_scene_find_source(&scenes[0], "same"), &items[1]);
	assert_ptr_equal(obs_scene_find_source_recursive(&scenes[0], "same"), &items[3]);
	assert_ptr_equal(obs_scene_find_source_recursive(&scenes[0], "group"), &items[0]);
	assert_null(obs_scene_find_source(&scenes[0], "nested"));
	assert_ptr_equal(obs_scene_find_source_recursive(&scenes[0], "nested"), &items[4]);
	assert_null(obs_scene_find_source_recursive(&scenes[0], "deep"));
	assert_ptr_equal(obs_scene_find_source_recursive(&scenes[1], "deep"), &items[5]);
	assert_ptr_equal(obs_scene_find_sceneitem_by_id(&scenes[0], 1), &items[1]);
	assert_null(obs_scene_find_sceneitem_by_id(&scenes[0], 3));
	assert_null(obs_scene_find_sceneitem_by_id(&scenes[0], -1));
	assert_ptr_equal(obs_scene_get_group(&scenes[0], "group"), &items[0]);
	assert_null(obs_scene_get_group(&scenes[0], "same"));
	assert_null(obs_scene_get_group(&scenes[0], "nested"));
	assert_null(obs_scene_find_source(&scenes[0], "missing"));
	assert_null(obs_scene_find_source_recursive(&scenes[0], "missing"));
	assert_null(obs_scene_get_group(&scenes[0], "missing"));
	/* Group lookup skips null stored names and non-group name matches. */
	sources[0].context.name = NULL;
	assert_null(obs_scene_get_group(&scenes[0], "group"));
	sources[0].context.name = names[0];
	items[0].is_group = false;
	items[2].is_group = true;
	sources[2].context.name = names[0];
	assert_ptr_equal(obs_scene_get_group(&scenes[0], "group"), &items[2]);
	for (size_t i = 0; i < 3; i++) {
		assert_int_equal(pthread_mutex_destroy(&scenes[i].audio_mutex), 0);
		assert_int_equal(pthread_mutex_destroy(&scenes[i].video_mutex), 0);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(scene_lookup)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
