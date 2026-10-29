/* Characterization of the transform update walk of
 * update_transforms_and_prune_sources() in libobs/obs-scene.c, through the
 * real libobs, before and after its decisions move to Rust (#2874).
 *
 * obs_scene_prune_sources() runs the walk that scene_video_render() runs on
 * each frame, without a render and with no scene size change. On each item
 * the walk:
 * - removes an item whose source was removed, and turns the group rebuild on;
 * - walks a group's own scene first (under its video lock), then
 * - computes the item's transform again when its update_transform flag is
 *   set, its source has a new size, or the scene has a new size, and turns
 *   the group rebuild on;
 * and resizes the group when the rebuild is on.
 *
 * Each case records the item_transform signals of the scene and of the
 * group's scene, in order, and counts the get_width and get_height calls of
 * its sources: the walk reads the source size only when the item's flag is
 * clear (the || of the original stopped at the flag), and
 * update_item_transform() reads it once more when it computes the transform.
 *
 * test_scene_transform_update_steps covers the decisions on their own; this
 * test runs on libobs and libobs-c-reference alike. Core video is not needed.
 * obs_startup() needs a display on Linux; without one the cases skip. */

#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <obs.h>
#include <util/base.h>

#define SIZE_SOURCE_ID "scene_transform_update_source"
#define LOG_MAX 32

static void quiet_log(int lvl, const char *msg, va_list args, void *p)
{
	(void)lvl;
	(void)msg;
	(void)args;
	(void)p;
}

/* A source with a size the case sets, which counts its size reads. */
struct size_source {
	uint32_t width;
	uint32_t height;
	long width_calls;
	long height_calls;
};

static const char *size_source_name(void *type_data)
{
	(void)type_data;
	return "size source";
}

static void *size_source_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	(void)source;
	struct size_source *s = bzalloc(sizeof(*s));
	s->width = 100;
	s->height = 50;
	return s;
}

static void size_source_destroy(void *data)
{
	bfree(data);
}

static uint32_t size_source_width(void *data)
{
	struct size_source *s = data;
	s->width_calls++;
	return s->width;
}

static uint32_t size_source_height(void *data)
{
	struct size_source *s = data;
	s->height_calls++;
	return s->height;
}

/* The item_transform signals of one walk, in order. */
struct signal_log {
	obs_sceneitem_t *items[LOG_MAX];
	size_t count;
};

static void on_item_transform(void *param, calldata_t *cd)
{
	struct signal_log *log = param;
	obs_sceneitem_t *item = calldata_ptr(cd, "item");

	if (log->count < LOG_MAX)
		log->items[log->count] = item;
	log->count++;
}

static void watch_transforms(obs_scene_t *scene, struct signal_log *log)
{
	signal_handler_t *sh = obs_source_get_signal_handler(obs_scene_get_source(scene));
	signal_handler_connect(sh, "item_transform", on_item_transform, log);
}

static void unwatch_transforms(obs_scene_t *scene, struct signal_log *log)
{
	signal_handler_t *sh = obs_source_get_signal_handler(obs_scene_get_source(scene));
	signal_handler_disconnect(sh, "item_transform", on_item_transform, log);
}

static int setup(void **state)
{
	(void)state;
	base_set_log_handler(quiet_log, NULL);
	if (!obs_startup("en-US", NULL, NULL)) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	struct obs_source_info info = {
		.id = SIZE_SOURCE_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = size_source_name,
		.create = size_source_create,
		.destroy = size_source_destroy,
		.get_width = size_source_width,
		.get_height = size_source_height,
	};
	obs_register_source(&info);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (obs_initialized())
		obs_shutdown();
	return 0;
}

static void require_obs(void)
{
	if (!obs_initialized())
		skip();
}

/* An item of a new size source in scene. The scene holds the only
 * reference to the source. */
static obs_sceneitem_t *add_item(obs_scene_t *scene, struct size_source **data)
{
	obs_source_t *source = obs_source_create_private(SIZE_SOURCE_ID, "item", NULL);
	obs_sceneitem_t *item = obs_scene_add(scene, source);

	*data = obs_obj_get_data(source);
	obs_source_release(source);
	return item;
}

static void reset_counts(struct size_source *data)
{
	data->width_calls = 0;
	data->height_calls = 0;
}

static void reset_log(struct signal_log *log)
{
	memset(log, 0, sizeof(*log));
}

static void prune(obs_scene_t *scene, struct signal_log *log, struct size_source *data)
{
	reset_log(log);
	if (data)
		reset_counts(data);
	obs_scene_prune_sources(scene);
}

static bool count_item(obs_scene_t *scene, obs_sceneitem_t *item, void *param)
{
	(void)scene;
	(void)item;
	(*(size_t *)param)++;
	return true;
}

static size_t item_count(obs_scene_t *scene)
{
	size_t count = 0;
	obs_scene_enum_items(scene, count_item, &count);
	return count;
}

static void crop_by(obs_sceneitem_t *item, int left)
{
	struct obs_sceneitem_crop crop = {left, 0, 0, 0};
	obs_sceneitem_set_crop(item, &crop);
}

/* A new item: obs_scene_add() does not compute its transform, so the item's
 * last size is 0 x 0 and its first walk computes it (the size read twice).
 * After that, with nothing new: no transform, and the walk reads the source
 * size once. */
static void idle_item(void **state)
{
	(void)state;
	require_obs();
	struct signal_log log;
	struct size_source *data;
	obs_scene_t *scene = obs_scene_create_private("idle");
	obs_sceneitem_t *item = add_item(scene, &data);
	watch_transforms(scene, &log);

	prune(scene, &log, data);
	assert_int_equal(log.count, 1);
	assert_ptr_equal(log.items[0], item);
	assert_int_equal(data->width_calls, 2);
	assert_int_equal(data->height_calls, 2);

	for (int i = 0; i < 3; i++) {
		prune(scene, &log, data);
		assert_int_equal(log.count, 0);
		assert_int_equal(data->width_calls, 1);
		assert_int_equal(data->height_calls, 1);
	}

	unwatch_transforms(scene, &log);
	obs_scene_release(scene);
}

/* set_crop() sets the update_transform flag without a transform. The walk
 * does not read the size for its decision, only update_item_transform()
 * reads it, and the flag is clear afterwards. */
static void flag_skips_size_read(void **state)
{
	(void)state;
	require_obs();
	struct signal_log log;
	struct size_source *data;
	obs_scene_t *scene = obs_scene_create_private("flag");
	obs_sceneitem_t *item = add_item(scene, &data);
	watch_transforms(scene, &log);

	reset_log(&log);
	crop_by(item, 2);
	assert_int_equal(log.count, 0);

	prune(scene, &log, data);
	assert_int_equal(log.count, 1);
	assert_ptr_equal(log.items[0], item);
	assert_int_equal(data->width_calls, 1);
	assert_int_equal(data->height_calls, 1);

	prune(scene, &log, data);
	assert_int_equal(log.count, 0);
	assert_int_equal(data->width_calls, 1);

	unwatch_transforms(scene, &log);
	obs_scene_release(scene);
}

/* A new source size: one transform, the size read twice (the decision and
 * update_item_transform()); the next walk finds the stored size. */
static void new_source_size(void **state)
{
	(void)state;
	require_obs();
	struct signal_log log;
	struct size_source *data;
	obs_scene_t *scene = obs_scene_create_private("size");
	obs_sceneitem_t *item = add_item(scene, &data);
	watch_transforms(scene, &log);

	static const struct {
		uint32_t width;
		uint32_t height;
	} sizes[] = {{120, 50}, {120, 60}, {60, 120}, {0, 0}, {100, 50}};

	for (size_t i = 0; i < sizeof(sizes) / sizeof(sizes[0]); i++) {
		data->width = sizes[i].width;
		data->height = sizes[i].height;

		prune(scene, &log, data);
		assert_int_equal(log.count, 1);
		assert_ptr_equal(log.items[0], item);
		assert_int_equal(data->width_calls, 2);
		assert_int_equal(data->height_calls, 2);

		prune(scene, &log, data);
		assert_int_equal(log.count, 0);
		assert_int_equal(data->width_calls, 1);
	}

	unwatch_transforms(scene, &log);
	obs_scene_release(scene);
}

/* A deferred item with a new size: update_item_transform() returns before it
 * reads the size or stores it, so every walk asks again and reads the size
 * once. defer_update_end() on an item of a scene computes the transform at
 * once. */
static void deferred_item_asks_every_walk(void **state)
{
	(void)state;
	require_obs();
	struct signal_log log;
	struct size_source *data;
	obs_scene_t *scene = obs_scene_create_private("deferred");
	obs_sceneitem_t *item = add_item(scene, &data);
	watch_transforms(scene, &log);

	obs_sceneitem_defer_update_begin(item);
	data->width = 300;
	for (int i = 0; i < 3; i++) {
		prune(scene, &log, data);
		assert_int_equal(log.count, 0);
		assert_int_equal(data->width_calls, 1);
		assert_int_equal(data->height_calls, 1);
	}

	reset_log(&log);
	obs_sceneitem_defer_update_end(item);
	assert_int_equal(log.count, 1);
	assert_ptr_equal(log.items[0], item);

	prune(scene, &log, data);
	assert_int_equal(log.count, 0);
	assert_int_equal(data->width_calls, 1);

	unwatch_transforms(scene, &log);
	obs_scene_release(scene);
}

/* The flag set while the item is deferred stays set: defer_update_end()
 * computes the transform without clearing it (update_tex false), so the next
 * walk computes it once more. Characterized, not endorsed. */
static void deferred_flag_survives_defer_end(void **state)
{
	(void)state;
	require_obs();
	struct signal_log log;
	struct size_source *data;
	obs_scene_t *scene = obs_scene_create_private("deferred flag");
	obs_sceneitem_t *item = add_item(scene, &data);
	watch_transforms(scene, &log);

	obs_sceneitem_defer_update_begin(item);
	crop_by(item, 4);
	prune(scene, &log, data);
	assert_int_equal(log.count, 0);
	assert_int_equal(data->width_calls, 0);

	reset_log(&log);
	obs_sceneitem_defer_update_end(item);
	assert_int_equal(log.count, 1);

	prune(scene, &log, data);
	assert_int_equal(log.count, 1);
	assert_ptr_equal(log.items[0], item);
	assert_int_equal(data->width_calls, 1);

	prune(scene, &log, data);
	assert_int_equal(log.count, 0);

	unwatch_transforms(scene, &log);
	obs_scene_release(scene);
}

/* A removed source: the walk takes its item out; the other item is idle. */
static void removed_item_is_pruned(void **state)
{
	(void)state;
	require_obs();
	struct signal_log log;
	struct size_source *data_a;
	struct size_source *data_b;
	obs_scene_t *scene = obs_scene_create_private("removed");
	obs_sceneitem_t *a = add_item(scene, &data_a);
	obs_sceneitem_t *b = add_item(scene, &data_b);
	obs_scene_prune_sources(scene); /* the first walk computes both */
	watch_transforms(scene, &log);

	obs_source_remove(obs_sceneitem_get_source(b));
	assert_int_equal(item_count(scene), 2);

	prune(scene, &log, data_a);
	assert_int_equal(log.count, 0);
	assert_int_equal(item_count(scene), 1);
	assert_int_equal(data_a->width_calls, 1);

	(void)a;
	unwatch_transforms(scene, &log);
	obs_scene_release(scene);
}

/* A scene with a group of one or two items. */
struct grouped {
	obs_scene_t *scene;
	obs_sceneitem_t *group;
	obs_scene_t *group_scene;
	obs_sceneitem_t *child[2];
	struct size_source *data[2];
	struct signal_log log;
};

static void build_group(struct grouped *g, size_t children)
{
	memset(g, 0, sizeof(*g));
	g->scene = obs_scene_create_private("grouped");
	g->group = obs_scene_add_group(g->scene, "group");
	g->group_scene = obs_sceneitem_group_get_scene(g->group);
	for (size_t i = 0; i < children; i++)
		g->child[i] = add_item(g->group_scene, &g->data[i]);

	/* the first walk computes the new children's transforms and so resizes
	 * the group */
	obs_scene_prune_sources(g->scene);

	watch_transforms(g->scene, &g->log);
	watch_transforms(g->group_scene, &g->log);
}

static void free_group(struct grouped *g)
{
	unwatch_transforms(g->group_scene, &g->log);
	unwatch_transforms(g->scene, &g->log);
	obs_scene_release(g->scene);
}

static void assert_log(const struct signal_log *log, obs_sceneitem_t *const *expected, size_t count)
{
	assert_int_equal(log->count, count);
	for (size_t i = 0; i < count; i++)
		assert_ptr_equal(log->items[i], expected[i]);
}

/* A settled group with nothing new: no transform, no resize. */
static void group_idle(void **state)
{
	(void)state;
	require_obs();
	struct grouped g;
	build_group(&g, 1);

	prune(g.scene, &g.log, g.data[0]);
	assert_int_equal(g.log.count, 0);
	assert_int_equal(g.data[0]->width_calls, 1);

	free_group(&g);
}

/* A child with the flag set (an item of a group only sets the flag): the
 * group's walk runs first and computes the child's transform, then the group
 * resizes, which computes each child's transform again and the group's twice
 * (resize_group() for a group without bounds). The group item itself then
 * has nothing new. */
static void group_child_update_resizes_group(void **state)
{
	(void)state;
	require_obs();
	struct grouped g;
	build_group(&g, 1);

	struct vec2 pos = {10.0f, 20.0f};
	obs_sceneitem_set_pos(g.child[0], &pos);
	assert_int_equal(g.log.count, 0);

	prune(g.scene, &g.log, NULL);
	obs_sceneitem_t *expected[] = {g.child[0], g.child[0], g.group, g.group};
	assert_log(&g.log, expected, 4);

	prune(g.scene, &g.log, NULL);
	assert_int_equal(g.log.count, 0);

	free_group(&g);
}

/* The group's update_group_resize flag alone (defer_group_resize_end())
 * resizes it on the next walk. */
static void group_resize_flag_alone(void **state)
{
	(void)state;
	require_obs();
	struct grouped g;
	build_group(&g, 1);

	obs_sceneitem_defer_group_resize_begin(g.group);
	obs_sceneitem_defer_group_resize_end(g.group);
	assert_int_equal(g.log.count, 0);

	prune(g.scene, &g.log, NULL);
	obs_sceneitem_t *expected[] = {g.child[0], g.group, g.group};
	assert_log(&g.log, expected, 3);

	prune(g.scene, &g.log, NULL);
	assert_int_equal(g.log.count, 0);

	free_group(&g);
}

/* A removed child resizes the group, with no transform of its own. */
static void group_removed_child_resizes_group(void **state)
{
	(void)state;
	require_obs();
	struct grouped g;
	build_group(&g, 2);

	obs_source_remove(obs_sceneitem_get_source(g.child[1]));
	prune(g.scene, &g.log, NULL);
	obs_sceneitem_t *expected[] = {g.child[0], g.group, g.group};
	assert_log(&g.log, expected, 3);
	assert_int_equal(item_count(g.group_scene), 1);

	prune(g.scene, &g.log, NULL);
	assert_int_equal(g.log.count, 0);

	free_group(&g);
}

/* A deferred child with a new size asks on every walk and keeps the group
 * resizing; its own transform waits (resize_group() asks for it as well), so
 * only the group's transform is computed. */
static void group_deferred_child_keeps_resizing(void **state)
{
	(void)state;
	require_obs();
	struct grouped g;
	build_group(&g, 1);

	obs_sceneitem_defer_update_begin(g.child[0]);
	g.data[0]->width = 300;
	for (int i = 0; i < 3; i++) {
		prune(g.scene, &g.log, NULL);
		obs_sceneitem_t *expected[] = {g.group, g.group};
		assert_log(&g.log, expected, 2);
	}

	obs_sceneitem_defer_update_end(g.child[0]);
	prune(g.scene, &g.log, NULL);
	obs_sceneitem_t *expected[] = {g.child[0], g.child[0], g.group, g.group};
	assert_log(&g.log, expected, 4);

	prune(g.scene, &g.log, NULL);
	assert_int_equal(g.log.count, 0);

	free_group(&g);
}

/* A removed group: the walk takes the group item out before it would walk
 * the group's scene, so the child's flag is not acted on. */
static void removed_group_skips_its_walk(void **state)
{
	(void)state;
	require_obs();
	struct grouped g;
	build_group(&g, 1);

	/* keep the group's scene alive for the listener after the removal */
	obs_source_t *group_source = obs_source_get_ref(obs_scene_get_source(g.group_scene));

	crop_by(g.child[0], 3);
	obs_source_remove(group_source);
	prune(g.scene, &g.log, NULL);
	assert_int_equal(g.log.count, 0);
	assert_int_equal(item_count(g.scene), 0);

	unwatch_transforms(g.group_scene, &g.log);
	unwatch_transforms(g.scene, &g.log);
	obs_scene_release(g.scene);
	obs_source_release(group_source);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(idle_item),
		cmocka_unit_test(flag_skips_size_read),
		cmocka_unit_test(new_source_size),
		cmocka_unit_test(deferred_item_asks_every_walk),
		cmocka_unit_test(deferred_flag_survives_defer_end),
		cmocka_unit_test(removed_item_is_pruned),
		cmocka_unit_test(group_idle),
		cmocka_unit_test(group_child_update_resizes_group),
		cmocka_unit_test(group_resize_flag_alone),
		cmocka_unit_test(group_removed_child_resizes_group),
		cmocka_unit_test(group_deferred_child_keeps_resizing),
		cmocka_unit_test(removed_group_skips_its_walk),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
