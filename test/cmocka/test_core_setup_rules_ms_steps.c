#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-core-setup-rules.h"

/* Characterization of the audio buffering in milliseconds, moved from
 * obs_reset_audio2() and obs_get_audio_info2() in libobs/obs.c to
 * libobs/obs-core-setup-rules-ms.c: ticks * 1024 * 1000 / (int)rate, in
 * int, rounded toward zero. The test calls only the moved function, which
 * is hidden in libobs, so this characterizes that C source. It pins the
 * CURRENT behaviour; cases marked "characterized, not endorsed" are odd but
 * real.
 *
 * A zero rate and 2098 ticks or more are left out: the divide traps and the
 * product overflows an int, and UBSan stops on both. */

/* The expression as obs.c wrote it before the move. */
static int as_written(int ticks, uint32_t samples_per_sec)
{
	return ticks * 1024 * 1000 / (int)samples_per_sec;
}

static void test_constants(void **state)
{
	(void)state;
	assert_int_equal(OBS_CORE_SETUP_RULES_AUDIO_OUTPUT_FRAMES, 1024);
	assert_int_equal(OBS_CORE_SETUP_RULES_SEC_TO_MSEC, 1000);
}

static void test_buffer_ms(void **state)
{
	(void)state;
	/* The default of 45 ticks. */
	assert_int_equal(obs_core_setup_rules_buffer_ms(45, 44100), 1044);
	assert_int_equal(obs_core_setup_rules_buffer_ms(45, 48000), 960);
	assert_int_equal(obs_core_setup_rules_buffer_ms(45, 96000), 480);
	assert_int_equal(obs_core_setup_rules_buffer_ms(0, 48000), 0);
	assert_int_equal(obs_core_setup_rules_buffer_ms(1, 1), 1024000);
	assert_int_equal(obs_core_setup_rules_buffer_ms(1, 48000), 21);
	/* 1000 ms at 48 kHz: 47 ticks, read back as 1002 ms. */
	assert_int_equal(obs_core_setup_rules_buffer_ms(47, 48000), 1002);
	/* The most ticks whose product fits an int. */
	assert_int_equal(obs_core_setup_rules_buffer_ms(2097, 48000), 44736);
	assert_int_equal(obs_core_setup_rules_buffer_ms(2097, 1), 2147328000);
}

/* Characterized, not endorsed: the rate is cast to int, so one above
 * INT_MAX divides by a negative number, and the log shows a negative
 * buffering (or 0). */
static void test_rate_past_int(void **state)
{
	(void)state;
	assert_int_equal(obs_core_setup_rules_buffer_ms(45, UINT32_MAX), -46080000);
	assert_int_equal(obs_core_setup_rules_buffer_ms(45, 0xfffffc00u), -45000);
	assert_int_equal(obs_core_setup_rules_buffer_ms(45, 0x80000000u), 0);
}

/* Every tick from -2097 to 2097 at the usual rates and the edges of the
 * cast gives what the expression in obs.c gave. */
static void test_every_tick(void **state)
{
	(void)state;
	static const uint32_t rates[] = {1,      7,           8000,        22050,       44100,       48000,     96000,
					 192000, 0x7fffffffu, 0x80000000u, 0x80000001u, 0xfffffc00u, UINT32_MAX};
	for (size_t r = 0; r < sizeof(rates) / sizeof(rates[0]); r++) {
		for (int ticks = -2097; ticks <= 2097; ticks++) {
			int got = obs_core_setup_rules_buffer_ms(ticks, rates[r]);
			int want = as_written(ticks, rates[r]);
			if (got != want) {
				fprintf(stderr, "ticks %d rate %u: %d, want %d\n", ticks, rates[r], got, want);
				fail();
			}
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_constants),
		cmocka_unit_test(test_buffer_ms),
		cmocka_unit_test(test_rate_past_int),
		cmocka_unit_test(test_every_tick),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
