#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <setjmp.h>
#include <cmocka.h>

#include <limits.h>
#include <math.h>

#include "obs-internal.h"
#include "obs-scene.h"

/*
 * The scene item crop and size arithmetic in obs-scene.h, which obs-scene.c
 * uses, on its own and built with UBSan (-fno-sanitize-recover): values that
 * come from a scene collection out of range must give the nearest value in
 * range, and nothing may overflow:
 * - obs_scene_crop_from_int64(): a crop read from a scene collection;
 * - obs_scene_cropped_size(): the size of a cropped source (calc_cx and
 *   calc_cy), whose int sum of four crops overflowed;
 * - obs_scene_scale_crop(): a nested scene's crop scaled when the scene
 *   changes size, a float to int cast;
 * - obs_scene_crop_offset(): where the crop moves the source when it is
 *   drawn, an int sum;
 * - obs_scene_size_from_extent(): a group's size from its content, a float
 *   to uint32_t cast.
 * Every value in range gives the result it gave before.
 * test_scene_item_crop_load covers the callers through the real libobs.
 */

static void crop_from_int64(void **state)
{
	(void)state;

	assert_int_equal(obs_scene_crop_from_int64(0), 0);
	assert_int_equal(obs_scene_crop_from_int64(123), 123);
	/* Negative crops are kept here; obs_sceneitem_set_crop() clamps them. */
	assert_int_equal(obs_scene_crop_from_int64(-5), -5);
	assert_int_equal(obs_scene_crop_from_int64(INT_MAX), INT_MAX);
	assert_int_equal(obs_scene_crop_from_int64(INT_MIN), INT_MIN);

	/* Past the int range: the nearest int, not the other sign. */
	assert_int_equal(obs_scene_crop_from_int64(3000000000LL), INT_MAX);
	assert_int_equal(obs_scene_crop_from_int64(-3000000000LL), INT_MIN);
	assert_int_equal(obs_scene_crop_from_int64(INT64_MAX), INT_MAX);
	assert_int_equal(obs_scene_crop_from_int64(INT64_MIN), INT_MIN);
}

static void cropped_size(void **state)
{
	(void)state;

	assert_int_equal(obs_scene_cropped_size(200, 0, 0, 0, 0), 200);
	assert_int_equal(obs_scene_cropped_size(200, 10, 30, 0, 0), 160);
	assert_int_equal(obs_scene_cropped_size(200, 10, 30, 5, 5), 150);
	/* Cropped exactly away: 0, as before. */
	assert_int_equal(obs_scene_cropped_size(200, 100, 100, 0, 0), 0);
	/* Cropped by more than the source: 2. */
	assert_int_equal(obs_scene_cropped_size(200, 150, 51, 0, 0), 2);
	assert_int_equal(obs_scene_cropped_size(0, 1, 0, 0, 0), 2);
	assert_int_equal(obs_scene_cropped_size(UINT32_MAX, 0, 0, 0, 0), UINT32_MAX);
	assert_int_equal(obs_scene_cropped_size(UINT32_MAX, INT_MAX, 0, 0, 0), (uint32_t)INT_MAX + 1);

	/* The sum is past the int range (it overflowed). */
	assert_int_equal(obs_scene_cropped_size(UINT32_MAX, INT_MAX, INT_MAX, 0, 0), 1);
	assert_int_equal(obs_scene_cropped_size(UINT32_MAX, INT_MAX, INT_MAX, INT_MAX, INT_MAX), 2);
	assert_int_equal(obs_scene_cropped_size(200, INT_MAX, INT_MAX, INT_MAX, INT_MAX), 2);

	/* A negative crop (which only a scene collection could give before
	 * it was clamped) crops it all away. */
	assert_int_equal(obs_scene_cropped_size(200, -100, 0, 0, 0), 2);
	assert_int_equal(obs_scene_cropped_size(200, INT_MIN, INT_MIN, 0, 0), 2);
}

static void scale_crop(void **state)
{
	(void)state;

	assert_int_equal(obs_scene_scale_crop(0, 4000.0f), 0);
	assert_int_equal(obs_scene_scale_crop(100, 2.0f), 200);
	assert_int_equal(obs_scene_scale_crop(100, 0.5f), 50);
	/* Truncated toward 0, as before. */
	assert_int_equal(obs_scene_scale_crop(10, 0.33f), 3);
	assert_int_equal(obs_scene_scale_crop(-10, 0.33f), -3);
	assert_int_equal(obs_scene_scale_crop(1000000000, 2.0f), 2000000000);

	/* Past the int range: the nearest int. */
	assert_int_equal(obs_scene_scale_crop(1000000000, 4000.0f), INT_MAX);
	assert_int_equal(obs_scene_scale_crop(INT_MAX, 1.0f), INT_MAX);
	assert_int_equal(obs_scene_scale_crop(-1000000000, 4000.0f), INT_MIN);
	assert_int_equal(obs_scene_scale_crop(INT_MIN, 1.0f), INT_MIN);
	assert_int_equal(obs_scene_scale_crop(1, INFINITY), INT_MAX);
	assert_int_equal(obs_scene_scale_crop(5, NAN), 0);
}

static void crop_offset(void **state)
{
	(void)state;

	assert_true(obs_scene_crop_offset(0, 0) == 0.0f);
	assert_true(obs_scene_crop_offset(10, 5) == 15.0f);
	assert_true(obs_scene_crop_offset(INT_MAX, 0) == (float)INT_MAX);
	/* The sum is past the int range (it overflowed). */
	assert_true(obs_scene_crop_offset(INT_MAX, INT_MAX) == 4294967294.0f);
}

static void size_from_extent(void **state)
{
	(void)state;

	assert_int_equal(obs_scene_size_from_extent(0.0f), 0);
	assert_int_equal(obs_scene_size_from_extent(1280.0f), 1280);
	/* Rounded up, as before. */
	assert_int_equal(obs_scene_size_from_extent(199.2f), 200);
	assert_int_equal(obs_scene_size_from_extent(-0.5f), 0);
	assert_int_equal(obs_scene_size_from_extent(4294967040.0f), 4294967040u);

	/* Past the uint32_t range: the nearest value; NaN is no size. */
	assert_int_equal(obs_scene_size_from_extent(4294967296.0f), UINT32_MAX);
	assert_int_equal(obs_scene_size_from_extent(1e20f), UINT32_MAX);
	assert_int_equal(obs_scene_size_from_extent(INFINITY), UINT32_MAX);
	assert_int_equal(obs_scene_size_from_extent(-1e20f), 0);
	assert_int_equal(obs_scene_size_from_extent(-INFINITY), 0);
	assert_int_equal(obs_scene_size_from_extent(NAN), 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(crop_from_int64), cmocka_unit_test(cropped_size),     cmocka_unit_test(scale_crop),
		cmocka_unit_test(crop_offset),     cmocka_unit_test(size_from_extent),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
