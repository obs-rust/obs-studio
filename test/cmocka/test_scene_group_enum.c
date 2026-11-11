/* Characterize the group enumeration adapter against both real libobs libraries. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "cmocka-compat.h"
#include "obs-internal.h"
#include "obs-scene.h"

struct group_enum_walk {
	obs_scene_t *scene;
	obs_sceneitem_t *items[3];
	void *param;
	size_t count;
	size_t stop_at;
};

static struct group_enum_walk *group_enum_current;

static bool group_enum_record(obs_scene_t *scene, obs_sceneitem_t *item, void *param)
{
	struct group_enum_walk *walk = group_enum_current;
	assert_non_null(walk);
	assert_ptr_equal(param, walk->param);
	assert_ptr_equal(scene, walk->scene);
	assert_true(walk->count < 3);
	assert_ptr_equal(item, walk->items[walk->count]);
	walk->count++;
	return walk->count != walk->stop_at;
}

static void group_enum_drain(void)
{
	while (obs_wait_for_destroy_queue())
		;
}

static obs_sceneitem_t *group_enum_add_scene(obs_scene_t *parent, const char *name)
{
	obs_scene_t *child = obs_scene_create_private(name);
	assert_non_null(child);
	obs_sceneitem_t *item = obs_scene_add(parent, obs_scene_get_source(child));
	assert_non_null(item);
	obs_scene_release(child);
	return item;
}

static void null_and_non_group(void **state)
{
	(void)state;
	struct group_enum_walk walk = {0};
	group_enum_current = &walk;
	obs_sceneitem_group_enum_items(NULL, group_enum_record, &walk);
	obs_sceneitem_group_enum_items(NULL, NULL, NULL);
	/* The flag must be checked before touching the NULL source. Read-only
	 * synthetic records exercise only the fields the adapter actually reads. */
	struct obs_scene_item non_group = {0};
	obs_sceneitem_group_enum_items(&non_group, group_enum_record, &walk);
	obs_sceneitem_group_enum_items(&non_group, NULL, NULL);
	assert_int_equal(walk.count, 0);
	group_enum_current = NULL;
}

static void absent_scene(void **state)
{
	(void)state;
	struct obs_source source = {0};
	struct obs_scene_item group = {.is_group = true, .source = &source};
	struct group_enum_walk walk = {0};
	group_enum_current = &walk;
	obs_sceneitem_group_enum_items(&group, group_enum_record, &walk);
	obs_sceneitem_group_enum_items(&group, NULL, NULL);
	assert_int_equal(walk.count, 0);
	group_enum_current = NULL;
}

static void empty_group_and_real_non_group(void **state)
{
	(void)state;
	obs_scene_t *parent = obs_scene_create_private("empty enumeration parent");
	assert_non_null(parent);
	obs_sceneitem_t *plain = group_enum_add_scene(parent, "plain scene item");
	obs_sceneitem_t *group = obs_scene_add_group(parent, "empty group");
	assert_non_null(group);
	struct group_enum_walk walk = {.scene = obs_sceneitem_group_get_scene(group)};
	assert_non_null(walk.scene);
	group_enum_current = &walk;
	obs_sceneitem_group_enum_items(plain, group_enum_record, &walk);
	obs_sceneitem_group_enum_items(group, group_enum_record, &walk);
	assert_int_equal(walk.count, 0);
	group_enum_current = NULL;
	obs_scene_release(parent);
	group_enum_drain();
}

static void group_enum_check(size_t stop_at, bool null_param)
{
	obs_scene_t *parent = obs_scene_create_private("enumeration parent");
	assert_non_null(parent);
	group_enum_add_scene(parent, "outside before");
	obs_sceneitem_t *group = obs_scene_add_group(parent, "enumerated group");
	assert_non_null(group);
	group_enum_add_scene(parent, "outside after");
	int token = 42;
	struct group_enum_walk walk = {
		.scene = obs_sceneitem_group_get_scene(group),
		.param = null_param ? NULL : &token,
		.stop_at = stop_at,
	};
	assert_non_null(walk.scene);
	walk.items[0] = group_enum_add_scene(walk.scene, "first");
	walk.items[1] = group_enum_add_scene(walk.scene, "second");
	walk.items[2] = group_enum_add_scene(walk.scene, "third");
	group_enum_current = &walk;
	/* NULL callback is forwarded to the scene enumerator, which ignores it. */
	obs_sceneitem_group_enum_items(group, NULL, walk.param);
	assert_int_equal(walk.count, 0);
	obs_sceneitem_group_enum_items(group, group_enum_record, walk.param);
	assert_int_equal(walk.count, stop_at ? stop_at : 3);
	assert_int_equal(token, 42);
	/* A later complete walk proves early stop did not alter group membership. */
	walk.count = 0;
	walk.stop_at = 0;
	obs_sceneitem_group_enum_items(group, group_enum_record, walk.param);
	assert_int_equal(walk.count, 3);
	group_enum_current = NULL;
	obs_scene_release(parent);
	group_enum_drain();
}

static void order_and_param(void **state)
{
	(void)state;
	group_enum_check(0, false);
	group_enum_check(0, true);
}

static void early_stop(void **state)
{
	(void)state;
	group_enum_check(1, false);
	group_enum_check(2, false);
}

int main(void)
{
	if (!obs_startup("en-US", NULL, NULL))
		return 77;
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_and_non_group),
		cmocka_unit_test(absent_scene),
		cmocka_unit_test(empty_group_and_real_non_group),
		cmocka_unit_test(order_and_param),
		cmocka_unit_test(early_stop),
	};
	int result = cmocka_run_group_tests(tests, NULL, NULL);
	group_enum_drain();
	obs_shutdown();
	return result;
}
