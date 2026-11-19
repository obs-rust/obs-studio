/* Characterize wrapper locking and the real linked parent-group walk (#5276).
 * Stack-owned native layouts need no OBS startup, sources or worker threads. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <string.h>
#include "libobs/build/libobs.pch.h"

static struct obs_scene scenes[3];
static struct obs_scene_item items[5], child;
static struct obs_source sources[3];
static unsigned locks, lookups, unlocks;
static bool locked;
static obs_sceneitem_t *lookup_result;

void sceneitem_group_record_lock(obs_scene_t *scene)
{
	assert_ptr_equal(scene, &scenes[0]);
	assert_false(locked);
	assert_int_equal(locks, lookups);
	assert_int_equal(locks, unlocks);
	locked = true;
	locks++;
}

obs_sceneitem_t *sceneitem_group_record_lookup(obs_scene_t *scene, obs_sceneitem_t *item)
{
	assert_ptr_equal(scene, &scenes[0]);
	assert_ptr_equal(item, &child);
	assert_true(locked);
	assert_int_equal(locks, lookups + 1);
	lookups++;
	return lookup_result;
}

void sceneitem_group_record_unlock(obs_scene_t *scene)
{
	assert_ptr_equal(scene, &scenes[0]);
	assert_true(locked);
	assert_int_equal(locks, lookups);
	assert_int_equal(locks, unlocks + 1);
	locked = false;
	unlocks++;
}

#ifndef SCENEITEM_GET_GROUP_EXTERNAL
#define obs_sceneitem_get_group sceneitem_group_recorded
#define obs_scene_atomic_full_lock sceneitem_group_record_lock
#define obs_scene_atomic_full_unlock sceneitem_group_record_unlock
#define obs_sceneitem_parent_group_lookup sceneitem_group_record_lookup
#include "libobs/obs-sceneitem-get-group.c.h"
#undef obs_sceneitem_parent_group_lookup
#undef obs_scene_atomic_full_unlock
#undef obs_scene_atomic_full_lock
#undef obs_sceneitem_get_group
#else
obs_sceneitem_t *sceneitem_group_recorded(obs_scene_t *scene, obs_sceneitem_t *item);
#endif

static int setup(void **state)
{
	(void)state;
	memset(scenes, 0, sizeof(scenes));
	memset(items, 0, sizeof(items));
	memset(sources, 0, sizeof(sources));
	memset(&child, 0, sizeof(child));
	locks = lookups = unlocks = 0;
	locked = false;
	lookup_result = NULL;
	assert_int_equal(pthread_mutex_init(&scenes[0].video_mutex, NULL), 0);
	assert_int_equal(pthread_mutex_init(&scenes[0].audio_mutex, NULL), 0);
	child.ref = 11;
	child.parent = &scenes[1];
	for (size_t i = 0; i < 5; i++) {
		items[i].ref = 17 + (long)i;
		items[i].next = i + 1 < 5 ? &items[i + 1] : NULL;
	}
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	assert_false(locked);
	assert_int_equal(locks, lookups);
	assert_int_equal(locks, unlocks);
	assert_int_equal(pthread_mutex_destroy(&scenes[0].audio_mutex), 0);
	assert_int_equal(pthread_mutex_destroy(&scenes[0].video_mutex), 0);
	return 0;
}

static void rejects_before_lock(void **state)
{
	(void)state;
	assert_null(sceneitem_group_recorded(NULL, NULL));
	assert_null(sceneitem_group_recorded(NULL, &child));
	assert_null(sceneitem_group_recorded(&scenes[0], NULL));
	child.is_group = true;
	assert_null(sceneitem_group_recorded(NULL, &child));
	assert_null(sceneitem_group_recorded(&scenes[0], &child));
	assert_int_equal(locks, 0);
	assert_int_equal(lookups, 0);
	assert_int_equal(unlocks, 0);
	/* Real library must also reject before touching uninitialized locks. */
	assert_null(obs_sceneitem_get_group(NULL, NULL));
	assert_null(obs_sceneitem_get_group(NULL, &child));
	assert_null(obs_sceneitem_get_group(&scenes[2], NULL));
	assert_null(obs_sceneitem_get_group(&scenes[2], &child));
}

static void balanced_lookup_returns_borrowed_pointer(void **state)
{
	(void)state;
	for (size_t i = 0; i < 6; i++) {
		lookup_result = i < 5 ? &items[i] : NULL;
		assert_ptr_equal(sceneitem_group_recorded(&scenes[0], &child), lookup_result);
		assert_int_equal(locks, i + 1);
		assert_int_equal(lookups, locks);
		assert_int_equal(unlocks, locks);
		assert_false(locked);
		assert_int_equal(child.ref, 11);
		for (size_t j = 0; j < 5; j++)
			assert_int_equal(items[j].ref, 17 + j);
	}
}

static void linked_query(obs_sceneitem_t *expected)
{
	unsigned char before_items[sizeof(items)], before_sources[sizeof(sources)], before_child[sizeof(child)];
	memcpy(before_items, items, sizeof(items));
	memcpy(before_sources, sources, sizeof(sources));
	memcpy(before_child, &child, sizeof(child));
	obs_sceneitem_t *first = scenes[0].first_item;
	for (unsigned repeat = 0; repeat < 2; repeat++) {
		assert_ptr_equal(obs_sceneitem_get_group(&scenes[0], &child), expected);
		assert_ptr_equal(scenes[0].first_item, first);
		/* Includes every item/source reference count: result is borrowed. */
		assert_memory_equal(items, before_items, sizeof(items));
		assert_memory_equal(sources, before_sources, sizeof(sources));
		assert_memory_equal(&child, before_child, sizeof(child));
	}
}

static void linked_first_matching_parent_group(void **state)
{
	(void)state;
	linked_query(NULL); /* empty scene */
	scenes[0].first_item = &items[0];
	linked_query(NULL); /* non-groups need no source, even with matching parent */
	items[0].parent = child.parent;
	items[1].is_group = items[2].is_group = items[3].is_group = true;
	items[1].source = &sources[0];
	items[2].source = &sources[1];
	items[3].source = &sources[2];
	sources[0].context.data = &scenes[2];
	sources[1].context.data = sources[2].context.data = child.parent;
	linked_query(&items[2]); /* skip ordinary item and foreign group */
	items[2].removed = true;
	linked_query(&items[2]); /* characterized: removed flag is not consulted */
	sources[1].context.data = &scenes[2];
	linked_query(&items[3]);
	child.parent = &scenes[0];
	linked_query(NULL); /* no matching group / foreign child */
	child.parent = NULL;
	linked_query(NULL);
	sources[0].context.data = NULL;
	linked_query(&items[1]); /* characterized: NULL parent matches NULL context */
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(rejects_before_lock, setup, teardown),
		cmocka_unit_test_setup_teardown(balanced_lookup_returns_borrowed_pointer, setup, teardown),
		cmocka_unit_test_setup_teardown(linked_first_matching_parent_group, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
