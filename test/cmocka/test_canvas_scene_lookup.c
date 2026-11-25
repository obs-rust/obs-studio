/* Direct public-API characterization of obs_canvas_get_scene_by_name before
 * extraction and a later Rust swap. No video device is needed. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "cmocka-compat.h"
#include <obs.h>

static void missing_and_canvas_scope(void **state)
{
	(void)state;
	obs_canvas_t *left = obs_canvas_create_private("lookup left", NULL, 0);
	obs_canvas_t *right = obs_canvas_create_private("lookup right", NULL, 0);
	assert_non_null(left);
	assert_non_null(right);
	assert_null(obs_canvas_get_scene_by_name(left, "absent"));
	assert_null(obs_canvas_get_scene_by_name(left, ""));

	obs_scene_t *scene = obs_canvas_scene_create(left, "lookup scene");
	assert_non_null(scene);
	assert_null(obs_canvas_get_scene_by_name(right, "lookup scene"));
	assert_null(obs_canvas_get_scene_by_name(left, "Lookup scene"));
	obs_scene_t *found = obs_canvas_get_scene_by_name(left, "lookup scene");
	assert_ptr_equal(found, scene);
	obs_scene_release(found);
	obs_canvas_scene_remove(scene);
	assert_null(obs_canvas_get_scene_by_name(left, "lookup scene"));
	obs_scene_release(scene);
	obs_wait_for_destroy_queue();
	obs_canvas_release(right);
	obs_canvas_release(left);
}

static void same_name_in_distinct_canvases(void **state)
{
	(void)state;
	obs_canvas_t *left = obs_canvas_create_private("same left", NULL, 0);
	obs_canvas_t *right = obs_canvas_create_private("same right", NULL, 0);
	assert_non_null(left);
	assert_non_null(right);
	obs_scene_t *a = obs_canvas_scene_create(left, "same scene");
	obs_scene_t *b = obs_canvas_scene_create(right, "same scene");
	assert_non_null(a);
	assert_non_null(b);
	obs_scene_t *found_a = obs_canvas_get_scene_by_name(left, "same scene");
	obs_scene_t *found_b = obs_canvas_get_scene_by_name(right, "same scene");
	assert_ptr_equal(found_a, a);
	assert_ptr_equal(found_b, b);
	assert_ptr_not_equal(found_a, found_b);
	obs_scene_release(found_a);
	obs_scene_release(found_b);
	obs_canvas_scene_remove(a);
	obs_canvas_scene_remove(b);
	obs_scene_release(a);
	obs_scene_release(b);
	obs_wait_for_destroy_queue();
	obs_canvas_release(right);
	obs_canvas_release(left);
}

static void successful_lookup_retains_exactly_one_reference(void **state)
{
	(void)state;
	obs_canvas_t *canvas = obs_canvas_create_private("retained canvas", NULL, 0);
	assert_non_null(canvas);
	obs_scene_t *scene = obs_canvas_scene_create(canvas, "retained scene");
	assert_non_null(scene);
	obs_weak_source_t *weak = obs_source_get_weak_source(obs_scene_get_source(scene));
	assert_non_null(weak);
	obs_scene_t *found = obs_canvas_get_scene_by_name(canvas, "retained scene");
	assert_ptr_equal(found, scene);
	obs_canvas_scene_remove(scene);
	obs_scene_release(scene);
	obs_wait_for_destroy_queue();
	assert_false(obs_weak_source_expired(weak));
	assert_string_equal(obs_source_get_name(obs_scene_get_source(found)), "retained scene");
	obs_scene_release(found);
	obs_wait_for_destroy_queue();
	assert_true(obs_weak_source_expired(weak));
	obs_weak_source_release(weak);
	obs_canvas_release(canvas);
}

static void rejected_group_releases_lookup_reference(void **state)
{
	(void)state;
	obs_canvas_t *canvas = obs_canvas_create_private("group canvas", NULL, 0);
	assert_non_null(canvas);
	obs_scene_t *scene = obs_canvas_scene_create(canvas, "group parent");
	assert_non_null(scene);
	obs_sceneitem_t *item = obs_scene_add_group(scene, "lookup group");
	assert_non_null(item);
	obs_source_t *group = obs_sceneitem_get_source(item);
	obs_weak_source_t *weak = obs_source_get_weak_source(group);
	assert_non_null(weak);
	/* A group is present in the canvas source table but is not a scene. */
	obs_source_t *source = obs_canvas_get_source_by_name(canvas, "lookup group");
	assert_ptr_equal(source, group);
	assert_null(obs_scene_from_source(source));
	obs_source_release(source);
	for (size_t i = 0; i < 3; i++)
		assert_null(obs_canvas_get_scene_by_name(canvas, "lookup group"));
	assert_false(obs_weak_source_expired(weak));
	obs_sceneitem_remove(item);
	obs_canvas_scene_remove(scene);
	obs_scene_release(scene);
	obs_wait_for_destroy_queue();
	/* Any leaked temporary lookup reference would keep the group alive. */
	assert_true(obs_weak_source_expired(weak));
	obs_weak_source_release(weak);
	obs_canvas_release(canvas);
}

static int setup(void **state)
{
	(void)state;
	return obs_startup("en-US", NULL, NULL) ? 0 : -1;
}

static int teardown(void **state)
{
	(void)state;
	obs_wait_for_destroy_queue();
	obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(missing_and_canvas_scope),
		cmocka_unit_test(same_name_in_distinct_canvases),
		cmocka_unit_test(successful_lookup_retains_exactly_one_reference),
		cmocka_unit_test(rejected_group_releases_lookup_reference),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
