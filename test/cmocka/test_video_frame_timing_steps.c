#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-video-frame-timing.h"

/*
 * Characterization of the frame timing decisions of the graphics thread loop
 * in libobs/obs-video.c, moved to libobs/obs-video-frame-timing.c before their
 * Rust port (#2754):
 * - obs_video_frame_timing_tick(): tick_sources(), the time since the last
 *   tick (one interval on the first frame) and its seconds as a float;
 * - obs_video_frame_timing_stats(): the end of obs_graphics_thread_loop(),
 *   the sums, and once they cover at least one second the fps, the average
 *   render time and the reset.
 * They pin the current C behaviour; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source into
 * itself and calls only them, through their header. obs-video.c keeps the
 * clock reads, the tick callbacks, the source list, the profiler and
 * obs_video_stats_set_timing(). The real-libobs tests (test_source_video_tick_steps,
 * test_source_profiler, test_media_restart_stale_frame,
 * test_transition_recursive_destination, test_slideshow_empty,
 * test_slideshow_manual_end) cover the callers.
 *
 * Nothing here comes from outside OBS: the times come from libobs' clock, the
 * interval from its video settings. Not covered: a report with
 * fps_total_frames wrapped to 0 (2^32 frames without one second of frame
 * time), where the C divides by zero; unreachable.
 */

#define SEC 1000000000ULL
#define FPS25 40000000ULL
#define FPS30 33333333ULL
#define FPS60 16666667ULL
#define NTSC 33366700ULL /* 29.97 fps, 1e9 * 1001 / 30000 rounded */
#define T0 1000000000000ULL

/* ------------------------------------------------------------------------ */
/* helpers */

static void check_tick(const char *what, uint64_t cur, uint64_t last, uint64_t interval, uint64_t delta, float seconds)
{
	struct obs_video_frame_tick got = obs_video_frame_timing_tick(cur, last, interval);
	if (got.delta_ns != delta || memcmp(&got.seconds, &seconds, sizeof(float)) != 0) {
		fprintf(stderr, "%s: delta %llu seconds %.9g; expected %llu %.9g\n", what,
			(unsigned long long)got.delta_ns, (double)got.seconds, (unsigned long long)delta,
			(double)seconds);
		fail();
	}
}

static struct obs_video_frame_sums sums_of(uint64_t frame_time_total, uint64_t fps_total, uint32_t frames)
{
	struct obs_video_frame_sums sums = {frame_time_total, fps_total, frames};
	return sums;
}

static void check_sums(const char *what, struct obs_video_frame_sums got, uint64_t frame_time_total, uint64_t fps_total,
		       uint32_t frames)
{
	if (got.frame_time_total_ns != frame_time_total || got.fps_total_ns != fps_total ||
	    got.fps_total_frames != frames) {
		fprintf(stderr, "%s: sums %llu %llu %u; expected %llu %llu %u\n", what,
			(unsigned long long)got.frame_time_total_ns, (unsigned long long)got.fps_total_ns,
			got.fps_total_frames, (unsigned long long)frame_time_total, (unsigned long long)fps_total,
			frames);
		fail();
	}
}

/* ------------------------------------------------------------------------ */
/* the tick time */

/* The first frame (last tick time 0) gets one interval; later frames the
 * time since the last tick. */
static void tick_first_frame_is_one_interval(void **state)
{
	(void)state;
	check_tick("first frame, 25 fps", T0, 0, FPS25, FPS25, (float)((double)FPS25 / 1e9));
	check_tick("first frame, 60 fps", T0, 0, FPS60, FPS60, (float)((double)FPS60 / 1e9));
	check_tick("normal frame", T0 + FPS25, T0, FPS25, FPS25, (float)((double)FPS25 / 1e9));
	check_tick("a late frame, 3 intervals", T0 + 3 * FPS30, T0, FPS30, 3 * FPS30,
		   (float)((double)(3 * FPS30) / 1e9));
	check_tick("29.97 fps", T0 + NTSC, T0, NTSC, NTSC, (float)((double)NTSC / 1e9));
	check_tick("same time", T0, T0, FPS25, 0, 0.0f);
}

/* The seconds are the double quotient rounded to float: 25 fps gives
 * 0.04f exactly as a float literal; 30 and 29.97 fps round. */
static void tick_seconds_round_to_float(void **state)
{
	(void)state;
	struct obs_video_frame_tick t = obs_video_frame_timing_tick(T0 + FPS25, T0, FPS25);
	assert_true(t.seconds == 0.04f);
	t = obs_video_frame_timing_tick(T0 + FPS30, T0, FPS30);
	assert_true(t.seconds == (float)0.033333333);
	t = obs_video_frame_timing_tick(T0 + SEC, T0, FPS25);
	assert_true(t.seconds == 1.0f);
}

/* Characterized, not endorsed: a clock that goes backwards wraps the
 * uint64_t delta to nearly 2^64, about 1.8e10 seconds as a float; a first
 * frame at a time earlier than one interval wraps last_time. */
static void tick_backwards_clock_wraps(void **state)
{
	(void)state;
	check_tick("1 ns backwards", T0 - 1, T0, FPS25, UINT64_MAX, (float)((double)UINT64_MAX / 1e9));
	check_tick("1 s backwards", T0 - SEC, T0, FPS25, UINT64_MAX - SEC + 1,
		   (float)((double)(UINT64_MAX - SEC + 1) / 1e9));
	check_tick("first frame at time 0", 0, 0, FPS25, FPS25, (float)((double)FPS25 / 1e9));
	check_tick("first frame earlier than one interval", 5, 0, FPS25, FPS25, (float)((double)FPS25 / 1e9));
}

/* ------------------------------------------------------------------------ */
/* the frame statistics */

/* Below one second: the sums grow, nothing is reported. */
static void stats_accumulate_below_one_second(void **state)
{
	(void)state;
	struct obs_video_frame_stats s = obs_video_frame_timing_stats(sums_of(0, 0, 0), 5000000, T0 + FPS25, T0);
	assert_false(s.report);
	check_sums("first frame", s.sums, 5000000, FPS25, 1);

	s = obs_video_frame_timing_stats(sums_of(5000000, FPS25 * 23, 23), 4000000, T0 + FPS25, T0);
	assert_false(s.report);
	check_sums("frame 24 of 25", s.sums, 9000000, FPS25 * 24, 24);
	assert_true(s.fps == 0.0);
	assert_int_equal(s.avg_frame_time_ns, 0);
}

/* At exactly one second the window reports and resets: 25 frames of 40 ms
 * is 25 fps, the average render time the sum over 25. */
static void stats_report_at_one_second(void **state)
{
	(void)state;
	struct obs_video_frame_stats s =
		obs_video_frame_timing_stats(sums_of(24 * 2000000ULL, FPS25 * 24, 24), 2000000, T0 + FPS25, T0);
	assert_true(s.report);
	assert_true(s.fps == 25.0);
	assert_int_equal(s.avg_frame_time_ns, 2000000);
	check_sums("reset", s.sums, 0, 0, 0);

	/* 29.97 fps: 30 frames of 33366700 ns are 1.001001 s. */
	s = obs_video_frame_timing_stats(sums_of(30 * 1000000ULL, NTSC * 29, 29), 1000000, T0 + NTSC, T0);
	assert_true(s.report);
	assert_true(s.fps == 30.0 / ((double)(NTSC * 30) / 1e9));
	assert_int_equal(s.avg_frame_time_ns, (30 * 1000000ULL + 1000000) / 30);
}

/* A late frame that crosses the second: the window ends with a large step,
 * the fps is frames over the whole span, the average render time rounds
 * down. */
static void stats_late_frame_ends_the_window(void **state)
{
	(void)state;
	struct obs_video_frame_stats s =
		obs_video_frame_timing_stats(sums_of(1000, FPS60 * 50, 50), 7, T0 + 20 * FPS60, T0);
	assert_true(s.report);
	assert_true(s.fps == 51.0 / ((double)(FPS60 * 70) / 1e9));
	assert_int_equal(s.avg_frame_time_ns, 1007 / 51);
	check_sums("reset", s.sums, 0, 0, 0);
}

/* Characterized, not endorsed: a frame time earlier than the tick time
 * (a clock step) wraps the step to nearly 2^64, which reports at once with
 * an fps near 0; uint64_t and uint32_t sums wrap. */
static void stats_wrap_at_the_limits(void **state)
{
	(void)state;
	struct obs_video_frame_stats s = obs_video_frame_timing_stats(sums_of(0, 0, 0), 10, T0 - 1, T0);
	assert_true(s.report);
	assert_true(s.fps == 1.0 / ((double)UINT64_MAX / 1e9));
	assert_int_equal(s.avg_frame_time_ns, 10);

	s = obs_video_frame_timing_stats(sums_of(UINT64_MAX, 0, 3), 2, T0, T0);
	assert_false(s.report);
	check_sums("render time sum wraps", s.sums, 1, 0, 4);

	s = obs_video_frame_timing_stats(sums_of(0, 0, UINT32_MAX - 1), 0, T0, T0);
	assert_false(s.report);
	check_sums("frame count to UINT32_MAX", s.sums, 0, 0, UINT32_MAX);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(tick_first_frame_is_one_interval), cmocka_unit_test(tick_seconds_round_to_float),
		cmocka_unit_test(tick_backwards_clock_wraps),       cmocka_unit_test(stats_accumulate_below_one_second),
		cmocka_unit_test(stats_report_at_one_second),       cmocka_unit_test(stats_late_frame_ends_the_window),
		cmocka_unit_test(stats_wrap_at_the_limits),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
