#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-output-reconnect.h"

/*
 * Characterization of the reconnect policy of libobs/obs-output.c, moved to
 * libobs/obs-output-reconnect.c before its Rust port (#2990):
 * - output_reconnect(): obs_output_reconnect_first_delay(), the first
 *   attempt's wait; obs_output_reconnect_give_up(), the end of the retries;
 *   obs_output_reconnect_grows() and obs_output_reconnect_next_delay(), a
 *   later attempt's wait and its 15-minute clamp;
 * - can_reconnect(): obs_output_reconnect_active(), whether reconnecting is
 *   on (and the reconnect callback asked), and obs_output_reconnect_can(),
 *   whether a stop reconnects.
 * They pin the current C behaviour; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source into
 * itself and calls only them, through their header. obs-output.c keeps the
 * reconnect thread, the stop event, the callback, the atomics and the
 * capture's start and end.
 *
 * Not covered: a delay setting whose * 1000 overflows int (undefined in C), a
 * negative delay setting, a first delay above 15 minutes, a later delay whose
 * product exceeds UINT32_MAX (undefined in C) and a negative retry count. The
 * Rust port changes those on purpose; its parity test lists them.
 */

#define DEFAULT_RETRY_SEC 2
#define DEFAULT_RETRY_MAX 20

static const int all_codes[] = {
	OBS_OUTPUT_RECONNECT_SUCCESS,
	-1, /* OBS_OUTPUT_BAD_PATH */
	-2, /* OBS_OUTPUT_CONNECT_FAILED */
	-3, /* OBS_OUTPUT_INVALID_STREAM */
	-4, /* OBS_OUTPUT_ERROR */
	OBS_OUTPUT_RECONNECT_DISCONNECTED,
	-6, /* OBS_OUTPUT_UNSUPPORTED */
	-7, /* OBS_OUTPUT_NO_SPACE */
	-8, /* OBS_OUTPUT_ENCODE_ERROR */
	-9, /* OBS_OUTPUT_HDR_DISABLED */
	1,
	-100,
};

#define NUM_CODES (sizeof(all_codes) / sizeof(all_codes[0]))

/* ------------------------------------------------------------------------ */
/* helpers */

/* The waits output_reconnect() sets for attempts 1 to n of one reconnect,
 * as it calls the policy: the first delay on the first attempt, grown on
 * each later one. */
static void walk(int retry_sec, float exp, uint32_t *delays, int n)
{
	uint32_t cur = 0;
	for (int retries = 0; retries < n; retries++) {
		if (retries == 0)
			cur = obs_output_reconnect_first_delay(retry_sec);
		if (obs_output_reconnect_grows(retries))
			cur = obs_output_reconnect_next_delay(cur, exp);
		delays[retries] = cur;
	}
}

static void check_walk(const char *what, int retry_sec, float exp, const uint32_t *want, int n)
{
	uint32_t got[DEFAULT_RETRY_MAX];
	walk(retry_sec, exp, got, n);
	for (int i = 0; i < n; i++) {
		if (got[i] != want[i]) {
			fprintf(stderr, "%s: attempt %d waits %u ms; expected %u\n", what, i + 1, got[i], want[i]);
			fail();
		}
	}
}

static void check_u32(const char *what, uint32_t got, uint32_t want)
{
	if (got != want) {
		fprintf(stderr, "%s: %u; expected %u\n", what, got, want);
		fail();
	}
}

/* ------------------------------------------------------------------------ */
/* the delays */

static void first_delay_from_the_setting(void **state)
{
	(void)state;
	check_u32("0 s", obs_output_reconnect_first_delay(0), 0);
	check_u32("1 s", obs_output_reconnect_first_delay(1), 1000);
	check_u32("2 s (default)", obs_output_reconnect_first_delay(DEFAULT_RETRY_SEC), 2000);
	check_u32("10 s", obs_output_reconnect_first_delay(10), 10000);
	check_u32("60 s", obs_output_reconnect_first_delay(60), 60000);
	check_u32("900 s, the limit", obs_output_reconnect_first_delay(900), 900000);
}

static void default_delays_grow_to_the_limit(void **state)
{
	(void)state;
	/* The exponent is 1.5 plus up to 0.05 of jitter, drawn at create. */
	static const uint32_t at_1_5[DEFAULT_RETRY_MAX] = {
		2000,   3000,   4500,   6750,   10125,  15187,  22780,  34170,  51255,  76882,
		115323, 172984, 259476, 389214, 583821, 875731, 900000, 900000, 900000, 900000,
	};
	static const uint32_t at_1_55[DEFAULT_RETRY_MAX] = {
		2000,   3100,   4805,   7447,   11542,  17890,  27729,  42979,  66617,  103256,
		160046, 248071, 384510, 595990, 900000, 900000, 900000, 900000, 900000, 900000,
	};
	check_walk("2 s at 1.5", DEFAULT_RETRY_SEC, 1.5f, at_1_5, DEFAULT_RETRY_MAX);
	check_walk("2 s at 1.55", DEFAULT_RETRY_SEC, 1.55f, at_1_55, DEFAULT_RETRY_MAX);
}

static void other_settings_and_exponents(void **state)
{
	(void)state;
	static const uint32_t ten_at_1_5[DEFAULT_RETRY_MAX] = {
		10000,  15000,  22500,  33750,  50625,  75937,  113905, 170857, 256285, 384427,
		576640, 864960, 900000, 900000, 900000, 900000, 900000, 900000, 900000, 900000,
	};
	static const uint32_t one_at_1_55[DEFAULT_RETRY_MAX] = {
		1000,  1550,   2402,   3723,   5770,   8943,   13861,  21484,  33300,  51615,
		80003, 124004, 192206, 297919, 461774, 715749, 900000, 900000, 900000, 900000,
	};
	static const uint32_t two_at_2[DEFAULT_RETRY_MAX] = {
		2000,   4000,   8000,   16000,  32000,  64000,  128000, 256000, 512000, 900000,
		900000, 900000, 900000, 900000, 900000, 900000, 900000, 900000, 900000, 900000,
	};
	static const uint32_t two_at_1[DEFAULT_RETRY_MAX] = {
		2000, 2000, 2000, 2000, 2000, 2000, 2000, 2000, 2000, 2000,
		2000, 2000, 2000, 2000, 2000, 2000, 2000, 2000, 2000, 2000,
	};
	check_walk("10 s at 1.5", 10, 1.5f, ten_at_1_5, DEFAULT_RETRY_MAX);
	check_walk("1 s at 1.55", 1, 1.55f, one_at_1_55, DEFAULT_RETRY_MAX);
	check_walk("2 s at 2", DEFAULT_RETRY_SEC, 2.0f, two_at_2, DEFAULT_RETRY_MAX);
	check_walk("2 s at 1", DEFAULT_RETRY_SEC, 1.0f, two_at_1, DEFAULT_RETRY_MAX);
}

static void a_delay_of_zero_stays_zero(void **state)
{
	(void)state;
	/* Characterized, not endorsed: every attempt starts at once. */
	static const uint32_t zeros[DEFAULT_RETRY_MAX] = {0};
	check_walk("0 s at 1.5", 0, 1.5f, zeros, DEFAULT_RETRY_MAX);
	check_walk("0 s at 1.55", 0, 1.55f, zeros, DEFAULT_RETRY_MAX);
}

static void next_delay_at_the_limit(void **state)
{
	(void)state;
	check_u32("599999 * 1.5", obs_output_reconnect_next_delay(599999, 1.5f), 899998);
	check_u32("600000 * 1.5", obs_output_reconnect_next_delay(600000, 1.5f), OBS_OUTPUT_RECONNECT_MAX_MSEC);
	check_u32("600001 * 1.5", obs_output_reconnect_next_delay(600001, 1.5f), OBS_OUTPUT_RECONNECT_MAX_MSEC);
	check_u32("the limit * 1.5", obs_output_reconnect_next_delay(OBS_OUTPUT_RECONNECT_MAX_MSEC, 1.5f),
		  OBS_OUTPUT_RECONNECT_MAX_MSEC);
	check_u32("2000000 * 1.5", obs_output_reconnect_next_delay(2000000, 1.5f), OBS_OUTPUT_RECONNECT_MAX_MSEC);
	check_u32("the limit * 1", obs_output_reconnect_next_delay(OBS_OUTPUT_RECONNECT_MAX_MSEC, 1.0f),
		  OBS_OUTPUT_RECONNECT_MAX_MSEC);
	/* Truncated, not rounded. */
	check_u32("1 * 1.5", obs_output_reconnect_next_delay(1, 1.5f), 1);
	check_u32("3 * 1.5", obs_output_reconnect_next_delay(3, 1.5f), 4);
	check_u32("0 * 1.55", obs_output_reconnect_next_delay(0, 1.55f), 0);
}

static void only_later_attempts_grow(void **state)
{
	(void)state;
	assert_false(obs_output_reconnect_grows(0));
	assert_true(obs_output_reconnect_grows(1));
	assert_true(obs_output_reconnect_grows(2));
	assert_true(obs_output_reconnect_grows(DEFAULT_RETRY_MAX - 1));
	assert_true(obs_output_reconnect_grows(DEFAULT_RETRY_MAX));
}

/* ------------------------------------------------------------------------ */
/* the retries */

static void gives_up_after_the_count(void **state)
{
	(void)state;
	for (int retries = 0; retries < DEFAULT_RETRY_MAX; retries++)
		assert_false(obs_output_reconnect_give_up(retries, DEFAULT_RETRY_MAX));
	assert_true(obs_output_reconnect_give_up(DEFAULT_RETRY_MAX, DEFAULT_RETRY_MAX));
	assert_true(obs_output_reconnect_give_up(DEFAULT_RETRY_MAX + 1, DEFAULT_RETRY_MAX));

	assert_false(obs_output_reconnect_give_up(0, 1));
	assert_true(obs_output_reconnect_give_up(1, 1));

	/* A count lowered to 0 while reconnecting: the next attempt gives up. */
	assert_true(obs_output_reconnect_give_up(0, 0));
	assert_true(obs_output_reconnect_give_up(5, 0));
}

/* ------------------------------------------------------------------------ */
/* the route */

static void reconnecting_is_on_for_a_count(void **state)
{
	(void)state;
	/* With a count of 0, the callback is not asked and a disconnect
	 * stops the output. */
	assert_false(obs_output_reconnect_active(0));
	assert_true(obs_output_reconnect_active(1));
	assert_true(obs_output_reconnect_active(DEFAULT_RETRY_MAX));
	assert_true(obs_output_reconnect_active(10000));
}

static void an_active_output_reconnects_only_on_a_disconnect(void **state)
{
	(void)state;
	for (size_t i = 0; i < NUM_CODES; i++) {
		int code = all_codes[i];
		bool want = code == OBS_OUTPUT_RECONNECT_DISCONNECTED;
		if (obs_output_reconnect_can(false, true, code) != want) {
			fprintf(stderr, "active, code %d: expected %d\n", code, want);
			fail();
		}
		/* Reconnecting off: never. */
		if (obs_output_reconnect_can(false, false, code)) {
			fprintf(stderr, "off, code %d: reconnects\n", code);
			fail();
		}
	}
}

static void a_failed_attempt_retries_on_any_error(void **state)
{
	(void)state;
	for (size_t i = 0; i < NUM_CODES; i++) {
		int code = all_codes[i];
		bool want = code != OBS_OUTPUT_RECONNECT_SUCCESS;
		/* Whether reconnecting is still on does not matter: a count
		 * lowered to 0 while reconnecting retries too, and gives up. */
		if (obs_output_reconnect_can(true, true, code) != want ||
		    obs_output_reconnect_can(true, false, code) != want) {
			fprintf(stderr, "reconnecting, code %d: expected %d\n", code, want);
			fail();
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(first_delay_from_the_setting),
		cmocka_unit_test(default_delays_grow_to_the_limit),
		cmocka_unit_test(other_settings_and_exponents),
		cmocka_unit_test(a_delay_of_zero_stays_zero),
		cmocka_unit_test(next_delay_at_the_limit),
		cmocka_unit_test(only_later_attempts_grow),
		cmocka_unit_test(gives_up_after_the_count),
		cmocka_unit_test(reconnecting_is_on_for_a_count),
		cmocka_unit_test(an_active_output_reconnects_only_on_a_disconnect),
		cmocka_unit_test(a_failed_attempt_retries_on_any_error),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
