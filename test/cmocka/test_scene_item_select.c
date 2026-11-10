/* Characterize the original C selection contract before its Rust swap.
 * Public APIs only; run against both hybrid libobs and libobs-c-reference. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "cmocka-compat.h"
#include <obs.h>

struct selection_probe {
	obs_scene_t *scene;
	obs_sceneitem_t *item;
	unsigned selects;
	unsigned deselects;
	bool reenter;
};

static void selected(void *data, calldata_t *params)
{
	struct selection_probe *probe = data;
	assert_ptr_equal(calldata_ptr(params, "scene"), probe->scene);
	assert_ptr_equal(calldata_ptr(params, "item"), probe->item);
	/* The write precedes synchronous delivery, even on a nested call. */
	assert_true(obs_sceneitem_selected(probe->item));
	probe->selects++;
	obs_sceneitem_select(probe->item, true);
	assert_int_equal(probe->selects, 1);
	if (probe->reenter) {
		obs_sceneitem_select(probe->item, false);
		assert_int_equal(probe->deselects, 1);
		assert_false(obs_sceneitem_selected(probe->item));
	}
}

static void deselected(void *data, calldata_t *params)
{
	struct selection_probe *probe = data;
	assert_ptr_equal(calldata_ptr(params, "scene"), probe->scene);
	assert_ptr_equal(calldata_ptr(params, "item"), probe->item);
	assert_false(obs_sceneitem_selected(probe->item));
	probe->deselects++;
	obs_sceneitem_select(probe->item, false);
	assert_int_equal(probe->deselects, 1);
}

static void count_signal(void *data, calldata_t *params)
{
	(void)params;
	(*(unsigned *)data)++;
}

static void null_item(void **state)
{
	(void)state;
	obs_sceneitem_select(NULL, false);
	obs_sceneitem_select(NULL, true);
}

static void exercise_selection(bool grouped, bool reenter)
{
	obs_scene_t *root = obs_scene_create_private("selection root");
	obs_scene_t *leaf = obs_scene_create_private("selection leaf");
	assert_non_null(root);
	assert_non_null(leaf);
	obs_scene_t *parent = root;
	if (grouped) {
		obs_sceneitem_t *group = obs_scene_add_group(root, "selection group");
		assert_non_null(group);
		parent = obs_sceneitem_group_get_scene(group);
	}
	obs_sceneitem_t *item = obs_scene_add(parent, obs_scene_get_source(leaf));
	assert_non_null(item);
	struct selection_probe probe = {.scene = parent, .item = item, .reenter = reenter};
	signal_handler_t *handler = obs_source_get_signal_handler(obs_scene_get_source(parent));
	signal_handler_t *root_handler = obs_source_get_signal_handler(obs_scene_get_source(root));
	unsigned root_signals = 0;
	if (grouped) {
		signal_handler_connect(root_handler, "item_select", count_signal, &root_signals);
		signal_handler_connect(root_handler, "item_deselect", count_signal, &root_signals);
	}
	signal_handler_connect(handler, "item_select", selected, &probe);
	signal_handler_connect(handler, "item_deselect", deselected, &probe);
	assert_false(obs_sceneitem_selected(item));
	obs_sceneitem_select(item, false);
	assert_int_equal(probe.selects, 0);
	assert_int_equal(probe.deselects, 0);
	obs_sceneitem_select(item, true);
	assert_int_equal(probe.selects, 1);
	assert_int_equal(probe.deselects, reenter ? 1 : 0);
	assert_int_equal(obs_sceneitem_selected(item), !reenter);
	if (!reenter) {
		obs_sceneitem_select(item, true);
		assert_int_equal(probe.selects, 1);
		obs_sceneitem_select(item, false);
	}
	assert_int_equal(probe.deselects, 1);
	obs_sceneitem_select(item, false);
	assert_int_equal(probe.deselects, 1);
	/* Characterized, not endorsed: a group emits only on its own scene. */
	assert_int_equal(root_signals, 0);

	obs_sceneitem_addref(item);
	obs_sceneitem_remove(item);
	assert_null(obs_sceneitem_get_scene(item));
	obs_sceneitem_select(item, true);
	obs_sceneitem_select(item, false);
	assert_false(obs_sceneitem_selected(item));
	assert_int_equal(probe.selects, 1);
	assert_int_equal(probe.deselects, 1);
	obs_sceneitem_release(item);

	signal_handler_disconnect(handler, "item_select", selected, &probe);
	signal_handler_disconnect(handler, "item_deselect", deselected, &probe);
	if (grouped) {
		signal_handler_disconnect(root_handler, "item_select", count_signal, &root_signals);
		signal_handler_disconnect(root_handler, "item_deselect", count_signal, &root_signals);
	}
	obs_scene_release(root);
	obs_scene_release(leaf);
	obs_wait_for_destroy_queue();
}

static void scene_selection(void **state)
{
	(void)state;
	exercise_selection(false, false);
}

static void group_selection(void **state)
{
	(void)state;
	exercise_selection(true, false);
}

static void reentrant_selection(void **state)
{
	(void)state;
	exercise_selection(false, true);
	exercise_selection(true, true);
}

int main(void)
{
	if (!obs_startup("en-US", NULL, NULL))
		return 77;
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_item),
		cmocka_unit_test(scene_selection),
		cmocka_unit_test(group_selection),
		cmocka_unit_test(reentrant_selection),
	};
	int result = cmocka_run_group_tests(tests, NULL, NULL);
	obs_shutdown();
	return result;
}
