/* Direct characterization of the real public obs_scene_prune_sources API
 * before its extraction (Refs #5731). Items whose sources were removed are
 * dropped (item_remove signalled, scene reference released), live items keep
 * their order, and the walk descends into groups. No graphics pipeline is
 * required. */
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <cmocka.h>

#include <obs.h>
#include <util/base.h>

#define MAX_SEEN 8

static unsigned removes;
static obs_sceneitem_t *removed[MAX_SEEN];

static void quiet_log(int level, const char *format, va_list args, void *param)
{
	(void)level;
	(void)format;
	(void)args;
	(void)param;
}

static const char *source_name(void *data)
{
	(void)data;
	return "prune source";
}

static void *source_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void source_destroy(void *data)
{
	(void)data;
}

static void on_item_remove(void *param, calldata_t *cd)
{
	(void)param;
	assert_true(removes < MAX_SEEN);
	removed[removes++] = calldata_ptr(cd, "item");
}

struct collect {
	size_t num;
	obs_sceneitem_t *items[MAX_SEEN];
};

static bool collect_item(obs_scene_t *scene, obs_sceneitem_t *item, void *param)
{
	(void)scene;
	struct collect *c = param;
	assert_true(c->num < MAX_SEEN);
	c->items[c->num++] = item;
	return true;
}

static struct collect items_of(obs_scene_t *scene)
{
	struct collect c = {0};
	obs_scene_enum_items(scene, collect_item, &c);
	return c;
}

static obs_source_t *make_source(const char *name)
{
	obs_source_t *source = obs_source_create_private("prune_test_source", name, NULL);
	assert_non_null(source);
	return source;
}

static obs_scene_t *make_scene(const char *name)
{
	obs_scene_t *scene = obs_scene_create_private(name);
	assert_non_null(scene);
	signal_handler_connect(obs_source_get_signal_handler(obs_scene_get_source(scene)), "item_remove",
			       on_item_remove, NULL);
	removes = 0;
	return scene;
}

static void free_scene(obs_scene_t *scene)
{
	signal_handler_disconnect(obs_source_get_signal_handler(obs_scene_get_source(scene)), "item_remove",
				  on_item_remove, NULL);
	obs_scene_release(scene);
	while (obs_wait_for_destroy_queue())
		;
}

static void empty_scene_is_noop(void **state)
{
	(void)state;
	obs_scene_t *scene = make_scene("empty");
	obs_scene_prune_sources(scene);
	assert_int_equal(removes, 0);
	assert_int_equal(items_of(scene).num, 0);
	free_scene(scene);
}

static void live_items_are_kept(void **state)
{
	(void)state;
	obs_scene_t *scene = make_scene("live");
	obs_source_t *a = make_source("a");
	obs_source_t *b = make_source("b");
	obs_sceneitem_t *ia = obs_scene_add(scene, a);
	obs_sceneitem_t *ib = obs_scene_add(scene, b);

	obs_scene_prune_sources(scene);
	obs_scene_prune_sources(scene);

	struct collect c = items_of(scene);
	assert_int_equal(removes, 0);
	assert_int_equal(c.num, 2);
	assert_ptr_equal(c.items[0], ia);
	assert_ptr_equal(c.items[1], ib);
	assert_false(obs_source_removed(a));

	obs_source_release(a);
	obs_source_release(b);
	free_scene(scene);
}

static void removed_sources_are_pruned_in_order(void **state)
{
	(void)state;
	obs_scene_t *scene = make_scene("prune");
	obs_source_t *a = make_source("a");
	obs_source_t *b = make_source("b");
	obs_source_t *c = make_source("c");
	obs_sceneitem_t *ia = obs_scene_add(scene, a);
	obs_sceneitem_t *ib = obs_scene_add(scene, b);
	obs_sceneitem_t *ic = obs_scene_add(scene, c);

	/* Removing the source alone leaves its item in place until pruned. */
	obs_source_remove(b);
	assert_true(obs_source_removed(b));
	assert_int_equal(items_of(scene).num, 3);
	assert_int_equal(removes, 0);

	/* Keep the pruned item alive to observe its detached state. */
	obs_sceneitem_addref(ib);
	obs_scene_prune_sources(scene);

	struct collect after = items_of(scene);
	assert_int_equal(removes, 1);
	assert_ptr_equal(removed[0], ib);
	assert_int_equal(after.num, 2);
	assert_ptr_equal(after.items[0], ia);
	assert_ptr_equal(after.items[1], ic);
	assert_null(obs_sceneitem_get_scene(ib));
	obs_sceneitem_release(ib);

	/* A second prune finds nothing more to remove. */
	obs_scene_prune_sources(scene);
	assert_int_equal(removes, 1);

	/* Every removed source is pruned in one pass. */
	obs_source_remove(a);
	obs_source_remove(c);
	obs_scene_prune_sources(scene);
	assert_int_equal(removes, 3);
	assert_ptr_equal(removed[1], ia);
	assert_ptr_equal(removed[2], ic);
	assert_int_equal(items_of(scene).num, 0);

	obs_source_release(a);
	obs_source_release(b);
	obs_source_release(c);
	free_scene(scene);
}

static void group_children_are_pruned(void **state)
{
	(void)state;
	obs_scene_t *scene = make_scene("outer");
	obs_sceneitem_t *group_item = obs_scene_add_group(scene, "group");
	assert_non_null(group_item);
	obs_scene_t *group = obs_sceneitem_group_get_scene(group_item);
	obs_source_t *a = make_source("a");
	obs_source_t *b = make_source("b");
	obs_sceneitem_t *ia = obs_scene_add(group, a);
	obs_sceneitem_t *ib = obs_scene_add(group, b);
	(void)ib;

	obs_source_remove(b);
	obs_scene_prune_sources(scene);

	struct collect outer = items_of(scene);
	struct collect inner = items_of(group);
	assert_int_equal(outer.num, 1);
	assert_ptr_equal(outer.items[0], group_item);
	assert_int_equal(inner.num, 1);
	assert_ptr_equal(inner.items[0], ia);

	obs_source_release(a);
	obs_source_release(b);
	free_scene(scene);
}

static int setup(void **state)
{
	(void)state;
	base_set_log_handler(quiet_log, NULL);
	if (!obs_startup("en-US", NULL, NULL))
		return -1;
	struct obs_source_info info = {
		.id = "prune_test_source",
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = source_name,
		.create = source_create,
		.destroy = source_destroy,
	};
	obs_register_source(&info);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(empty_scene_is_noop),
		cmocka_unit_test(live_items_are_kept),
		cmocka_unit_test(removed_sources_are_pruned_in_order),
		cmocka_unit_test(group_children_are_pruned),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
