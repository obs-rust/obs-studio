/* Direct characterization of the real public force-update API before extraction.
 * Deferred calls consume dirtiness even when no transform is computed:
 * characterized, not endorsed. No graphics pipeline is required. */
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <cmocka.h>

#include <obs.h>
#include <util/threading.h>
#include <obs-scene.h>
#include <util/base.h>

static unsigned width_reads, height_reads, signals, logs;
static obs_sceneitem_t *expected_item;
static bool redirty;

static void quiet_log(int level, const char *format, va_list args, void *param)
{
	(void)level;
	(void)format;
	(void)args;
	(void)param;
	logs++;
}

static const char *source_name(void *data)
{
	(void)data;
	return "force update size source";
}

static void *source_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void source_destroy(void *data)
{
	/* create returns the borrowed source as an inert, non-owning token. */
	(void)data;
}

static uint32_t source_width(void *data)
{
	(void)data;
	width_reads++;
	return 100;
}

static uint32_t source_height(void *data)
{
	(void)data;
	height_reads++;
	return 50;
}

static void on_transform(void *param, calldata_t *cd)
{
	(void)param;
	assert_ptr_equal(calldata_ptr(cd, "item"), expected_item);
	assert_false(expected_item->update_transform);
	assert_int_equal(expected_item->last_width, 100);
	assert_int_equal(expected_item->last_height, 50);
	assert_float_equal(expected_item->draw_transform.t.x, 17.0f, 0.0001f);
	assert_int_equal(width_reads, signals + 1);
	assert_int_equal(height_reads, signals + 1);
	signals++;
	if (redirty)
		expected_item->update_transform = true;
}

struct fixture {
	obs_scene_t *scene;
	obs_sceneitem_t *item;
};

static void reset_observations(void)
{
	width_reads = height_reads = signals = logs = 0;
	redirty = false;
}

static void build_fixture(struct fixture *f, bool group)
{
	f->scene = obs_scene_create_private("force update scene");
	assert_non_null(f->scene);
	obs_source_t *source = obs_source_create_private("force_update_size_source", "size", NULL);
	assert_non_null(source);
	if (group) {
		obs_sceneitem_t *group_item = obs_scene_add_group(f->scene, "group");
		assert_non_null(group_item);
		f->item = obs_scene_add(obs_sceneitem_group_get_scene(group_item), source);
	} else {
		f->item = obs_scene_add(f->scene, source);
	}
	assert_non_null(f->item);
	obs_source_release(source);
	f->item->absolute_coordinates = true;
	vec2_set(&f->item->pos, 17.0f, 23.0f);
	vec2_set(&f->item->scale, 2.0f, 3.0f);
	f->item->rot = 0.0f;
	f->item->align = OBS_ALIGN_TOP | OBS_ALIGN_LEFT;
	f->item->bounds_type = OBS_BOUNDS_NONE;
	f->item->defer_update = 0;
	expected_item = f->item;
	signal_handler_connect(obs_source_get_signal_handler(obs_scene_get_source(f->item->parent)), "item_transform",
			       on_transform, NULL);
	reset_observations();
}

static void free_fixture(struct fixture *f)
{
	signal_handler_disconnect(obs_source_get_signal_handler(obs_scene_get_source(f->item->parent)),
				  "item_transform", on_transform, NULL);
	obs_scene_release(f->scene);
	while (obs_wait_for_destroy_queue())
		;
	expected_item = NULL;
}

static void null_is_silent(void **state)
{
	(void)state;
	reset_observations();
	obs_sceneitem_force_update_transform(NULL);
	assert_int_equal(logs, 0);
	assert_int_equal(width_reads + height_reads + signals, 0);
}

static void dirty_then_clean(void **state)
{
	(void)state;
	struct fixture f;
	build_fixture(&f, false);
	f.item->update_transform = false;
	f.item->last_width = 7;
	obs_sceneitem_force_update_transform(f.item);
	assert_int_equal(width_reads + height_reads + signals, 0);
	assert_int_equal(f.item->last_width, 7);
	f.item->update_transform = true;
	obs_sceneitem_force_update_transform(f.item);
	assert_false(f.item->update_transform);
	assert_int_equal(signals, 1);
	assert_float_equal(f.item->draw_transform.t.y, 23.0f, 0.0001f);
	assert_float_equal(f.item->output_scale.x, 2.0f, 0.0001f);
	assert_float_equal(f.item->output_scale.y, 3.0f, 0.0001f);
	assert_int_equal(f.item->bounds_crop.left + f.item->bounds_crop.right, 0);
	obs_sceneitem_force_update_transform(f.item);
	assert_int_equal(width_reads, 1);
	assert_int_equal(height_reads, 1);
	assert_int_equal(signals, 1);
	free_fixture(&f);
}

static void deferred_consumes_flag(void **state)
{
	(void)state;
	struct fixture f;
	build_fixture(&f, false);
	f.item->defer_update = 2;
	f.item->update_transform = true;
	f.item->last_width = 7;
	obs_sceneitem_force_update_transform(f.item);
	assert_false(f.item->update_transform);
	assert_int_equal(f.item->defer_update, 2);
	assert_int_equal(f.item->last_width, 7);
	assert_int_equal(width_reads + height_reads + signals, 0);
	f.item->defer_update = 0;
	obs_sceneitem_force_update_transform(f.item);
	assert_int_equal(width_reads + height_reads + signals, 0);
	f.item->defer_update = -1;
	f.item->update_transform = true;
	obs_sceneitem_force_update_transform(f.item);
	assert_int_equal(signals, 1);
	assert_int_equal(f.item->defer_update, -1);
	assert_false(f.item->update_transform);
	free_fixture(&f);
}

static void group_child_computes_immediately(void **state)
{
	(void)state;
	struct fixture f;
	build_fixture(&f, true);
	f.item->update_transform = true;
	obs_sceneitem_force_update_transform(f.item);
	assert_false(f.item->update_transform);
	assert_int_equal(signals, 1);
	free_fixture(&f);
}

static void parentless_clean_and_deferred(void **state)
{
	(void)state;
	struct fixture f;
	build_fixture(&f, false);
	obs_scene_t *parent = f.item->parent;
	f.item->parent = NULL;
	/* A dirty, non-deferred parentless item is outside this API's
	 * preconditions: the helper signals its parent unconditionally. */
	f.item->update_transform = false;
	obs_sceneitem_force_update_transform(f.item);
	assert_int_equal(width_reads + height_reads + signals, 0);
	f.item->update_transform = true;
	f.item->defer_update = 1;
	obs_sceneitem_force_update_transform(f.item);
	assert_false(f.item->update_transform);
	assert_int_equal(f.item->defer_update, 1);
	assert_int_equal(width_reads + height_reads + signals, 0);
	f.item->defer_update = 0;
	f.item->parent = parent;
	free_fixture(&f);
}

static void signal_can_redirty(void **state)
{
	(void)state;
	struct fixture f;
	build_fixture(&f, false);
	redirty = true;
	f.item->update_transform = true;
	obs_sceneitem_force_update_transform(f.item);
	assert_true(f.item->update_transform);
	assert_int_equal(signals, 1);
	redirty = false;
	obs_sceneitem_force_update_transform(f.item);
	assert_false(f.item->update_transform);
	assert_int_equal(signals, 2);
	obs_sceneitem_force_update_transform(f.item);
	assert_int_equal(width_reads, 2);
	assert_int_equal(height_reads, 2);
	assert_int_equal(signals, 2);
	free_fixture(&f);
}

static int setup(void **state)
{
	(void)state;
	base_set_log_handler(quiet_log, NULL);
	if (!obs_startup("en-US", NULL, NULL))
		return -1;
	struct obs_source_info info = {
		.id = "force_update_size_source",
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = source_name,
		.create = source_create,
		.destroy = source_destroy,
		.get_width = source_width,
		.get_height = source_height,
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
		cmocka_unit_test(null_is_silent),
		cmocka_unit_test(dirty_then_clean),
		cmocka_unit_test(deferred_consumes_flag),
		cmocka_unit_test(group_child_computes_immediately),
		cmocka_unit_test(parentless_clean_and_deferred),
		cmocka_unit_test(signal_can_redirty),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
