/* Characterize canvas migration before extracting its C implementation.
 * These cases exercise the real public API against both libobs variants. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <obs.h>

struct migration_event {
	obs_source_t *source;
	obs_canvas_t *canvas;
	bool added;
};

struct migration_log {
	struct migration_event events[16];
	size_t count;
};

struct migration_listener {
	struct migration_log *log;
	obs_canvas_t *canvas;
	bool added;
};

static void migration_signal(void *param, calldata_t *data)
{
	struct migration_listener *listener = param;
	struct migration_log *log = listener->log;
	assert_true(log->count < 16);
	log->events[log->count++] = (struct migration_event){
		.source = calldata_ptr(data, "source"),
		.canvas = listener->canvas,
		.added = listener->added,
	};
}

static void expect_owner(obs_source_t *source, obs_canvas_t *expected)
{
	obs_canvas_t *owner = obs_source_get_canvas(source);
	assert_ptr_equal(owner, expected);
	obs_canvas_release(owner);
}

static bool count_items(obs_scene_t *scene, obs_sceneitem_t *item, void *param)
{
	(void)scene;
	(void)item;
	(*(size_t *)param)++;
	return true;
}

static void expect_items(obs_scene_t *scene, size_t expected)
{
	size_t count = 0;
	obs_scene_enum_items(scene, count_items, &count);
	assert_int_equal(count, expected);
}

static void empty_scene_move(void **state)
{
	(void)state;
	/* Both canvas reference policies must keep the caller's scene alive. */
	const uint32_t flags[] = {0, SCENE_REF};
	for (size_t i = 0; i < 2; i++) {
		obs_canvas_t *src = obs_canvas_create_private("empty src", NULL, flags[i]);
		obs_canvas_t *dst = obs_canvas_create_private("empty dst", NULL, flags[1 - i]);
		assert_non_null(src);
		assert_non_null(dst);
		obs_scene_t *scene = obs_canvas_scene_create(src, "empty moved");
		assert_non_null(scene);
		obs_canvas_move_scene(scene, dst);
		expect_owner(obs_scene_get_source(scene), dst);
		assert_null(obs_canvas_get_scene_by_name(src, "empty moved"));
		obs_scene_t *found = obs_canvas_get_scene_by_name(dst, "empty moved");
		assert_ptr_equal(found, scene);
		obs_scene_release(found);
		expect_items(scene, 0);
		obs_canvas_move_scene(scene, src);
		expect_owner(obs_scene_get_source(scene), src);
		assert_null(obs_canvas_get_scene_by_name(dst, "empty moved"));
		obs_source_remove(obs_scene_get_source(scene));
		obs_scene_release(scene);
		obs_wait_for_destroy_queue();
		obs_canvas_release(src);
		obs_canvas_release(dst);
		obs_wait_for_destroy_queue();
	}
}

static void migrate_groups(bool same_canvas)
{
	obs_canvas_t *src = obs_canvas_create_private("groups src", NULL, 0);
	obs_canvas_t *dst = same_canvas ? obs_canvas_get_ref(src) : obs_canvas_create_private("groups dst", NULL, 0);
	assert_non_null(src);
	assert_non_null(dst);
	obs_scene_t *scene = obs_canvas_scene_create(src, "groups moved");
	obs_scene_t *observer = obs_canvas_scene_create(src, "groups observer");
	obs_scene_t *ordinary = obs_canvas_scene_create(src, "ordinary child");
	assert_non_null(scene);
	assert_non_null(observer);
	assert_non_null(ordinary);
	obs_sceneitem_t *first = obs_scene_add_group(scene, "first group");
	assert_non_null(first);
	assert_non_null(obs_scene_add(scene, obs_scene_get_source(ordinary)));
	obs_sceneitem_t *second = obs_scene_add_group(scene, "second group");
	assert_non_null(second);
	obs_source_t *groups[] = {obs_sceneitem_get_source(first), obs_sceneitem_get_source(second)};
	obs_source_get_ref(groups[0]);
	obs_source_get_ref(groups[1]);
	assert_non_null(obs_scene_add(observer, groups[0]));
	assert_non_null(obs_scene_add(observer, groups[1]));
	/* Moving a group moves its source, not the ordinary sources inside it. */
	assert_non_null(obs_scene_add(obs_group_from_source(groups[0]), obs_scene_get_source(ordinary)));
	/* Pin enumeration order independently of creation order. */
	obs_sceneitem_set_order(second, OBS_ORDER_MOVE_BOTTOM);
	obs_sceneitem_set_order(first, OBS_ORDER_MOVE_TOP);
	obs_source_t *expected[] = {obs_scene_get_source(scene), groups[1], groups[0]};
	struct migration_log log = {0};
	struct migration_listener listeners[] = {
		{&log, src, false},
		{&log, dst, true},
	};
	signal_handler_connect(obs_canvas_get_signal_handler(src), "source_remove", migration_signal, &listeners[0]);
	signal_handler_connect(obs_canvas_get_signal_handler(dst), "source_add", migration_signal, &listeners[1]);

	obs_canvas_move_scene(scene, dst);

	assert_int_equal(log.count, 6);
	for (size_t i = 0; i < 3; i++) {
		assert_ptr_equal(log.events[2 * i].source, expected[i]);
		assert_ptr_equal(log.events[2 * i].canvas, src);
		assert_false(log.events[2 * i].added);
		assert_ptr_equal(log.events[2 * i + 1].source, expected[i]);
		assert_ptr_equal(log.events[2 * i + 1].canvas, dst);
		assert_true(log.events[2 * i + 1].added);
		expect_owner(expected[i], dst);
	}
	expect_owner(obs_scene_get_source(ordinary), src);
	expect_owner(obs_scene_get_source(observer), src);
	expect_items(scene, same_canvas ? 1 : 3);
	expect_items(obs_group_from_source(groups[0]), 1);
	/* Characterized, not endorsed: remove/insert still runs for the same
	 * canvas, so group items disappear from both old scenes, including the
	 * scene being moved when it is reinserted into that same canvas. */
	expect_items(observer, 0);
	if (same_canvas) {
		assert_null(obs_scene_find_source(scene, "first group"));
		assert_null(obs_scene_find_source(scene, "second group"));
	} else {
		assert_ptr_equal(obs_scene_find_source(scene, "first group"), first);
		assert_ptr_equal(obs_scene_find_source(scene, "second group"), second);
	}
	if (!same_canvas) {
		assert_null(obs_canvas_get_source_by_name(src, "first group"));
		assert_null(obs_canvas_get_source_by_name(src, "second group"));
	}
	obs_source_t *found = obs_canvas_get_source_by_name(dst, "first group");
	assert_ptr_equal(found, groups[0]);
	obs_source_release(found);

	signal_handler_disconnect(obs_canvas_get_signal_handler(src), "source_remove", migration_signal, &listeners[0]);
	signal_handler_disconnect(obs_canvas_get_signal_handler(dst), "source_add", migration_signal, &listeners[1]);
	obs_source_remove(obs_scene_get_source(scene));
	obs_source_remove(obs_scene_get_source(observer));
	obs_source_remove(obs_scene_get_source(ordinary));
	obs_scene_release(scene);
	obs_scene_release(observer);
	obs_scene_release(ordinary);
	obs_source_release(groups[0]);
	obs_source_release(groups[1]);
	obs_wait_for_destroy_queue();
	obs_canvas_release(src);
	obs_canvas_release(dst);
	obs_wait_for_destroy_queue();
}

static void group_migration_order(void **state)
{
	(void)state;
	migrate_groups(false);
}

static void same_canvas_migration(void **state)
{
	(void)state;
	migrate_groups(true);
}

int main(void)
{
	if (!obs_startup("en-US", NULL, NULL))
		return 77;
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(empty_scene_move),
		cmocka_unit_test(group_migration_order),
		cmocka_unit_test(same_canvas_migration),
	};
	int result = cmocka_run_group_tests(tests, NULL, NULL);
	obs_shutdown();
	return result;
}
