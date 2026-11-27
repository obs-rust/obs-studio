/* Direct characterization of obs_scene_reorder_items before extraction.
 * Public API only; no video device or rendering. Invalid pointer inputs are
 * outside the API contract. Current no-op/refusal results are characterized,
 * not endorsed. Run against both C-reference and hybrid libobs. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "cmocka-compat.h"
#include <obs.h>
#include <util/bmem.h>

#define LEAF_ID "scene_reorder_characterization_leaf"

struct fixture {
	obs_scene_t *scene;
	obs_scene_t *foreign;
	obs_source_t *sources[4];
	obs_sceneitem_t *items[4];
	obs_sceneitem_t *expected[3];
	size_t expected_count;
	int signals;
	bool release_in_signal;
	obs_weak_source_t *weak;
};

static const char *leaf_name(void *unused)
{
	(void)unused;
	return "reorder characterization leaf";
}

static void *leaf_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	(void)source;
	return bzalloc(1);
}

static void leaf_destroy(void *data)
{
	bfree(data);
}

static uint32_t leaf_size(void *unused)
{
	(void)unused;
	return 16;
}

struct enumeration {
	obs_sceneitem_t **expected;
	size_t count;
	size_t seen;
};

static bool check_item(obs_scene_t *scene, obs_sceneitem_t *item, void *opaque)
{
	struct enumeration *walk = opaque;
	assert_true(walk->seen < walk->count);
	assert_ptr_equal(item, walk->expected[walk->seen]);
	assert_ptr_equal(obs_sceneitem_get_scene(item), scene);
	assert_int_equal(obs_sceneitem_get_order_position(item), (int)walk->seen);
	walk->seen++;
	return true;
}

static void check_order(obs_scene_t *scene, obs_sceneitem_t **expected, size_t count)
{
	struct enumeration walk = {expected, count, 0};
	obs_scene_enum_items(scene, check_item, &walk);
	assert_int_equal(walk.seen, count);
}

static void reordered(void *opaque, calldata_t *data)
{
	struct fixture *fx = opaque;
	assert_ptr_equal(calldata_ptr(data, "scene"), fx->scene);
	check_order(fx->scene, fx->expected, fx->expected_count);
	fx->signals++;
	if (fx->release_in_signal) {
		/* The API's retained scene keeps the source promotable after the
		 * caller releases its last reference from the synchronous signal. */
		obs_scene_release(fx->scene);
		obs_source_t *held = obs_weak_source_get_source(fx->weak);
		assert_non_null(held);
		obs_source_release(held);
		fx->scene = NULL;
	}
}

static int setup(void **state)
{
	struct fixture *fx = bzalloc(sizeof(*fx));
	*state = fx;
	fx->scene = obs_scene_create_private("reorder scene");
	fx->foreign = obs_scene_create_private("foreign scene");
	assert_non_null(fx->scene);
	assert_non_null(fx->foreign);
	for (size_t i = 0; i < 4; i++) {
		fx->sources[i] = obs_source_create_private(LEAF_ID, "leaf", NULL);
		assert_non_null(fx->sources[i]);
		fx->items[i] = obs_scene_add(i < 3 ? fx->scene : fx->foreign, fx->sources[i]);
		assert_non_null(fx->items[i]);
		if (i < 3)
			fx->expected[i] = fx->items[i];
	}
	fx->expected_count = 3;
	signal_handler_connect(obs_source_get_signal_handler(obs_scene_get_source(fx->scene)), "reorder", reordered,
			       fx);
	return 0;
}

static int teardown(void **state)
{
	struct fixture *fx = *state;
	if (fx->scene) {
		signal_handler_disconnect(obs_source_get_signal_handler(obs_scene_get_source(fx->scene)), "reorder",
					  reordered, fx);
		obs_scene_release(fx->scene);
	}
	obs_scene_release(fx->foreign);
	for (size_t i = 0; i < 4; i++)
		obs_source_release(fx->sources[i]);
	obs_wait_for_destroy_queue();
	if (fx->weak)
		obs_weak_source_release(fx->weak);
	bfree(fx);
	return 0;
}

static void null_and_zero(void **state)
{
	struct fixture *fx = *state;
	assert_false(obs_scene_reorder_items(NULL, NULL, 0));
	assert_false(obs_scene_reorder_items(NULL, fx->items, 3));
	assert_false(obs_scene_reorder_items(fx->scene, NULL, 0));
	check_order(fx->scene, fx->expected, 3);
	assert_int_equal(fx->signals, 0);
}

static void empty_single_and_unchanged(void **state)
{
	struct fixture *fx = *state;
	obs_scene_t *empty = obs_scene_create_private("empty reorder scene");
	assert_non_null(empty);
	assert_false(obs_scene_reorder_items(empty, NULL, 0));
	assert_false(obs_scene_reorder_items(empty, fx->items, 1));
	check_order(empty, NULL, 0);
	obs_scene_release(empty);
	assert_false(obs_scene_reorder_items(fx->foreign, &fx->items[3], 1));
	check_order(fx->foreign, &fx->items[3], 1);
	assert_false(obs_scene_reorder_items(fx->scene, fx->items, 3));
	check_order(fx->scene, fx->expected, 3);
	assert_int_equal(fx->signals, 0);
}

static void rejected_arrays(void **state)
{
	struct fixture *fx = *state;
	obs_sceneitem_t *duplicate[] = {fx->items[2], fx->items[0], fx->items[0]};
	obs_sceneitem_t *foreign[] = {fx->items[2], fx->items[0], fx->items[3]};
	obs_sceneitem_t *null_member[] = {fx->items[2], NULL, fx->items[0]};
	assert_false(obs_scene_reorder_items(fx->scene, fx->items, 2));
	assert_false(obs_scene_reorder_items(fx->scene, fx->items, 4));
	assert_false(obs_scene_reorder_items(fx->scene, duplicate, 3));
	assert_false(obs_scene_reorder_items(fx->scene, foreign, 3));
	assert_false(obs_scene_reorder_items(fx->scene, null_member, 3));
	check_order(fx->scene, fx->expected, 3);
	check_order(fx->foreign, &fx->items[3], 1);
	assert_int_equal(fx->signals, 0);
}

static void all_permutations(void **state)
{
	struct fixture *fx = *state;
	static const size_t permutations[][3] = {{2, 1, 0}, {2, 0, 1}, {1, 2, 0}, {1, 0, 2}, {0, 2, 1}, {0, 1, 2}};
	for (size_t i = 0; i < 6; i++) {
		for (size_t j = 0; j < 3; j++)
			fx->expected[j] = fx->items[permutations[i][j]];
		assert_true(obs_scene_reorder_items(fx->scene, fx->expected, 3));
		assert_int_equal(fx->signals, (int)i + 1);
		check_order(fx->scene, fx->expected, 3);
		assert_false(obs_scene_reorder_items(fx->scene, fx->expected, 3));
		assert_int_equal(fx->signals, (int)i + 1);
	}
}

static void links_survive_removal(void **state)
{
	struct fixture *fx = *state;
	fx->expected[0] = fx->items[2];
	fx->expected[1] = fx->items[0];
	fx->expected[2] = fx->items[1];
	assert_true(obs_scene_reorder_items(fx->scene, fx->expected, 3));
	/* Removing middle, head and tail exercises detach's prev/next writes. */
	obs_sceneitem_remove(fx->items[0]);
	fx->expected[1] = fx->items[1];
	check_order(fx->scene, fx->expected, 2);
	obs_sceneitem_remove(fx->items[2]);
	check_order(fx->scene, &fx->items[1], 1);
	obs_sceneitem_remove(fx->items[1]);
	check_order(fx->scene, NULL, 0);
	assert_int_equal(fx->signals, 1);
}

static void signal_retains_scene(void **state)
{
	struct fixture *fx = *state;
	fx->weak = obs_source_get_weak_source(obs_scene_get_source(fx->scene));
	assert_non_null(fx->weak);
	fx->expected[0] = fx->items[2];
	fx->expected[2] = fx->items[0];
	fx->release_in_signal = true;
	assert_true(obs_scene_reorder_items(fx->scene, fx->expected, 3));
	assert_null(fx->scene);
	assert_int_equal(fx->signals, 1);
	obs_wait_for_destroy_queue();
	assert_true(obs_weak_source_expired(fx->weak));
	assert_null(obs_weak_source_get_source(fx->weak));
}

int main(void)
{
	if (!obs_startup("en-US", NULL, NULL))
		return 77;
	struct obs_source_info leaf = {
		.id = LEAF_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = leaf_name,
		.create = leaf_create,
		.destroy = leaf_destroy,
		.get_width = leaf_size,
		.get_height = leaf_size,
	};
	obs_register_source(&leaf);
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(null_and_zero, setup, teardown),
		cmocka_unit_test_setup_teardown(empty_single_and_unchanged, setup, teardown),
		cmocka_unit_test_setup_teardown(rejected_arrays, setup, teardown),
		cmocka_unit_test_setup_teardown(all_permutations, setup, teardown),
		cmocka_unit_test_setup_teardown(links_survive_removal, setup, teardown),
		cmocka_unit_test_setup_teardown(signal_retains_scene, setup, teardown),
	};
	int result = cmocka_run_group_tests(tests, NULL, NULL);
	obs_wait_for_destroy_queue();
	obs_shutdown();
	return result;
}
