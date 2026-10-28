#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdbool.h>
#include <util/c99defs.h>

#include "display-reinit.h"

/* A display capture that loses its display — unplugged, replugged into
 * another port, or reconfigured to a different resolution — keeps showing the
 * last frame while the UI still says it is recording: nothing re-creates the
 * CGDisplayStream. The decision of what to do with the saved `display_uuid`
 * after a configuration change is plain C here, so the Linux cmocka suite runs
 * exactly what the macOS build runs: no ObjC, no CoreGraphics types
 * (obs-rust/obs-studio#2838). */

static const char *const SINGLE[] = {"8A1F4C32-0000-4000-8000-00000000A001"};
static const char *const OTHER[] = {"8A1F4C32-0000-4000-8000-00000000A002"};
static const char *const DUPLICATE[] = {"8A1F4C32-0000-4000-8000-00000000A001", "8A1F4C32-0000-4000-8000-00000000A001"};

static void test_present_and_running_keeps_the_stream(void **state)
{
	UNUSED_PARAMETER(state);
	assert_int_equal(display_reinit_decide(SINGLE[0], SINGLE, 1, true), DISPLAY_REINIT_KEEP);
}

static void test_present_after_the_stream_stopped_rebuilds(void **state)
{
	UNUSED_PARAMETER(state);
	assert_int_equal(display_reinit_decide(SINGLE[0], SINGLE, 1, false), DISPLAY_REINIT_REBUILD);
}

static void test_absent_display_waits(void **state)
{
	UNUSED_PARAMETER(state);
	assert_int_equal(display_reinit_decide(SINGLE[0], OTHER, 1, false), DISPLAY_REINIT_WAIT);
	assert_int_equal(display_reinit_decide(SINGLE[0], OTHER, 1, true), DISPLAY_REINIT_WAIT);
}

static void test_no_enumerated_display_waits(void **state)
{
	UNUSED_PARAMETER(state);
	assert_int_equal(display_reinit_decide(SINGLE[0], SINGLE, 0, false), DISPLAY_REINIT_WAIT);
	assert_int_equal(display_reinit_decide(SINGLE[0], NULL, 0, false), DISPLAY_REINIT_WAIT);
}

static void test_no_saved_uuid_waits(void **state)
{
	UNUSED_PARAMETER(state);
	assert_int_equal(display_reinit_decide(NULL, SINGLE, 1, false), DISPLAY_REINIT_WAIT);
	assert_int_equal(display_reinit_decide("", SINGLE, 1, false), DISPLAY_REINIT_WAIT);
}

static void test_ambiguous_identity_waits(void **state)
{
	UNUSED_PARAMETER(state);
	assert_int_equal(display_reinit_decide(SINGLE[0], DUPLICATE, 2, false), DISPLAY_REINIT_WAIT);
}

static void test_saved_uuid_is_not_swapped(void **state)
{
	UNUSED_PARAMETER(state);
	/* One other display is present and the saved one is gone: the capture
	 * waits for its own display rather than showing a different one. */
	assert_int_equal(display_reinit_decide(SINGLE[0], OTHER, 1, true), DISPLAY_REINIT_WAIT);

	const char *const two[] = {SINGLE[0], OTHER[0]};
	assert_int_equal(display_reinit_decide(SINGLE[0], two, 2, true), DISPLAY_REINIT_KEEP);
}

static void test_action_names(void **state)
{
	UNUSED_PARAMETER(state);
	assert_string_equal(display_reinit_action_name(DISPLAY_REINIT_KEEP), "keep");
	assert_string_equal(display_reinit_action_name(DISPLAY_REINIT_WAIT), "wait");
	assert_string_equal(display_reinit_action_name(DISPLAY_REINIT_REBUILD), "rebuild");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_present_and_running_keeps_the_stream),
		cmocka_unit_test(test_present_after_the_stream_stopped_rebuilds),
		cmocka_unit_test(test_absent_display_waits),
		cmocka_unit_test(test_no_enumerated_display_waits),
		cmocka_unit_test(test_no_saved_uuid_waits),
		cmocka_unit_test(test_ambiguous_identity_waits),
		cmocka_unit_test(test_saved_uuid_is_not_swapped),
		cmocka_unit_test(test_action_names),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
