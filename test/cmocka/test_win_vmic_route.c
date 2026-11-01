#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <cmocka.h>

#include "vmic-route.h"

static const char *const names[] = {"Speakers", "VoiceMeeter Input", "CABLE Input"};
static const char *const ids[] = {"{spk}", "{vm}", "{cable}"};

static void saved_device_wins_when_present(void **state)
{
	(void)state;
	assert_string_equal(vmic_route_pick("{spk}", names, ids, 3), "{spk}");
}

static void cable_is_preselected_when_saved_is_gone(void **state)
{
	(void)state;
	assert_string_equal(vmic_route_pick("{unplugged}", names, ids, 3), "{cable}");
	assert_string_equal(vmic_route_pick(NULL, names, ids, 2), "{vm}");
}

static void no_cable_means_video_only(void **state)
{
	(void)state;
	assert_null(vmic_route_pick("", names, ids, 1));
}

static void packet_that_does_not_fit_is_dropped_whole(void **state)
{
	(void)state;
	float buf[6];
	struct vmic_ring r = {buf, 6, 0, 0};
	const float a[4] = {1, 2, 3, 4};
	const float b[4] = {5, 6, 7, 8};
	float out[6] = {0};

	assert_true(vmic_ring_push(&r, a, 4));
	assert_false(vmic_ring_push(&r, b, 4));
	assert_int_equal(vmic_ring_pop(&r, out, 2), 2);
	assert_true(vmic_ring_push(&r, b, 4));
	assert_int_equal(vmic_ring_pop(&r, out, 6), 6);
	const float want[6] = {3, 4, 5, 6, 7, 8};
	assert_memory_equal(out, want, sizeof(want));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(saved_device_wins_when_present),
		cmocka_unit_test(cable_is_preselected_when_saved_is_gone),
		cmocka_unit_test(no_cable_means_video_only),
		cmocka_unit_test(packet_that_does_not_fit_is_dropped_whole),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
