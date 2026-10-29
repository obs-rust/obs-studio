#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-output-begin.h"

/*
 * Characterization of the begin-capture plan of libobs/obs-output.c, moved
 * to libobs/obs-output-begin.c before its Rust port (#2927):
 * - obs_output_begin_video_interval() and obs_output_begin_audio_interval():
 *   step 1 of calculate_batch_size(), each encoder's packet interval in ns;
 * - obs_output_begin_batch_size(): the largest interval and step 2, the
 *   interleaver's maximum batch size;
 * - obs_output_begin_pair_video() and obs_output_begin_pair_audio(): the
 *   gates of pair_encoders().
 * They pin the current C behaviour; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source into
 * itself and calls only them, through their header. obs-output.c keeps the
 * encoder getters, the walk over the slots, the locks and the weak
 * references. test_output_start_steps, test_output_interleave,
 * test_output_interleave_e2e and test_output_interleave_restart cover the
 * callers.
 *
 * Not covered, undefined in C: an fps_num or sample_rate of 0 and an interval
 * of 0 (division by 0), and a quotient above 64 bits on MSVC x64.
 */

#define NTSC_DEN 1001 /* fps_den of 29.97 fps (fps_num 30000) */

/* ------------------------------------------------------------------------ */
/* helpers */

static void check_u64(const char *what, uint64_t got, uint64_t want)
{
	if (got != want) {
		fprintf(stderr, "%s: %llu; expected %llu\n", what, (unsigned long long)got, (unsigned long long)want);
		fail();
	}
}

static void check_batch(const char *what, const uint64_t *intervals, size_t num, size_t want)
{
	size_t got = obs_output_begin_batch_size(intervals, num);
	if (got != want) {
		fprintf(stderr, "%s: %zu; expected %zu\n", what, got, want);
		fail();
	}
}

/* ------------------------------------------------------------------------ */
/* step 1: the intervals */

static void video_interval_common_rates(void **state)
{
	(void)state;
	check_u64("30 fps", obs_output_begin_video_interval(30, 1, 1), 33333333);
	check_u64("29.97 fps", obs_output_begin_video_interval(30000, NTSC_DEN, 1), 33366666);
	check_u64("25 fps", obs_output_begin_video_interval(25, 1, 1), 40000000);
	check_u64("60 fps", obs_output_begin_video_interval(60, 1, 1), 16666666);
	check_u64("59.94 fps", obs_output_begin_video_interval(60000, NTSC_DEN, 1), 16683333);
	check_u64("24 fps", obs_output_begin_video_interval(24, 1, 1), 41666666);
}

static void video_interval_with_divisor(void **state)
{
	(void)state;
	check_u64("60 fps / 2", obs_output_begin_video_interval(60, 1, 2), 33333333);
	check_u64("60 fps / 3", obs_output_begin_video_interval(60, 1, 3), 50000000);
	check_u64("30 fps / 2", obs_output_begin_video_interval(30, 1, 2), 66666666);
	check_u64("59.94 fps / 2", obs_output_begin_video_interval(60000, NTSC_DEN, 2), 33366666);
}

static void video_interval_at_the_limits(void **state)
{
	(void)state;
	/* characterized, not endorsed: fps_den * divisor wraps as uint32_t, to
	 * 0 here, so the interval is 0 (and step 2 would divide by it) */
	check_u64("65536 * 65536 wraps to 0", obs_output_begin_video_interval(30, 65536, 65536), 0);
	check_u64("0x80000000 * 2 wraps to 0", obs_output_begin_video_interval(30, 0x80000000u, 2), 0);
	check_u64("0x80000001 * 2 wraps to 2", obs_output_begin_video_interval(30, 0x80000001u, 2), 66666666);
	check_u64("the largest product", obs_output_begin_video_interval(1, UINT32_MAX, 1), 4294967295000000000ULL);
	check_u64("the largest fps_num", obs_output_begin_video_interval(UINT32_MAX, 1, 1), 0);
	check_u64("fps_num of a billion", obs_output_begin_video_interval(1000000000u, 1, 1), 1);
}

static void audio_interval_common_rates(void **state)
{
	(void)state;
	check_u64("AAC 48 kHz", obs_output_begin_audio_interval(48000, 1024), 21333333);
	check_u64("AAC 44.1 kHz", obs_output_begin_audio_interval(44100, 1024), 23219954);
	check_u64("Opus 48 kHz", obs_output_begin_audio_interval(48000, 960), 20000000);
	check_u64("AAC 96 kHz", obs_output_begin_audio_interval(96000, 1024), 10666666);
	check_u64("one sample at 48 kHz", obs_output_begin_audio_interval(48000, 1), 20833);
}

static void audio_interval_at_the_limits(void **state)
{
	(void)state;
	/* A frame size of 0 gives an interval of 0, which step 2 would divide
	 * by (characterized, not endorsed). */
	check_u64("frame size 0", obs_output_begin_audio_interval(48000, 0), 0);
	check_u64("2^33 samples at 1 Hz", obs_output_begin_audio_interval(1, (size_t)1 << 33), 8589934592000000000ULL);
	check_u64("the largest sample rate", obs_output_begin_audio_interval(UINT32_MAX, 1024), 238);
}

/* ------------------------------------------------------------------------ */
/* step 2: the batch size */

static void batch_of_one_encoder(void **state)
{
	(void)state;
	const uint64_t video[] = {33333333};
	check_batch("video only", video, 1, 2);
	const uint64_t audio[] = {21333333};
	check_batch("audio only", audio, 1, 2);
}

static void batch_of_video_and_audio(void **state)
{
	(void)state;
	/* The comment's own example: 33.3 ms video and 21.3 ms AAC */
	const uint64_t stream[] = {33333333, 21333333};
	check_batch("30 fps + AAC", stream, 2, 2 + 3);
	const uint64_t audio_first[] = {21333333, 33333333};
	check_batch("the order does not matter", audio_first, 2, 3 + 2);
	const uint64_t ntsc[] = {33366666, 23219954};
	check_batch("29.97 fps + AAC 44.1 kHz", ntsc, 2, 2 + 2);
	const uint64_t slow[] = {16666666, 21333333};
	check_batch("60 fps + AAC (audio is the largest)", slow, 2, 2 + 2);
}

static void batch_of_a_multitrack_set(void **state)
{
	(void)state;
	/* 60 fps, 60 fps / 2, three AAC tracks at 48 kHz */
	const uint64_t set[] = {16666666, 33333333, 21333333, 21333333, 21333333};
	check_batch("multitrack", set, 5, 4 + 2 + 3 * 3);
	const uint64_t equal[] = {33333333, 33333333, 33333333, 33333333};
	check_batch("four equal intervals", equal, 4, 4 * 2);
}

static void batch_of_no_encoder(void **state)
{
	(void)state;
	check_batch("none", NULL, 0, 0);
}

static void batch_at_the_limits(void **state)
{
	(void)state;
	const uint64_t far[] = {1000000000, 1};
	check_batch("one second and one ns", far, 2, 2 + 2000000000);
	/* characterized, not endorsed: largest * 2 wraps, to 0 here, so every
	 * encoder adds 0 */
	const uint64_t wrap[] = {0x8000000000000000ULL, 1};
	check_batch("largest * 2 wraps to 0", wrap, 2, 0);
	const uint64_t below[] = {0x7fffffffffffffffULL, 0x7fffffffffffffffULL};
	check_batch("largest * 2 just fits", below, 2, 4);
}

/* ------------------------------------------------------------------------ */
/* pair_encoders() */

static void pair_gates(void **state)
{
	(void)state;
	assert_true(obs_output_begin_pair_video(false));
	assert_false(obs_output_begin_pair_video(true));

	assert_true(obs_output_begin_pair_audio(false, 0));
	assert_false(obs_output_begin_pair_audio(true, 0));
	assert_false(obs_output_begin_pair_audio(false, 1));
	assert_false(obs_output_begin_pair_audio(true, 1));
	assert_false(obs_output_begin_pair_audio(false, 2));
	assert_false(obs_output_begin_pair_audio(false, SIZE_MAX));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(video_interval_common_rates),
		cmocka_unit_test(video_interval_with_divisor),
		cmocka_unit_test(video_interval_at_the_limits),
		cmocka_unit_test(audio_interval_common_rates),
		cmocka_unit_test(audio_interval_at_the_limits),
		cmocka_unit_test(batch_of_one_encoder),
		cmocka_unit_test(batch_of_video_and_audio),
		cmocka_unit_test(batch_of_a_multitrack_set),
		cmocka_unit_test(batch_of_no_encoder),
		cmocka_unit_test(batch_at_the_limits),
		cmocka_unit_test(pair_gates),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
