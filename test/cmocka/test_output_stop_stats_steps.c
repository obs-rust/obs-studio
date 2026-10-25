#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <limits.h>
#include <math.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-output-stop-stats.h"

/*
 * Characterization of the stop statistics of log_frame_info() in
 * libobs/obs-output.c, moved to libobs/obs-output-stop-stats.c before their
 * Rust port: the frames drawn and lagged since the output started, the two
 * percentages, and which of the lines after "stopping" are logged with which
 * numbers:
 * - "Total frames output: %d", or "%d (%d attempted)" with total - dropped
 *   and total;
 * - "Total drawn frames: %u", or "%u (%u attempted)" with drawn - lagged and
 *   drawn;
 * - "Number of lagged frames due to rendering lag/stalls: %u (%0.1f%%)";
 * - "Number of dropped frames due to insufficient bandwidth/connection
 *   stalls: %d (%0.1f%%)".
 * They pin the current C behaviour; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * The function is hidden in libobs, so this test compiles the C source into
 * itself and calls only it, through its header. log_frame_info() itself
 * (the reads, the plugin's get_dropped_frames() and the blog() calls) stays
 * in obs-output.c. The real-libobs tests that stop a video output
 * (test_output_interleave_e2e, test_raw_output_video_reset,
 * test_texture_encoder_raw_format and others; the tests.d file lists them)
 * run it, though none checks its lines.
 *
 * dropped comes from the output plugin (obs_output_get_frames_dropped()),
 * so it can be any int. total - dropped is int arithmetic: where it would
 * overflow (dropped INT_MIN with total 0 or more, for one) the C is
 * undefined, and this test, built with UBSan, does not call it there.
 */

struct counts {
	uint32_t total_frames;
	uint32_t lagged_frames;
	uint32_t starting_drawn;
	uint32_t starting_lagged;
	int dropped;
	int total;
};

static struct obs_output_stop_stats run(struct counts c)
{
	return obs_output_stop_stats(c.total_frames, c.lagged_frames, c.starting_drawn, c.starting_lagged, c.dropped,
				     c.total);
}

/* The bits of a double, so the percentages are compared exactly. */
static uint64_t bits(double d)
{
	uint64_t u;
	memcpy(&u, &d, sizeof(u));
	return u;
}

static void assert_pct(double got, double want)
{
	if (bits(got) != bits(want)) {
		fprintf(stderr, "percentage %a, want %a\n", got, want);
		fail();
	}
}

/* The text log_frame_info() prints for a percentage. */
static void assert_pct_text(double pct, const char *want)
{
	char buf[64];
	snprintf(buf, sizeof(buf), "%0.1f", pct);
	assert_string_equal(buf, want);
}

static void assert_plain(const struct obs_output_stop_stats *s)
{
	assert_false(s->output_attempted);
	assert_false(s->drawn_attempted);
	assert_false(s->lag_line);
	assert_false(s->drop_line);
}

/* ------------------------------------------------------------------------ */

/* Nothing drawn, lagged, output or dropped: the two plain lines with 0, no
 * lag or drop line, both percentages 0. */
static void zero_counters(void **state)
{
	(void)state;
	struct obs_output_stop_stats s = run((struct counts){0});
	assert_int_equal(s.drawn, 0);
	assert_int_equal(s.lagged, 0);
	assert_int_equal(s.frames_output, 0);
	assert_int_equal(s.frames_drawn, 0);
	assert_plain(&s);
	assert_pct(s.percentage_lagged, 0.0);
	assert_pct(s.percentage_dropped, 0.0);

	/* the same counters now and at the start */
	s = run((struct counts){5000, 7, 5000, 7, 0, 0});
	assert_int_equal(s.drawn, 0);
	assert_int_equal(s.lagged, 0);
	assert_plain(&s);
}

/* A recording that drew 900 frames since it started, 40 of them lagged,
 * and output 600 with 30 dropped. */
static void typical_stop(void **state)
{
	(void)state;
	struct obs_output_stop_stats s = run((struct counts){1000, 50, 100, 10, 30, 600});
	assert_int_equal(s.drawn, 900);
	assert_int_equal(s.lagged, 40);
	assert_true(s.output_attempted);
	assert_int_equal(s.frames_output, 570);
	assert_true(s.drawn_attempted);
	assert_int_equal(s.frames_drawn, 860);
	assert_true(s.lag_line);
	assert_true(s.drop_line);
	assert_pct(s.percentage_lagged, (double)40 / (double)900 * 100.0);
	assert_pct(s.percentage_dropped, (double)30 / (double)600 * 100.0);
	assert_pct_text(s.percentage_lagged, "4.4");
	assert_pct_text(s.percentage_dropped, "5.0");

	/* no lag and no drops: plain lines only */
	s = run((struct counts){1000, 50, 100, 50, 0, 900});
	assert_int_equal(s.drawn, 900);
	assert_int_equal(s.frames_drawn, 900);
	assert_int_equal(s.frames_output, 900);
	assert_plain(&s);
	assert_pct(s.percentage_lagged, 0.0);
	assert_pct(s.percentage_dropped, 0.0);
}

/* The video counters are uint32_t: a start count above the current one
 * (the counter wrapped while the output ran) gives the wrapped difference. */
static void counters_wrapped(void **state)
{
	(void)state;
	struct obs_output_stop_stats s = run((struct counts){5, 2, UINT32_MAX - 15, UINT32_MAX - 1, 0, 21});
	assert_int_equal(s.drawn, 21);
	assert_int_equal(s.lagged, 4);
	assert_true(s.drawn_attempted);
	assert_int_equal(s.frames_drawn, 17);
	assert_true(s.lag_line);
	assert_pct(s.percentage_lagged, (double)4 / (double)21 * 100.0);

	/* one past: the whole range */
	s = run((struct counts){0, 0, 1, 1, 0, 0});
	assert_int_equal(s.drawn, UINT32_MAX);
	assert_int_equal(s.lagged, UINT32_MAX);
	/* lagged equal to drawn: 0 drawn on time, 100% */
	assert_true(s.drawn_attempted);
	assert_int_equal(s.frames_drawn, 0);
	assert_pct(s.percentage_lagged, 100.0);
	assert_pct_text(s.percentage_lagged, "100.0");

	s = run((struct counts){UINT32_MAX, UINT32_MAX, 0, 0, 0, 0});
	assert_int_equal(s.drawn, UINT32_MAX);
	assert_int_equal(s.lagged, UINT32_MAX);
}

/* Lag needs something drawn: lagged frames with drawn 0 log the plain drawn
 * line, no lag line, and the percentage stays 0. */
static void lagged_without_drawn(void **state)
{
	(void)state;
	struct obs_output_stop_stats s = run((struct counts){100, 13, 100, 10, 0, 0});
	assert_int_equal(s.drawn, 0);
	assert_int_equal(s.lagged, 3);
	assert_false(s.drawn_attempted);
	assert_int_equal(s.frames_drawn, 0);
	assert_false(s.lag_line);
	assert_pct(s.percentage_lagged, 0.0);
}

/* More lagged than drawn (the lagged counter started later than the drawn
 * one): drawn - lagged wraps in uint32_t and the percentage is over 100
 * (characterized, not endorsed). */
static void lagged_above_drawn(void **state)
{
	(void)state;
	struct obs_output_stop_stats s = run((struct counts){10, 30, 0, 0, 0, 0});
	assert_int_equal(s.drawn, 10);
	assert_int_equal(s.lagged, 30);
	assert_true(s.drawn_attempted);
	assert_int_equal(s.frames_drawn, UINT32_MAX - 19);
	assert_true(s.lag_line);
	assert_pct(s.percentage_lagged, 300.0);
}

/* dropped from the plugin, against total. */
static void dropped_values(void **state)
{
	(void)state;
	struct obs_output_stop_stats s;

	/* none dropped: plain line with total */
	s = run((struct counts){0, 0, 0, 0, 0, 600});
	assert_false(s.output_attempted);
	assert_int_equal(s.frames_output, 600);
	assert_false(s.drop_line);
	assert_pct(s.percentage_dropped, 0.0);

	/* all dropped: 0 output, 100% */
	s = run((struct counts){0, 0, 0, 0, 600, 600});
	assert_true(s.output_attempted);
	assert_int_equal(s.frames_output, 0);
	assert_true(s.drop_line);
	assert_pct(s.percentage_dropped, 100.0);

	/* more dropped than attempted: a negative "Total frames output" and
	 * more than 100% (characterized, not endorsed) */
	s = run((struct counts){0, 0, 0, 0, 700, 600});
	assert_true(s.output_attempted);
	assert_int_equal(s.frames_output, -100);
	assert_true(s.drop_line);
	assert_pct(s.percentage_dropped, (double)700 / (double)600 * 100.0);
	assert_pct_text(s.percentage_dropped, "116.7");

	/* a negative count from the plugin: more output than attempted and a
	 * negative percentage (characterized, not endorsed) */
	s = run((struct counts){0, 0, 0, 0, -5, 100});
	assert_true(s.output_attempted);
	assert_int_equal(s.frames_output, 105);
	assert_true(s.drop_line);
	assert_pct(s.percentage_dropped, -5.0);
	assert_pct_text(s.percentage_dropped, "-5.0");
}

/* Drops with total 0: the plain line with 0 and no drop line, but the
 * percentage is still computed, a division by 0 (an infinity, never
 * logged). */
static void dropped_without_total(void **state)
{
	(void)state;
	struct obs_output_stop_stats s = run((struct counts){0, 0, 0, 0, 3, 0});
	assert_false(s.output_attempted);
	assert_int_equal(s.frames_output, 0);
	assert_false(s.drop_line);
	assert_true(isinf(s.percentage_dropped));
	assert_true(s.percentage_dropped > 0);

	s = run((struct counts){0, 0, 0, 0, -3, 0});
	assert_false(s.drop_line);
	assert_true(isinf(s.percentage_dropped));
	assert_true(s.percentage_dropped < 0);
}

/* The int extremes where total - dropped is defined. */
static void int_extremes(void **state)
{
	(void)state;
	struct obs_output_stop_stats s;

	s = run((struct counts){0, 0, 0, 0, INT_MAX, INT_MAX});
	assert_true(s.output_attempted);
	assert_int_equal(s.frames_output, 0);
	assert_pct(s.percentage_dropped, 100.0);

	s = run((struct counts){0, 0, 0, 0, INT_MAX, 1});
	assert_int_equal(s.frames_output, 1 - INT_MAX);
	assert_pct(s.percentage_dropped, (double)INT_MAX / (double)1 * 100.0);

	s = run((struct counts){0, 0, 0, 0, INT_MIN, -1});
	assert_int_equal(s.frames_output, INT_MAX);
	assert_pct(s.percentage_dropped, (double)INT_MIN / (double)-1 * 100.0);

	s = run((struct counts){0, 0, 0, 0, INT_MIN, INT_MIN});
	assert_int_equal(s.frames_output, 0);
	assert_pct(s.percentage_dropped, 100.0);

	s = run((struct counts){0, 0, 0, 0, -1, INT_MAX - 1});
	assert_int_equal(s.frames_output, INT_MAX);

	s = run((struct counts){0, 0, 0, 0, 1, INT_MIN + 1});
	assert_int_equal(s.frames_output, INT_MIN);
	assert_pct(s.percentage_dropped, (double)1 / (double)(INT_MIN + 1) * 100.0);

	/* INT_MIN dropped with total 0: the plain line, no subtraction */
	s = run((struct counts){0, 0, 0, 0, INT_MIN, 0});
	assert_false(s.output_attempted);
	assert_int_equal(s.frames_output, 0);
	assert_false(s.drop_line);
	assert_true(isinf(s.percentage_dropped));

	/* total INT_MIN (the output's counter wrapped) with none dropped */
	s = run((struct counts){0, 0, 0, 0, 0, INT_MIN});
	assert_false(s.output_attempted);
	assert_int_equal(s.frames_output, INT_MIN);
	assert_false(s.drop_line);
}

/* Each line's condition is independent of the others: the output lines
 * depend only on dropped and total, the drawn lines only on the video
 * counters. */
static void lines_independent(void **state)
{
	(void)state;
	const int ints[] = {INT_MIN, -2, -1, 0, 1, 2, INT_MAX};
	const uint32_t u32s[] = {0, 1, 2, UINT32_MAX - 1, UINT32_MAX};
	const size_t ni = sizeof(ints) / sizeof(ints[0]);
	const size_t nu = sizeof(u32s) / sizeof(u32s[0]);

	for (size_t d = 0; d < ni; d++) {
		for (size_t t = 0; t < ni; t++) {
			int dropped = ints[d];
			int total = ints[t];
			bool attempted = dropped && total;
			long long diff = (long long)total - dropped;
			if (attempted && (diff < INT_MIN || diff > INT_MAX))
				continue; /* undefined in the C */
			for (size_t a = 0; a < nu; a++) {
				for (size_t b = 0; b < nu; b++) {
					struct obs_output_stop_stats s =
						run((struct counts){u32s[a], u32s[b], 0, 0, dropped, total});
					uint32_t drawn = u32s[a];
					uint32_t lagged = u32s[b];
					assert_int_equal(s.drawn, drawn);
					assert_int_equal(s.lagged, lagged);
					assert_int_equal(s.output_attempted, attempted);
					assert_int_equal(s.drop_line, attempted);
					assert_int_equal(s.frames_output, attempted ? (int)diff : total);
					assert_int_equal(s.drawn_attempted, drawn && lagged);
					assert_int_equal(s.lag_line, drawn && lagged);
					assert_int_equal(s.frames_drawn, drawn && lagged ? drawn - lagged : drawn);
					assert_pct(s.percentage_lagged,
						   drawn ? (double)lagged / (double)drawn * 100.0 : 0.0);
					assert_pct(s.percentage_dropped,
						   dropped ? (double)dropped / (double)total * 100.0 : 0.0);
				}
			}
		}
	}
}

/* The percentages are double: (double)lagged / (double)drawn * 100.0,
 * divided first. Pinned bit for bit over small counts, where dividing
 * first and multiplying first differ (1/3 of 100 for one), and as
 * log_frame_info() prints them. */
static void percentages_exact(void **state)
{
	(void)state;
	unsigned differ = 0;
	for (uint32_t drawn = 1; drawn <= 200; drawn++) {
		for (uint32_t lagged = 0; lagged <= 200; lagged++) {
			struct obs_output_stop_stats s =
				run((struct counts){drawn, lagged, 0, 0, (int)lagged, (int)drawn});
			double want = (double)lagged / (double)drawn * 100.0;
			assert_pct(s.percentage_lagged, want);
			if (lagged)
				assert_pct(s.percentage_dropped, want);
			if (bits(want) != bits((double)lagged * 100.0 / (double)drawn))
				differ++;
		}
	}
	/* the order matters for some of them */
	assert_true(differ > 0);

	/* 100.0 / 3 would be 33.333333333333336 */
	struct obs_output_stop_stats s = run((struct counts){3, 1, 0, 0, 1, 3});
	assert_pct(s.percentage_lagged, 33.33333333333333);
	assert_pct_text(s.percentage_lagged, "33.3");
	s = run((struct counts){3, 2, 0, 0, 2, 3});
	assert_pct(s.percentage_lagged, 66.66666666666666);
	assert_pct_text(s.percentage_lagged, "66.7");
	s = run((struct counts){1000, 1, 0, 0, 1, 2000});
	assert_pct_text(s.percentage_lagged, "0.1");
	assert_pct_text(s.percentage_dropped, "0.1");
	s = run((struct counts){2000, 1, 0, 0, 1, 4000});
	assert_pct_text(s.percentage_lagged, "0.1");
	assert_pct(s.percentage_dropped, 0.025);
	assert_pct_text(s.percentage_dropped, "0.0");

	/* the largest counts are exact in a double */
	s = run((struct counts){UINT32_MAX, UINT32_MAX - 1, 0, 0, INT_MAX - 1, INT_MAX});
	assert_pct(s.percentage_lagged, (double)(UINT32_MAX - 1) / (double)UINT32_MAX * 100.0);
	assert_pct(s.percentage_dropped, (double)(INT_MAX - 1) / (double)INT_MAX * 100.0);
	assert_pct_text(s.percentage_lagged, "100.0");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(zero_counters),         cmocka_unit_test(typical_stop),
		cmocka_unit_test(counters_wrapped),      cmocka_unit_test(lagged_without_drawn),
		cmocka_unit_test(lagged_above_drawn),    cmocka_unit_test(dropped_values),
		cmocka_unit_test(dropped_without_total), cmocka_unit_test(int_extremes),
		cmocka_unit_test(lines_independent),     cmocka_unit_test(percentages_exact),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
