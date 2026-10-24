#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdbool.h>
#include <cmocka.h>

#include "dshow-rotation.h"

/* A Video Capture Device source with Autorotation on turns the picture by
 * the device's roll. Turning Autorotation off must turn it back, and turning
 * it on again must re-apply the roll (obs-rust/obs-studio#2137). */

static void test_frame_applies_roll_once(void **state)
{
	(void)state;
	long last = 0;

	assert_false(dshow_rotation_on_frame(true, &last, 0));
	assert_true(dshow_rotation_on_frame(true, &last, 90));
	assert_int_equal(last, 90);
	assert_false(dshow_rotation_on_frame(true, &last, 90));
	assert_true(dshow_rotation_on_frame(true, &last, 180));
	assert_int_equal(last, 180);
}

static void test_frame_ignores_roll_when_off(void **state)
{
	(void)state;
	long last = 0;

	assert_false(dshow_rotation_on_frame(false, &last, 90));
	assert_int_equal(last, 0);
}

static void test_off_resets_applied_rotation(void **state)
{
	(void)state;
	long last = 0;

	assert_true(dshow_rotation_on_frame(true, &last, 90));
	assert_true(dshow_rotation_on_settings(false, &last));
	assert_int_equal(last, 0);
	/* Already back at 0: nothing more to undo. */
	assert_false(dshow_rotation_on_settings(false, &last));
	assert_false(dshow_rotation_on_frame(false, &last, 90));
}

static void test_on_again_reapplies_same_roll(void **state)
{
	(void)state;
	long last = 0;

	assert_true(dshow_rotation_on_frame(true, &last, 270));
	assert_true(dshow_rotation_on_settings(false, &last));
	assert_false(dshow_rotation_on_settings(true, &last));
	assert_true(dshow_rotation_on_frame(true, &last, 270));
	assert_int_equal(last, 270);
}

static void test_settings_leave_rotation_alone(void **state)
{
	(void)state;
	long last = 0;

	/* Off with nothing applied. */
	assert_false(dshow_rotation_on_settings(false, &last));
	assert_int_equal(last, 0);

	/* On: an update (e.g. a resolution change) keeps the applied roll. */
	assert_true(dshow_rotation_on_frame(true, &last, 90));
	assert_false(dshow_rotation_on_settings(true, &last));
	assert_int_equal(last, 90);
	assert_false(dshow_rotation_on_frame(true, &last, 90));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_frame_applies_roll_once),
		cmocka_unit_test(test_frame_ignores_roll_when_off),
		cmocka_unit_test(test_off_resets_applied_rotation),
		cmocka_unit_test(test_on_again_reapplies_same_roll),
		cmocka_unit_test(test_settings_leave_rotation_alone),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
