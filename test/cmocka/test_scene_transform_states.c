/* Characterization of scene transform snapshots/restore before the Rust swap.
 * Uses actual scene/source ownership and the public C API in both libobs builds.
 * Absolute coordinates avoid depending on a running video thread or GPU. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <limits.h>
#include <stdint.h>
#include <string.h>
#include <obs.h>

#define LEAF_ID "scene_transform_states_leaf"

static const char *leaf_name(void *data)
{
	(void)data;
	return "snapshot leaf";
}

static void *leaf_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void leaf_destroy(void *data)
{
	/* leaf_create borrows the source pointer; it owns no allocation. */
	(void)data;
}

static uint32_t leaf_size(void *data)
{
	(void)data;
	return 128;
}

struct fixture {
	obs_scene_t *scene;
	obs_source_t *leaf;
	obs_sceneitem_t *first;
	obs_sceneitem_t *second;
};

static int setup(void **state)
{
	if (!obs_startup("en-US", NULL, NULL))
		return -1;
	obs_data_t *settings = obs_get_private_data();
	obs_data_set_bool(settings, "AbsoluteCoordinates", true);
	obs_data_release(settings);
	struct obs_source_info leaf = {.id = LEAF_ID,
				       .type = OBS_SOURCE_TYPE_INPUT,
				       .output_flags = OBS_SOURCE_VIDEO,
				       .get_name = leaf_name,
				       .create = leaf_create,
				       .destroy = leaf_destroy,
				       .get_width = leaf_size,
				       .get_height = leaf_size};
	obs_register_source(&leaf);
	struct fixture *f = bzalloc(sizeof(*f));
	f->scene = obs_scene_create("snapshot scene");
	f->leaf = obs_source_create(LEAF_ID, "snapshot leaf", NULL, NULL);
	assert_non_null(f->scene);
	assert_non_null(f->leaf);
	f->first = obs_scene_add(f->scene, f->leaf);
	f->second = obs_scene_add(f->scene, f->leaf);
	assert_non_null(f->first);
	assert_non_null(f->second);
	*state = f;
	return 0;
}

static int teardown(void **state)
{
	struct fixture *f = *state;
	obs_scene_release(f->scene);
	obs_source_release(f->leaf);
	obs_wait_for_destroy_queue();
	bfree(f);
	obs_shutdown();
	return 0;
}

static struct obs_transform_info transform(void)
{
	struct obs_transform_info info = {0};
	vec2_set(&info.pos, 37.25f, -19.5f);
	vec2_set(&info.scale, -1.25f, 2.5f);
	info.rot = 31.5f;
	info.alignment = OBS_ALIGN_RIGHT | OBS_ALIGN_BOTTOM;
	info.bounds_type = OBS_BOUNDS_SCALE_OUTER;
	info.bounds_alignment = OBS_ALIGN_LEFT | OBS_ALIGN_TOP;
	vec2_set(&info.bounds, 333.0f, 222.0f);
	info.crop_to_bounds = true;
	return info;
}

static void configure(obs_sceneitem_t *item)
{
	struct obs_transform_info info = transform();
	struct obs_sceneitem_crop crop = {.left = 3, .top = 5, .right = 7, .bottom = 11};
	obs_sceneitem_set_info2(item, &info);
	obs_sceneitem_set_crop(item, &crop);
}

static void expect_transform(obs_sceneitem_t *item)
{
	struct obs_transform_info actual;
	struct obs_transform_info expected = transform();
	struct obs_sceneitem_crop crop;
	obs_sceneitem_get_info2(item, &actual);
	obs_sceneitem_get_crop(item, &crop);
	assert_true(actual.pos.x == expected.pos.x);
	assert_true(actual.pos.y == expected.pos.y);
	assert_true(actual.scale.x == expected.scale.x);
	assert_true(actual.scale.y == expected.scale.y);
	assert_true(actual.rot == expected.rot);
	assert_int_equal(actual.alignment, expected.alignment);
	assert_int_equal(actual.bounds_type, expected.bounds_type);
	assert_int_equal(actual.bounds_alignment, expected.bounds_alignment);
	assert_true(actual.bounds.x == expected.bounds.x);
	assert_true(actual.bounds.y == expected.bounds.y);
	assert_true(actual.crop_to_bounds);
	assert_int_equal(crop.left, 3);
	assert_int_equal(crop.top, 5);
	assert_int_equal(crop.right, 7);
	assert_int_equal(crop.bottom, 11);
}

static obs_data_t *record_at(obs_data_t *snapshot, size_t index)
{
	obs_data_array_t *records = obs_data_get_array(snapshot, "scenes_and_groups");
	obs_data_t *record = obs_data_array_item(records, index);
	obs_data_array_release(records);
	assert_non_null(record);
	return record;
}

static void expect_item_record(obs_data_t *record, size_t index, obs_sceneitem_t *item)
{
	obs_data_array_t *items = obs_data_get_array(record, "items");
	obs_data_t *saved = obs_data_array_item(items, index);
	assert_non_null(saved);
	assert_int_equal(obs_data_get_int(saved, "id"), obs_sceneitem_get_id(item));
	struct vec2 value;
	obs_data_get_vec2(saved, "pos", &value);
	assert_true(value.x == 37.25f && value.y == -19.5f);
	obs_data_get_vec2(saved, "scale", &value);
	assert_true(value.x == -1.25f && value.y == 2.5f);
	obs_data_get_vec2(saved, "bounds", &value);
	assert_true(value.x == 333.0f && value.y == 222.0f);
	assert_true(obs_data_get_double(saved, "rot") == 31.5);
	assert_int_equal(obs_data_get_int(saved, "alignment"), OBS_ALIGN_RIGHT | OBS_ALIGN_BOTTOM);
	assert_int_equal(obs_data_get_int(saved, "bounds_type"), OBS_BOUNDS_SCALE_OUTER);
	assert_int_equal(obs_data_get_int(saved, "bounds_alignment"), OBS_ALIGN_LEFT | OBS_ALIGN_TOP);
	assert_true(obs_data_get_bool(saved, "crop_to_bounds"));
	assert_int_equal(obs_data_get_int(saved, "left"), 3);
	assert_int_equal(obs_data_get_int(saved, "top"), 5);
	assert_int_equal(obs_data_get_int(saved, "right"), 7);
	assert_int_equal(obs_data_get_int(saved, "bottom"), 11);
	obs_data_release(saved);
	obs_data_array_release(items);
}

static size_t item_count(obs_data_t *record)
{
	obs_data_array_t *items = obs_data_get_array(record, "items");
	size_t count = obs_data_array_count(items);
	obs_data_array_release(items);
	return count;
}

static void selected_and_all_snapshots(void **state)
{
	struct fixture *f = *state;
	configure(f->first);
	configure(f->second);
	obs_sceneitem_select(f->second, true);
	obs_data_t *snapshot = obs_scene_save_transform_states(f->scene, false);
	obs_data_array_t *records = obs_data_get_array(snapshot, "scenes_and_groups");
	assert_int_equal(obs_data_array_count(records), 1);
	obs_data_array_release(records);
	obs_data_t *record = record_at(snapshot, 0);
	assert_string_equal(obs_data_get_string(record, "scene_name"), "snapshot scene");
	assert_string_equal(obs_data_get_string(record, "scene_uuid"),
			    obs_source_get_uuid(obs_scene_get_source(f->scene)));
	assert_false(obs_data_get_bool(record, "is_group"));
	assert_int_equal(item_count(record), 1);
	expect_item_record(record, 0, f->second);
	obs_data_release(record);
	obs_data_release(snapshot);

	snapshot = obs_scene_save_transform_states(f->scene, true);
	record = record_at(snapshot, 0);
	assert_int_equal(item_count(record), 2);
	expect_item_record(record, 0, f->first);
	expect_item_record(record, 1, f->second);
	obs_data_release(record);
	obs_data_release(snapshot);

	obs_sceneitem_select(f->second, false);
	snapshot = obs_scene_save_transform_states(f->scene, false);
	record = record_at(snapshot, 0);
	assert_int_equal(item_count(record), 0);
	obs_data_release(record);
	obs_data_release(snapshot);
}

static void count_transform(void *data, calldata_t *params)
{
	(void)params;
	(*(unsigned *)data)++;
}

static void restore_by_uuid_and_defer(void **state)
{
	struct fixture *f = *state;
	configure(f->first);
	obs_sceneitem_select(f->first, true);
	obs_data_t *snapshot = obs_scene_save_transform_states(f->scene, false);
	struct obs_transform_info zero = {0};
	obs_sceneitem_set_info2(f->first, &zero);
	struct obs_sceneitem_crop no_crop = {0};
	obs_sceneitem_set_crop(f->first, &no_crop);
	obs_source_set_name(obs_scene_get_source(f->scene), "renamed scene");
	obs_sceneitem_select(f->first, false);
	unsigned calls = 0;
	signal_handler_t *signals = obs_source_get_signal_handler(obs_scene_get_source(f->scene));
	signal_handler_connect(signals, "item_transform", count_transform, &calls);
	obs_sceneitem_defer_update_begin(f->first);
	obs_scene_load_transform_states(obs_data_get_json(snapshot));
	assert_int_equal(calls, 0);
	expect_transform(f->first);
	obs_sceneitem_defer_update_end(f->first);
	assert_int_equal(calls, 1);
	signal_handler_disconnect(signals, "item_transform", count_transform, &calls);
	assert_false(obs_sceneitem_selected(f->first));
	assert_false(obs_sceneitem_selected(f->second));
	assert_true(obs_sceneitem_get_rot(f->second) == 0.0f);
	obs_data_release(snapshot);
}

static void nested_groups_and_parent_resolution(void **state)
{
	struct fixture *f = *state;
	obs_sceneitem_t *outer = obs_scene_add_group(f->scene, "outer");
	assert_non_null(outer);
	obs_scene_t *outer_scene = obs_sceneitem_group_get_scene(outer);
	obs_sceneitem_t *inner = obs_scene_add_group(outer_scene, "inner");
	assert_non_null(inner);
	obs_scene_t *inner_scene = obs_sceneitem_group_get_scene(inner);
	obs_sceneitem_t *child = obs_scene_add(inner_scene, f->leaf);
	assert_non_null(child);
	configure(child);
	obs_sceneitem_select(child, true);
	obs_sceneitem_t *outer_child = obs_scene_add(outer_scene, f->leaf);
	assert_non_null(outer_child);
	configure(outer_child);
	obs_sceneitem_select(outer_child, true);
	obs_data_t *snapshot = obs_scene_save_transform_states(f->scene, false);
	obs_data_array_t *records = obs_data_get_array(snapshot, "scenes_and_groups");
	assert_int_equal(obs_data_array_count(records), 3);
	obs_data_array_release(records);
	/* Depth-first group records precede their parent, even for unselected groups. */
	obs_data_t *inner_record = record_at(snapshot, 0);
	obs_data_t *outer_record = record_at(snapshot, 1);
	obs_data_t *root_record = record_at(snapshot, 2);
	assert_true(obs_data_get_bool(inner_record, "is_group"));
	assert_true(obs_data_get_bool(outer_record, "is_group"));
	assert_false(obs_data_get_bool(root_record, "is_group"));
	assert_string_equal(obs_data_get_string(inner_record, "scene_name"), "inner");
	assert_string_equal(obs_data_get_string(inner_record, "scene_uuid"),
			    obs_source_get_uuid(obs_scene_get_source(inner_scene)));
	assert_string_equal(obs_data_get_string(inner_record, "group_parent"),
			    obs_source_get_uuid(obs_scene_get_source(outer_scene)));
	assert_string_equal(obs_data_get_string(outer_record, "group_parent"),
			    obs_source_get_uuid(obs_scene_get_source(f->scene)));
	assert_int_equal(item_count(root_record), 0);
	assert_int_equal(item_count(outer_record), 1);
	expect_item_record(outer_record, 0, outer_child);
	assert_int_equal(item_count(inner_record), 1);
	expect_item_record(inner_record, 0, child);
	obs_data_t *all = obs_scene_save_transform_states(f->scene, true);
	for (size_t i = 0; i < 3; i++) {
		obs_data_t *all_record = record_at(all, i);
		assert_int_equal(item_count(all_record), i + 1);
		obs_data_release(all_record);
	}
	obs_data_release(all);
	struct obs_transform_info zero = {0};
	obs_sceneitem_set_info2(child, &zero);
	obs_sceneitem_set_info2(outer_child, &zero);
	/* Characterized, not endorsed: a group's parent is resolved using
	 * obs_scene_from_source, which rejects a group source. Nested group
	 * children are serialized but are not restored. */
	obs_scene_load_transform_states(obs_data_get_json(snapshot));
	expect_transform(outer_child);
	struct obs_transform_info nested_info;
	obs_sceneitem_get_info2(child, &nested_info);
	assert_true(nested_info.pos.x == 0.0f && nested_info.pos.y == 0.0f);
	assert_true(nested_info.rot == 0.0f);
	/* Characterized, not endorsed: group UUID is ignored in favor of parent/name. */
	obs_data_set_string(outer_record, "scene_uuid", "missing group uuid");
	obs_sceneitem_set_info2(outer_child, &zero);
	obs_scene_load_transform_states(obs_data_get_json(snapshot));
	expect_transform(outer_child);
	obs_data_release(inner_record);
	obs_data_release(outer_record);
	obs_data_release(root_record);
	obs_data_release(snapshot);

	/* Restoring must release both the UUID and group-parent lookups. */
	/* Group scenes are borrowed from their items. Removing a canvas owner
	 * also removes group items and can queue the last source release before
	 * the removal call returns. Keep caller-owned refs across those calls. */
	obs_scene_t *outer_hold = obs_scene_get_ref(outer_scene);
	obs_scene_t *inner_hold = obs_scene_get_ref(inner_scene);
	assert_non_null(outer_hold);
	assert_non_null(inner_hold);
	obs_weak_source_t *root_weak = obs_source_get_weak_source(obs_scene_get_source(f->scene));
	obs_weak_source_t *outer_weak = obs_source_get_weak_source(obs_scene_get_source(outer_scene));
	obs_weak_source_t *inner_weak = obs_source_get_weak_source(obs_scene_get_source(inner_scene));
	/* Public scenes/groups also belong to the canvas. Drop those real
	 * owners before testing whether restore leaked a lookup reference. */
	obs_canvas_scene_remove(inner_scene);
	obs_canvas_scene_remove(outer_scene);
	obs_canvas_scene_remove(f->scene);
	obs_scene_release(inner_hold);
	obs_scene_release(outer_hold);
	obs_scene_release(f->scene);
	f->scene = NULL;
	while (obs_wait_for_destroy_queue()) {
	}
	assert_true(obs_weak_source_expired(root_weak));
	assert_true(obs_weak_source_expired(outer_weak));
	assert_true(obs_weak_source_expired(inner_weak));
	assert_null(obs_weak_source_get_source(root_weak));
	assert_null(obs_weak_source_get_source(outer_weak));
	assert_null(obs_weak_source_get_source(inner_weak));
	obs_weak_source_release(root_weak);
	obs_weak_source_release(outer_weak);
	obs_weak_source_release(inner_weak);
}

static void bounds_and_missing_records(void **state)
{
	struct fixture *f = *state;
	configure(f->first);
	obs_data_t *snapshot = obs_scene_save_transform_states(f->scene, true);
	obs_data_t *record = record_at(snapshot, 0);
	obs_data_array_t *items = obs_data_get_array(record, "items");
	obs_data_t *saved = obs_data_array_item(items, 0);
	const int64_t invalid[] = {-1, OBS_BOUNDS_MAX_ONLY + 1, INT64_MIN, INT64_MAX};
	for (size_t i = 0; i < sizeof(invalid) / sizeof(invalid[0]); i++) {
		obs_data_set_int(saved, "bounds_type", invalid[i]);
		obs_scene_load_transform_states(obs_data_get_json(snapshot));
		assert_int_equal(obs_sceneitem_get_bounds_type(f->first), OBS_BOUNDS_NONE);
	}
	for (int64_t type = OBS_BOUNDS_NONE; type <= OBS_BOUNDS_MAX_ONLY; type++) {
		obs_data_set_int(saved, "bounds_type", type);
		obs_scene_load_transform_states(obs_data_get_json(snapshot));
		assert_int_equal(obs_sceneitem_get_bounds_type(f->first), type);
	}
	configure(f->first);
	obs_data_set_int(saved, "id", INT64_MAX);
	obs_scene_load_transform_states(obs_data_get_json(snapshot));
	expect_transform(f->first);
	obs_data_set_string(record, "scene_uuid", "missing scene uuid");
	obs_scene_load_transform_states(obs_data_get_json(snapshot));
	expect_transform(f->first);
	obs_scene_load_transform_states("{}");
	obs_scene_load_transform_states("invalid json");
	expect_transform(f->first);
	obs_data_release(saved);
	obs_data_array_release(items);
	obs_data_release(record);
	obs_data_release(snapshot);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(selected_and_all_snapshots, setup, teardown),
		cmocka_unit_test_setup_teardown(restore_by_uuid_and_defer, setup, teardown),
		cmocka_unit_test_setup_teardown(nested_groups_and_parent_resolution, setup, teardown),
		cmocka_unit_test_setup_teardown(bounds_and_missing_records, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
