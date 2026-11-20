/* Characterize the public C entry point without OBS startup or worker threads.
 * Stack-owned scenes/items use only their real parent list and scene mutexes. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "libobs/build/libobs.pch.h"

/* #2899 intentionally retains the permissive legacy attached-item predicate
 * in libobs-c-reference. NULL/detached calls there are undefined. Probe the
 * exact getter body with the current hybrid predicate contract instead; valid
 * list assertions below also exercise the real public shared-library symbol. */
static bool position_attached(bool has_item, bool has_parent);
static void position_lock(obs_scene_t *scene);
static void position_unlock(obs_scene_t *scene);
#define obs_sceneitem_get_order_position position_body
#define obs_scene_item_order_item_attached position_attached
#define obs_scene_atomic_full_lock position_lock
#define obs_scene_atomic_full_unlock position_unlock
#include "libobs/obs-scene-item-order-position.c.h"
#undef obs_scene_atomic_full_unlock
#undef obs_scene_atomic_full_lock
#undef obs_scene_item_order_item_attached
#undef obs_sceneitem_get_order_position

static unsigned lock_calls, unlock_calls;

static bool position_attached(bool has_item, bool has_parent)
{
	return has_item && has_parent;
}

static void position_lock(obs_scene_t *scene)
{
	lock_calls++;
	assert_int_equal(pthread_mutex_lock(&scene->video_mutex), 0);
	assert_int_equal(pthread_mutex_lock(&scene->audio_mutex), 0);
}

static void position_unlock(obs_scene_t *scene)
{
	unlock_calls++;
	assert_int_equal(pthread_mutex_unlock(&scene->audio_mutex), 0);
	assert_int_equal(pthread_mutex_unlock(&scene->video_mutex), 0);
}

static void assert_position(obs_sceneitem_t *item, int expected)
{
	lock_calls = unlock_calls = 0;
	assert_int_equal(position_body(item), expected);
	assert_int_equal(lock_calls, 1);
	assert_int_equal(unlock_calls, 1);
	assert_int_equal(obs_sceneitem_get_order_position(item), expected);
}

static void init_scene(struct obs_scene *scene)
{
	assert_int_equal(pthread_mutex_init(&scene->video_mutex, NULL), 0);
	assert_int_equal(pthread_mutex_init(&scene->audio_mutex, NULL), 0);
}

static void free_scene(struct obs_scene *scene)
{
	assert_int_equal(pthread_mutex_destroy(&scene->audio_mutex), 0);
	assert_int_equal(pthread_mutex_destroy(&scene->video_mutex), 0);
}

static void link_items(struct obs_scene *scene, struct obs_scene_item *items, size_t count)
{
	scene->first_item = count ? items : NULL;
	for (size_t i = 0; i < count; i++) {
		items[i].parent = scene;
		items[i].prev = i ? &items[i - 1] : NULL;
		items[i].next = i + 1 < count ? &items[i + 1] : NULL;
	}
}

static void null_and_detached_return_zero(void **state)
{
	(void)state;
	struct obs_scene_item detached = {0};
	lock_calls = unlock_calls = 0;
	assert_int_equal(position_body(NULL), 0);
	assert_int_equal(position_body(&detached), 0);
	assert_int_equal(lock_calls, 0);
	assert_int_equal(unlock_calls, 0);
}

static void positions_count_siblings(void **state)
{
	(void)state;
	struct obs_scene scene = {0};
	struct obs_scene_item items[6] = {0};
	init_scene(&scene);
	for (size_t count = 1; count <= 6; count++) {
		link_items(&scene, items, count);
		for (size_t i = 0; i < count; i++) {
			/* IDs and visibility do not affect position. */
			items[i].id = 99 - (int64_t)i;
			items[i].user_visible = i % 2 == 0;
			assert_position(&items[i], (int)i);
		}
	}
	/* Re-link the same handles in a different order. */
	scene.first_item = &items[5];
	for (size_t i = 0; i < 6; i++) {
		items[i].next = i ? &items[i - 1] : NULL;
		items[i].prev = i < 5 ? &items[i + 1] : NULL;
	}
	for (size_t i = 0; i < 6; i++)
		assert_position(&items[5 - i], (int)i);
	free_scene(&scene);
}

static void nested_groups_count_only_the_parent_list(void **state)
{
	(void)state;
	struct obs_scene root = {0}, group = {0}, nested = {0};
	struct obs_scene_item roots[3] = {0}, children[4] = {0}, grandchildren[2] = {0};
	init_scene(&root);
	init_scene(&group);
	init_scene(&nested);
	link_items(&root, roots, 3);
	link_items(&group, children, 4);
	link_items(&nested, grandchildren, 2);
	roots[1].is_group = true;
	children[2].is_group = true;
	group.is_group = nested.is_group = true;
	/* A group occupies one sibling slot; its descendants never inflate it.
	 * Each child's parent is the group's own scene, including nested groups. */
	for (size_t i = 0; i < 3; i++)
		assert_position(&roots[i], (int)i);
	for (size_t i = 0; i < 4; i++)
		assert_position(&children[i], (int)i);
	for (size_t i = 0; i < 2; i++)
		assert_position(&grandchildren[i], (int)i);
	free_scene(&nested);
	free_scene(&group);
	free_scene(&root);
}

static void absent_item_returns_list_length(void **state)
{
	(void)state;
	struct obs_scene scene = {0};
	struct obs_scene_item items[3] = {0}, absent = {0};
	init_scene(&scene);
	absent.parent = &scene;
	/* Inconsistent membership is characterized, not endorsed: count until
	 * NULL when the parent list does not contain the requested item. */
	for (size_t count = 0; count <= 3; count++) {
		link_items(&scene, items, count);
		assert_position(&absent, (int)count);
	}
	free_scene(&scene);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(null_and_detached_return_zero),
					   cmocka_unit_test(positions_count_siblings),
					   cmocka_unit_test(nested_groups_count_only_the_parent_list),
					   cmocka_unit_test(absent_item_returns_list_length)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
