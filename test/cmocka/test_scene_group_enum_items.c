#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "obs-internal.h"
#include "obs-scene.h"

/* Call the exported implementation in the selected libobs, without replacing
 * either enumeration function. Keep one owning reference to each stack item;
 * enumeration must release only the extra references it takes. */
struct enum_fixture {
	struct obs_scene scene;
	struct obs_source source;
	struct obs_scene_item group;
	struct obs_scene_item items[3];
};

struct enum_walk {
	struct enum_fixture *fixture;
	size_t visits;
	size_t stop_after;
};

static void init_fixture(struct enum_fixture *f, size_t count)
{
	assert_int_equal(pthread_mutex_init(&f->scene.video_mutex, NULL), 0);
	assert_int_equal(pthread_mutex_init(&f->scene.audio_mutex, NULL), 0);
	f->source.context.data = &f->scene;
	f->group.is_group = true;
	f->group.source = &f->source;
	f->scene.first_item = count ? &f->items[0] : NULL;
	for (size_t i = 0; i < count; i++) {
		f->items[i].ref = 1;
		f->items[i].parent = &f->scene;
		f->items[i].prev = i ? &f->items[i - 1] : NULL;
		f->items[i].next = i + 1 < count ? &f->items[i + 1] : NULL;
	}
}

static void destroy_fixture(struct enum_fixture *f, size_t count)
{
	for (size_t i = 0; i < count; i++)
		assert_int_equal(f->items[i].ref, 1);
	assert_int_equal(pthread_mutex_destroy(&f->scene.audio_mutex), 0);
	assert_int_equal(pthread_mutex_destroy(&f->scene.video_mutex), 0);
}

static bool record_item(obs_scene_t *scene, obs_sceneitem_t *item, void *param)
{
	struct enum_walk *walk = param;
	assert_non_null(walk);
	assert_ptr_equal(scene, &walk->fixture->scene);
	assert_true(walk->visits < 3);
	assert_ptr_equal(item, &walk->fixture->items[walk->visits]);
	walk->visits++;
	return walk->visits < walk->stop_after;
}

static void null_and_non_group_are_noops(void **state)
{
	(void)state;
	struct enum_fixture f = {0};
	init_fixture(&f, 3);
	struct enum_walk walk = {&f, 0, 3};
	obs_sceneitem_group_enum_items(NULL, record_item, &walk);
	obs_sceneitem_group_enum_items(NULL, NULL, NULL);
	f.group.is_group = false;
	obs_sceneitem_group_enum_items(&f.group, record_item, &walk);
	/* A non-group must not even dereference a NULL source. */
	f.group.source = NULL;
	obs_sceneitem_group_enum_items(&f.group, record_item, &walk);
	obs_sceneitem_group_enum_items(&f.group, NULL, NULL);
	assert_int_equal(walk.visits, 0);
	destroy_fixture(&f, 3);
}

static void missing_scene_and_empty_group_are_noops(void **state)
{
	(void)state;
	struct enum_fixture f = {0};
	init_fixture(&f, 0);
	struct enum_walk walk = {&f, 0, 3};
	obs_sceneitem_group_enum_items(&f.group, record_item, &walk);
	f.source.context.data = NULL;
	obs_sceneitem_group_enum_items(&f.group, record_item, &walk);
	obs_sceneitem_group_enum_items(&f.group, NULL, NULL);
	assert_int_equal(walk.visits, 0);
	destroy_fixture(&f, 0);
}

static void item_order_scene_and_param_are_forwarded(void **state)
{
	(void)state;
	struct enum_fixture f = {0};
	init_fixture(&f, 3);
	struct enum_walk walk = {&f, 0, 4};
	/* The delegated scene comes from source context, not the group's parent. */
	struct obs_scene parent = {0};
	f.group.parent = &parent;
	obs_sceneitem_group_enum_items(&f.group, record_item, &walk);
	assert_int_equal(walk.visits, 3);
	destroy_fixture(&f, 3);
}

static void false_callback_stops_at_each_position(void **state)
{
	(void)state;
	struct enum_fixture f = {0};
	init_fixture(&f, 3);
	for (size_t stop = 1; stop <= 3; stop++) {
		struct enum_walk walk = {&f, 0, stop};
		obs_sceneitem_group_enum_items(&f.group, record_item, &walk);
		assert_int_equal(walk.visits, stop);
		for (size_t i = 0; i < 3; i++)
			assert_int_equal(f.items[i].ref, 1);
	}
	destroy_fixture(&f, 3);
}

static bool null_param_callback(obs_scene_t *scene, obs_sceneitem_t *item, void *param)
{
	assert_null(param);
	assert_ptr_equal(scene, item->parent);
	item->id++;
	return true;
}

static void null_callback_and_param(void **state)
{
	(void)state;
	struct enum_fixture f = {0};
	init_fixture(&f, 3);
	obs_sceneitem_group_enum_items(&f.group, NULL, &f);
	obs_sceneitem_group_enum_items(&f.group, NULL, NULL);
	for (size_t i = 0; i < 3; i++)
		assert_int_equal(f.items[i].ref, 1);
	obs_sceneitem_group_enum_items(&f.group, null_param_callback, NULL);
	for (size_t i = 0; i < 3; i++)
		assert_int_equal(f.items[i].id, 1);
	destroy_fixture(&f, 3);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_and_non_group_are_noops),
		cmocka_unit_test(missing_scene_and_empty_group_are_noops),
		cmocka_unit_test(item_order_scene_and_param_are_forwarded),
		cmocka_unit_test(false_callback_stops_at_each_position),
		cmocka_unit_test(null_callback_and_param),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
