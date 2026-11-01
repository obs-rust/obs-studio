#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <cmocka.h>

#include "vmic-clock.h"

/* 48 kHz with 1 GHz host ticks, period 512 frames. */
#define TPF (1e9 / 48000.0)

static void before_the_first_period_is_the_anchor(void **state)
{
	(void)state;
	struct vmic_zero_timestamp ts = vmic_clock_zero(1000, 1000 + 5000000, TPF, 512);
	assert_true(ts.sample_time == 0);
	assert_int_equal(ts.host_time, 1000);
	assert_int_equal(ts.seed, 1);
}

static void timestamps_land_on_whole_periods(void **state)
{
	(void)state;
	/* 3.5 periods in: the third boundary. */
	uint64_t now = 1000 + (uint64_t)(3.5 * 512 * TPF);
	struct vmic_zero_timestamp ts = vmic_clock_zero(1000, now, TPF, 512);
	assert_true(ts.sample_time == 3 * 512);
	assert_int_equal(ts.host_time, 1000 + (uint64_t)(3 * 512 * TPF));
}

static void host_time_before_anchor_is_the_anchor(void **state)
{
	(void)state;
	struct vmic_zero_timestamp ts = vmic_clock_zero(1000, 10, TPF, 512);
	assert_true(ts.sample_time == 0);
	assert_int_equal(ts.host_time, 1000);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(before_the_first_period_is_the_anchor),
		cmocka_unit_test(timestamps_land_on_whole_periods),
		cmocka_unit_test(host_time_before_anchor_is_the_anchor),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
