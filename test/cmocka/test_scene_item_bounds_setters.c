#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <limits.h>
#include <cmocka.h>
#include "util/threading.h"
#include "obs-scene.h"

/* Actual-library characterization; stack fixtures own no sources or buffers.
 * Group/orphan updates are deferred, so no graphics context is needed. */
static void bounds_writes(void **state)
{
	(void)state;
	obs_sceneitem_set_bounds_type(NULL, (enum obs_bounds_type)INT_MAX);
	obs_sceneitem_set_bounds_alignment(NULL, UINT32_MAX);
	obs_sceneitem_set_bounds_crop(NULL, true);
	obs_sceneitem_set_bounds(NULL, NULL);

	struct obs_scene scene = {0};
	scene.is_group = true;
	assert_int_equal(pthread_mutex_init_recursive(&scene.video_mutex), 0);
	for (unsigned parented = 0; parented < 2; parented++) {
		struct obs_scene_item item = {0};
		item.parent = parented ? &scene : NULL;
		item.absolute_coordinates = true;
		for (int type = OBS_BOUNDS_NONE; type <= OBS_BOUNDS_MAX_ONLY; type++) {
			item.update_transform = false;
			obs_sceneitem_set_bounds_type(&item, (enum obs_bounds_type)type);
			assert_int_equal(item.bounds_type, type);
			assert_true(item.update_transform);
		}
		const int invalid[] = {-1, OBS_BOUNDS_MAX_ONLY + 1, INT_MAX};
		for (size_t i = 0; i < sizeof(invalid) / sizeof(invalid[0]); i++) {
			item.bounds_type = OBS_BOUNDS_STRETCH;
			item.update_transform = false;
			obs_sceneitem_set_bounds_type(&item, (enum obs_bounds_type)invalid[i]);
			assert_int_equal(item.bounds_type, OBS_BOUNDS_NONE);
			assert_true(item.update_transform);
		}
		const uint32_t alignments[] = {0, OBS_ALIGN_LEFT | OBS_ALIGN_TOP, UINT32_MAX};
		for (size_t i = 0; i < sizeof(alignments) / sizeof(alignments[0]); i++) {
			item.update_transform = false;
			obs_sceneitem_set_bounds_alignment(&item, alignments[i]);
			/* Unknown alignment bits are characterized, not endorsed. */
			assert_int_equal(item.bounds_align, alignments[i]);
			assert_true(item.update_transform);
		}
		for (unsigned crop = 0; crop < 2; crop++) {
			item.update_transform = false;
			obs_sceneitem_set_bounds_crop(&item, crop != 0);
			assert_int_equal(item.crop_to_bounds, crop != 0);
			assert_true(item.update_transform);
		}
		const struct vec2 sizes[] = {{0.0f, 0.0f}, {1920.0f, 1080.0f}, {-20.0f, 35.5f}};
		for (size_t i = 0; i < sizeof(sizes) / sizeof(sizes[0]); i++) {
			item.update_transform = false;
			obs_sceneitem_set_bounds(&item, &sizes[i]);
			assert_memory_equal(&item.bounds, &sizes[i], sizeof(struct vec2));
			assert_true(item.update_transform);
		}
	}
	assert_int_equal(pthread_mutex_destroy(&scene.video_mutex), 0);
}

static void relative_bounds(void **state)
{
	(void)state;
	struct obs_scene scene = {0};
	scene.custom_size = true;
	scene.cx = 1920;
	scene.cy = 1080;
	assert_int_equal(pthread_mutex_init_recursive(&scene.video_mutex), 0);
	struct obs_scene_item item = {0};
	item.parent = &scene;
	/* A normal scene requests an immediate update; the existing defer counter
	 * suppresses it without changing the bounds write or coordinate conversion. */
	item.defer_update = 1;
	const struct vec2 input = {540.0f, -270.0f};
	obs_sceneitem_set_bounds(&item, &input);
	assert_float_equal(item.bounds.x, 1.0f, 0.000001f);
	assert_float_equal(item.bounds.y, -0.5f, 0.000001f);
	assert_false(item.update_transform);
	assert_int_equal(item.defer_update, 1);
	assert_float_equal(input.x, 540.0f, 0.0f);
	assert_float_equal(input.y, -270.0f, 0.0f);
	assert_int_equal(pthread_mutex_destroy(&scene.video_mutex), 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(bounds_writes), cmocka_unit_test(relative_bounds)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
