#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <limits.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-video-pacing.h"

/*
 * Characterization of the frame pacing decisions of video_sleep() in
 * libobs/obs-video.c, moved to libobs/obs-video-pacing.c before their Rust
 * port (#2738):
 * - obs_video_pacing_step(): the new frame time and the frame count, after a
 *   sleep that reached the next frame time or after a late wake-up;
 * - obs_video_pacing_group_starts(): whether a ready encoder's group gets its
 *   start timestamp.
 * They pin the current C behaviour; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source into
 * itself and calls only them, through their header. video_sleep() keeps the
 * sleep, the clock read, the stats, the encoder groups' locks and the deque
 * pushes. It stores the new frame time as a group's start timestamp, while
 * the frame info gets the old one; that stays in obs-video.c. The real-libobs
 * tests (test_video_output_formats, test_raw_output_video_reset,
 * test_encoding_lag_rescale, test_encoder_group_null,
 * test_encoder_lifecycle_steps, test_encoder_audio_start_steps,
 * test_media_restart_stale_frame) cover the caller.
 *
 * Nothing here comes from outside OBS: the frame time and the interval come
 * from libobs' video settings and its clock, the group counts from its own
 * encoders. libobs never passes an interval of 0 (the step divides by it).
 */

#define FPS30 33333333ULL /* 1e9 * 1001 / 30000 rounded, a common interval */
#define FPS25 40000000ULL
#define T0 1000000000000ULL /* any frame time well away from 0 */

/* ------------------------------------------------------------------------ */
/* helpers */

static void check_step(const char *what, uint64_t frame_time, uint64_t interval, bool slept, uint64_t now,
		       uint64_t expected_time, int expected_count)
{
	struct obs_video_pacing_step got = obs_video_pacing_step(frame_time, interval, slept, now);
	if (got.frame_time != expected_time || got.count != expected_count) {
		fprintf(stderr, "%s: frame time %llu count %d; expected %llu %d\n", what,
			(unsigned long long)got.frame_time, got.count, (unsigned long long)expected_time,
			expected_count);
		fail();
	}
}

/* ------------------------------------------------------------------------ */
/* the frame time step */

/* The sleep reached the next frame time: one frame, no lag, the next frame
 * time. now is not used then. */
static void slept_is_one_frame(void **state)
{
	(void)state;
	check_step("30 fps", T0, FPS30, true, 0, T0 + FPS30, 1);
	check_step("25 fps", T0, FPS25, true, 0, T0 + FPS25, 1);
	check_step("now ignored (late)", T0, FPS25, true, T0 + 10 * FPS25, T0 + FPS25, 1);
	check_step("now ignored (earlier)", T0, FPS25, true, T0 - 5, T0 + FPS25, 1);
	check_step("frame time 0", 0, FPS25, true, 0, FPS25, 1);
	check_step("interval 1 ns", T0, 1, true, 0, T0 + 1, 1);
}

/* A late wake-up: the whole intervals since the frame time, at least one,
 * and the frame time that many intervals on (on the interval grid, not the
 * wake-up time). */
static void late_wakeup_counts_whole_intervals(void **state)
{
	(void)state;
	check_step("late, less than 2 intervals", T0, FPS25, false, T0 + FPS25 + 1, T0 + FPS25, 1);
	check_step("late, exactly 1 interval", T0, FPS25, false, T0 + FPS25, T0 + FPS25, 1);
	check_step("late, just under 2", T0, FPS25, false, T0 + 2 * FPS25 - 1, T0 + FPS25, 1);
	check_step("late, exactly 2", T0, FPS25, false, T0 + 2 * FPS25, T0 + 2 * FPS25, 2);
	check_step("late, 2.5 intervals", T0, FPS25, false, T0 + 5 * FPS25 / 2, T0 + 2 * FPS25, 2);
	check_step("late, 10.9 intervals", T0, FPS30, false, T0 + 10 * FPS30 + 9 * FPS30 / 10, T0 + 10 * FPS30, 10);
	check_step("late, 1 s at 25 fps", T0, FPS25, false, T0 + 1000000000ULL, T0 + 25 * FPS25, 25);
	check_step("interval 1 ns, 7 ns late", T0, 1, false, T0 + 7, T0 + 7, 7);
}

/* Not slept, but the clock reads less than one interval on, or earlier than
 * the frame time (a negative difference): one interval. */
static void short_or_negative_difference_is_one_interval(void **state)
{
	(void)state;
	check_step("same time", T0, FPS25, false, T0, T0 + FPS25, 1);
	check_step("half an interval", T0, FPS25, false, T0 + FPS25 / 2, T0 + FPS25, 1);
	check_step("1 ns earlier", T0, FPS25, false, T0 - 1, T0 + FPS25, 1);
	check_step("1 s earlier", T0, FPS25, false, T0 - 1000000000ULL, T0 + FPS25, 1);
	/* now - frame_time wraps to at least 2^63: negative as int64, so one
	 * interval. */
	check_step("now 0", T0, FPS25, false, 0, T0 + FPS25, 1);
	check_step("difference 2^63", 0, FPS25, false, 1ULL << 63, FPS25, 1);
}

/* Large jumps (suspend and resume, a clock step) near the limits of the int
 * count. Characterized, not endorsed: a count above INT_MAX is converted to
 * int as the compilers libobs supports do (modulo 2^32), and the frame time
 * is then the frame time plus the interval times that negative count,
 * modulo 2^64. With a 25 fps interval INT_MAX frames are about 2.7 years. */
static void large_jumps_wrap_the_count(void **state)
{
	(void)state;
	const uint64_t max = (uint64_t)INT_MAX;
	check_step("INT_MAX intervals", 0, FPS25, false, max * FPS25, max * FPS25, INT_MAX);
	check_step("INT_MAX intervals and a bit", 0, FPS25, false, max * FPS25 + FPS25 / 2, max * FPS25, INT_MAX);
	check_step("INT_MAX + 1 intervals", 0, FPS25, false, (max + 1) * FPS25, (uint64_t)INT_MIN * FPS25, INT_MIN);
	check_step("2^32 intervals", 0, 1, false, 1ULL << 32, 0, 0);
	check_step("2^32 + 3 intervals", 0, 1, false, (1ULL << 32) + 3, 3, 3);
}

/* ------------------------------------------------------------------------ */
/* the encoder group start */

/* Every encoder started and no start timestamp yet: the group starts. A
 * group whose start timestamp is set keeps it. Counts above the number of
 * encoders and an empty group are characterized, not endorsed. */
static void group_starts_once_all_encoders_started(void **state)
{
	(void)state;
	assert_true(obs_video_pacing_group_starts(2, 2, 0));
	assert_true(obs_video_pacing_group_starts(1, 1, 0));
	assert_false(obs_video_pacing_group_starts(1, 2, 0));
	assert_false(obs_video_pacing_group_starts(0, 2, 0));
	assert_false(obs_video_pacing_group_starts(2, 2, T0));
	assert_false(obs_video_pacing_group_starts(2, 2, 1));
	assert_false(obs_video_pacing_group_starts(1, 2, T0));
	assert_true(obs_video_pacing_group_starts(3, 2, 0));
	assert_true(obs_video_pacing_group_starts(0, 0, 0));
	assert_true(obs_video_pacing_group_starts(UINT32_MAX, UINT32_MAX, 0));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(slept_is_one_frame),
		cmocka_unit_test(late_wakeup_counts_whole_intervals),
		cmocka_unit_test(short_or_negative_difference_is_one_interval),
		cmocka_unit_test(large_jumps_wrap_the_count),
		cmocka_unit_test(group_starts_once_all_encoders_started),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
