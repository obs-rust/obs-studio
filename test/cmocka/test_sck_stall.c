#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdint.h>
#include <util/c99defs.h>

#include "mac-sck-stall.h"

/* A ScreenCaptureKit source that stops publishing frames keeps showing the
 * last frame, and the recording keeps running, so the only way a user learns
 * about it is the log. The decision - is the gap long enough to report, and
 * which side of the gap is at fault - is plain C here, so the Linux cmocka
 * suite runs exactly what the macOS build runs: no ObjC, no SCK types. */

#define MS 1000000ULL

static void test_no_warning_before_threshold(void **state)
{
	UNUSED_PARAMETER(state);
	const uint64_t now = 600 * 1000 * MS;
	assert_false(sck_stall_should_warn(now, now - 1000 * MS, 0, SCK_STALL_WARN_MS, SCK_STALL_REWARN_MS));
}

static void test_warning_after_threshold(void **state)
{
	UNUSED_PARAMETER(state);
	const uint64_t now = 600 * 1000 * MS;
	assert_true(sck_stall_should_warn(now, now - 2500 * MS, 0, SCK_STALL_WARN_MS, SCK_STALL_REWARN_MS));
}

static void test_warning_repeats_after_rewarn(void **state)
{
	UNUSED_PARAMETER(state);
	const uint64_t now = 600 * 1000 * MS;
	const uint64_t delivered = now - 5000 * MS;
	assert_false(sck_stall_should_warn(now, delivered, now - 1000 * MS, SCK_STALL_WARN_MS, SCK_STALL_REWARN_MS));
	assert_true(sck_stall_should_warn(now, delivered, now - 31000 * MS, SCK_STALL_WARN_MS, SCK_STALL_REWARN_MS));
}

static void test_never_delivered_is_no_frames(void **state)
{
	UNUSED_PARAMETER(state);
	const uint64_t now = 600 * 1000 * MS;
	assert_int_equal(sck_stall_classify(now, 0, 0), SCK_STALL_NO_FRAMES);
	assert_int_equal(sck_stall_classify(now, 0, now - 1000 * MS), SCK_STALL_NO_FRAMES);
}

static void test_recent_delivery_is_no_stall(void **state)
{
	UNUSED_PARAMETER(state);
	const uint64_t now = 600 * 1000 * MS;
	assert_int_equal(sck_stall_classify(now, now - 100 * MS, 0), SCK_STALL_FRAMES_ARRIVED);
}

static void test_drop_after_delivery_is_dropped(void **state)
{
	UNUSED_PARAMETER(state);
	const uint64_t now = 600 * 1000 * MS;
	assert_int_equal(sck_stall_classify(now, now - 5000 * MS, now - 1000 * MS), SCK_STALL_FRAMES_DROPPED);
}

static void test_drop_before_delivery_is_no_frames(void **state)
{
	UNUSED_PARAMETER(state);
	const uint64_t now = 600 * 1000 * MS;
	assert_int_equal(sck_stall_classify(now, now - 1000 * MS, now - 5000 * MS), SCK_STALL_NO_FRAMES);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_no_warning_before_threshold),
		cmocka_unit_test(test_warning_after_threshold),
		cmocka_unit_test(test_warning_repeats_after_rewarn),
		cmocka_unit_test(test_never_delivered_is_no_frames),
		cmocka_unit_test(test_recent_delivery_is_no_stall),
		cmocka_unit_test(test_drop_after_delivery_is_dropped),
		cmocka_unit_test(test_drop_before_delivery_is_no_frames),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
