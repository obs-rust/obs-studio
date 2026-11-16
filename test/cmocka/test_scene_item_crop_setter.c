/* Characterization of obs_sceneitem_set_crop through actual libobs.
 * Private structs expose stored crop and scheduling without a render thread.
 * Repeated negative input is characterized, not endorsed (#1947).
 * No OBS-owned allocation or source is created by these field-only fixtures. */
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <cmocka.h>

#include <pthread.h>
#include <obs.h>
#include <obs-scene.h>
#include <util/base.h>
#include <util/threading.h>

static unsigned diagnostics;

static void count_diagnostic(int level, const char *format, va_list args, void *data)
{
	(void)level;
	(void)format;
	(void)args;
	(void)data;
	diagnostics++;
}

static void assert_crop(const struct obs_sceneitem_crop *actual, const struct obs_sceneitem_crop *expected)
{
	assert_int_equal(actual->left, expected->left);
	assert_int_equal(actual->right, expected->right);
	assert_int_equal(actual->top, expected->top);
	assert_int_equal(actual->bottom, expected->bottom);
}

static void positive_input_and_equal_noop(void **state)
{
	(void)state;
	struct obs_scene_item item = {0};
	const struct obs_sceneitem_crop crop = {.left = 11, .right = 22, .top = 33, .bottom = 44};
	obs_sceneitem_set_crop(&item, &crop);
	assert_crop(&item.crop, &crop);
	assert_true(os_atomic_load_bool(&item.update_transform));
	os_atomic_set_bool(&item.update_transform, false);
	obs_sceneitem_set_crop(&item, &crop);
	assert_crop(&item.crop, &crop);
	assert_false(os_atomic_load_bool(&item.update_transform));
	os_atomic_set_bool(&item.update_transform, true);
	obs_sceneitem_set_crop(&item, &crop);
	assert_true(os_atomic_load_bool(&item.update_transform));
}

static void raw_negative_input_marks_dirty_every_time(void **state)
{
	(void)state;
	const struct obs_sceneitem_crop inputs[] = {
		{.left = -1, .right = 2, .top = 3, .bottom = 4},
		{.left = 1, .right = -2, .top = 3, .bottom = 4},
		{.left = 1, .right = 2, .top = -3, .bottom = 4},
		{.left = 1, .right = 2, .top = 3, .bottom = -4},
		{.left = INT_MIN, .right = INT_MIN, .top = INT_MIN, .bottom = INT_MIN},
	};
	const struct obs_sceneitem_crop stored[] = {
		{.left = 0, .right = 2, .top = 3, .bottom = 4},
		{.left = 1, .right = 0, .top = 3, .bottom = 4},
		{.left = 1, .right = 2, .top = 0, .bottom = 4},
		{.left = 1, .right = 2, .top = 3, .bottom = 0},
		{0},
	};
	for (size_t i = 0; i < sizeof(inputs) / sizeof(inputs[0]); i++) {
		struct obs_scene_item item = {.crop = stored[i]};
		for (unsigned repeat = 0; repeat < 3; repeat++) {
			os_atomic_set_bool(&item.update_transform, false);
			obs_sceneitem_set_crop(&item, &inputs[i]);
			assert_crop(&item.crop, &stored[i]);
			assert_true(os_atomic_load_bool(&item.update_transform));
		}
		os_atomic_set_bool(&item.update_transform, false);
		obs_sceneitem_set_crop(&item, &stored[i]);
		assert_false(os_atomic_load_bool(&item.update_transform));
	}
}

static void null_arguments_preserve_fields(void **state)
{
	(void)state;
	struct obs_scene_item item = {.crop = {.left = 9, .right = 8, .top = 7, .bottom = 6}};
	const struct obs_sceneitem_crop original = item.crop;
	diagnostics = 0;
	base_set_log_handler(count_diagnostic, NULL);
	obs_sceneitem_set_crop(NULL, &original);
	obs_sceneitem_set_crop(NULL, NULL);
	obs_sceneitem_set_crop(&item, NULL);
	base_set_log_handler(NULL, NULL);
	assert_int_equal(diagnostics, 3);
	assert_crop(&item.crop, &original);
	assert_false(os_atomic_load_bool(&item.update_transform));
}

static void parent_lock_is_released_on_both_paths(void **state)
{
	(void)state;
	struct obs_scene scene = {0};
	struct obs_scene_item item = {.parent = &scene};
	const struct obs_sceneitem_crop crop = {.left = 1, .right = 2, .top = 3, .bottom = 4};
	assert_int_equal(pthread_mutex_init_recursive(&scene.video_mutex), 0);
	for (unsigned repeat = 0; repeat < 2; repeat++) {
		os_atomic_set_bool(&item.update_transform, false);
		obs_sceneitem_set_crop(&item, &crop);
		assert_crop(&item.crop, &crop);
		assert_int_equal(os_atomic_load_bool(&item.update_transform), repeat == 0);
		assert_int_equal(pthread_mutex_trylock(&scene.video_mutex), 0);
		assert_int_equal(pthread_mutex_unlock(&scene.video_mutex), 0);
	}
	assert_int_equal(pthread_mutex_destroy(&scene.video_mutex), 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(positive_input_and_equal_noop),
		cmocka_unit_test(raw_negative_input_marks_dirty_every_time),
		cmocka_unit_test(null_arguments_preserve_fields),
		cmocka_unit_test(parent_lock_is_released_on_both_paths),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
