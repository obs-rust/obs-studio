#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <limits.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-output-raw-audio.h"
#include "obs-output-raw-audio-trim.h"
#include "util/util_uint64.h"

/*
 * Characterization of the two sample-rate conversions of prepare_audio() in
 * libobs/obs-output.c, moved to libobs/obs-output-raw-audio-trim.c before
 * their Rust port. For a raw-audio packet that starts before the output's
 * first video frame:
 * - obs_output_raw_audio_trim_duration(): the packet's length,
 *   util_mul_div64(frames, 1000000000, sample_rate), which prepare_audio()
 *   passes to obs_output_raw_audio_ends_before() to drop a packet that ends
 *   at or before the video start (the drop cases run both, as it does);
 * - obs_output_raw_audio_trim_frames(): the cut in frames,
 *   util_mul_div64(cut_ns, sample_rate, 1000000000).
 * They pin the current C behaviour; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * Every case is also checked against the expressions obs-output.c had
 * before the move, written out here with the real util_mul_div64(). Rates
 * stay at or below UINT32_MAX, where util_mul_div64() gives the same result
 * on every platform for these inputs (the frames are a uint32_t and the cut
 * is shorter than the packet). A zero rate divides by zero in
 * _trim_duration(), so it is not called with one.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them, through their header.
 * test_output_raw_audio_steps (prepare_audio() copied onto a stand-in
 * output), test_output_before_video and test_raw_output_video_reset cover
 * the caller.
 *
 * Nothing here comes from outside OBS: timestamps come from the audio and
 * video threads, the frame count from the audio thread, the rate from
 * libobs's audio settings or an output's conversion.
 */

#define NS 1000000000ULL
/* one 1024-frame packet at 48 kHz: util_mul_div64(1024, NS, 48000) */
#define PKT 21333333ULL

static bool master_drops(uint64_t timestamp, uint32_t frames, uint64_t video_start_ts, size_t sample_rate)
{
	uint64_t duration = util_mul_div64(frames, 1000000000ULL, sample_rate);
	return obs_output_raw_audio_ends_before(timestamp, duration, video_start_ts);
}

static uint64_t master_frames(uint64_t cut_ns, size_t sample_rate)
{
	return util_mul_div64(cut_ns, sample_rate, 1000000000ULL);
}

static void check_drops(uint64_t timestamp, uint32_t frames, uint64_t video_start_ts, size_t sample_rate, bool expect)
{
	bool got = obs_output_raw_audio_ends_before(timestamp, obs_output_raw_audio_trim_duration(frames, sample_rate),
						    video_start_ts);
	if (got != expect || got != master_drops(timestamp, frames, video_start_ts, sample_rate)) {
		fprintf(stderr, "drops(%llu, %u, %llu, %llu) = %d, expected %d\n", (unsigned long long)timestamp,
			frames, (unsigned long long)video_start_ts, (unsigned long long)sample_rate, got, expect);
		fail();
	}
}

static void check_frames(uint64_t cut_ns, size_t sample_rate, uint64_t expect)
{
	uint64_t got = obs_output_raw_audio_trim_frames(cut_ns, sample_rate);
	if (got != expect || got != master_frames(cut_ns, sample_rate)) {
		fprintf(stderr, "frames(%llu, %llu) = %llu, expected %llu\n", (unsigned long long)cut_ns,
			(unsigned long long)sample_rate, (unsigned long long)got, (unsigned long long)expect);
		fail();
	}
}

/* ------------------------------------------------------------------------ */
/* obs_output_raw_audio_trim_duration(), through the drop */

static void drops_at_48k(void **state)
{
	(void)state;
	const uint64_t v = 5 * NS;

	/* ends before the video start, exactly at it (dropped), one ns into it */
	check_drops(v - PKT - 1, 1024, v, 48000, true);
	check_drops(v - PKT, 1024, v, 48000, true);
	check_drops(v - PKT + 1, 1024, v, 48000, false);
	/* starts a nanosecond before the video start */
	check_drops(v - 1, 1024, v, 48000, false);
	/* the length rounds down: 1 frame is 20833 ns */
	check_drops(v - 20833, 1, v, 48000, true);
	check_drops(v - 20832, 1, v, 48000, false);
}

static void drops_other_rates(void **state)
{
	(void)state;
	const uint64_t v = 5 * NS;

	/* 1024 frames: 23219954 ns at 44.1 kHz, 10666666 ns at 96 kHz */
	check_drops(v - 23219954, 1024, v, 44100, true);
	check_drops(v - 23219953, 1024, v, 44100, false);
	check_drops(v - 10666666, 1024, v, 96000, true);
	check_drops(v - 10666665, 1024, v, 96000, false);
	/* 1 Hz: a frame is a second */
	check_drops(v - NS, 1, v, 1, true);
	check_drops(v - NS + 1, 1, v, 1, false);
	/* over 1 GHz a frame lasts 0 ns, so one frame before video is dropped
	 * (characterized, not endorsed); 1024 frames at 2 GHz are 512 ns */
	check_drops(v - 1, 1, v, 2000000000, true);
	check_drops(v - 512, 1024, v, 2000000000, true);
	check_drops(v - 511, 1024, v, 2000000000, false);
	/* the highest rate: UINT32_MAX - 1 frames are 999999999 ns */
	check_drops(v - 999999999, UINT32_MAX - 1, v, UINT32_MAX, true);
	check_drops(v - 999999998, UINT32_MAX - 1, v, UINT32_MAX, false);
	check_drops(v - NS, UINT32_MAX, v, UINT32_MAX, true);
	check_drops(v - 1, UINT32_MAX, v, UINT32_MAX, false);
}

static void drops_edges(void **state)
{
	(void)state;

	/* no frames: 0 ns long, dropped (characterized, not endorsed) */
	check_drops(0, 0, 1, 48000, true);
	/* the most frames: 89478485312500 ns at 48 kHz */
	check_drops(0, UINT32_MAX, 89478485312500ULL, 48000, true);
	check_drops(0, UINT32_MAX, 89478485312499ULL, 48000, false);
	/* at the top of the clock: ending exactly at the video start is
	 * dropped, a nanosecond past it is kept */
	check_drops(UINT64_MAX - PKT, 1024, UINT64_MAX, 48000, true);
	check_drops(UINT64_MAX - PKT - 1, 1024, UINT64_MAX - 1, 48000, true);
	check_drops(UINT64_MAX - PKT - 1, 1024, UINT64_MAX - 2, 48000, false);
	/* the end wraps past UINT64_MAX, so it is dropped although it ends
	 * after the video start (characterized, not endorsed) */
	check_drops(UINT64_MAX - PKT + 1, 1024, UINT64_MAX, 48000, true);
	check_drops(UINT64_MAX - 1, 1024, UINT64_MAX, 48000, true);
}

/* ------------------------------------------------------------------------ */
/* obs_output_raw_audio_trim_frames() */

static void frames_at_48k(void **state)
{
	(void)state;

	check_frames(0, 48000, 0);
	/* less than one sample (20833.3 ns) cuts no frame */
	check_frames(1, 48000, 0);
	check_frames(20833, 48000, 0);
	check_frames(20834, 48000, 1);
	check_frames(PKT - 1, 48000, 1023);
	check_frames(PKT, 48000, 1023);
	check_frames(PKT + 1, 48000, 1024);
	check_frames(NS, 48000, 48000);
	check_frames(NS - 1, 48000, 47999);
}

static void frames_other_rates(void **state)
{
	(void)state;

	check_frames(22675, 44100, 0);
	check_frames(22676, 44100, 1);
	check_frames(NS, 44100, 44100);
	check_frames(10416, 96000, 0);
	check_frames(10417, 96000, 1);
	check_frames(NS - 1, 1, 0);
	check_frames(NS, 1, 1);
	check_frames(1, UINT32_MAX, 4);
	check_frames(NS, UINT32_MAX, UINT32_MAX);
	/* a zero rate cuts nothing; nothing divides by it here */
	check_frames(NS, 0, 0);
	check_frames(UINT64_MAX, 0, 0);
}

static void frames_edges(void **state)
{
	(void)state;

	/* the longest cut a kept packet of UINT32_MAX frames has at 48 kHz,
	 * one ns short of its length, and the length itself */
	check_frames(89478485312499ULL, 48000, UINT32_MAX - 1);
	check_frames(89478485312500ULL, 48000, UINT32_MAX);
	/* past what a kept packet has: the C's formula, exact here */
	check_frames(UINT64_MAX, 48000, 885443715538058ULL);
	check_frames(UINT64_MAX, 1, 18446744073ULL);
}

/* ------------------------------------------------------------------------ */
/* both, as prepare_audio() calls them */

static void trim_grid(void **state)
{
	(void)state;
	static const uint64_t starts[] = {1, 1000, NS, 5 * NS, 0x8000000000000000ULL, UINT64_MAX - 1, UINT64_MAX};
	static const uint64_t before[] = {1, 2, 20833, 20834, PKT - 1, PKT, PKT + 1, NS, 89478485312500ULL};
	static const uint32_t frames[] = {0, 1, 2, 480, 1023, 1024, 1025, 4096, UINT32_MAX - 1, UINT32_MAX};
	static const size_t rates[] = {1, 2, 8000, 22050, 44100, 48000, 96000, 192000, 384000, NS, UINT32_MAX};

	for (size_t s = 0; s < sizeof(starts) / sizeof(starts[0]); s++) {
		for (size_t b = 0; b < sizeof(before) / sizeof(before[0]); b++) {
			const uint64_t v = starts[s];
			if (before[b] > v)
				continue;
			const uint64_t ts = v - before[b];
			for (size_t f = 0; f < sizeof(frames) / sizeof(frames[0]); f++) {
				for (size_t r = 0; r < sizeof(rates) / sizeof(rates[0]); r++) {
					const bool drop = master_drops(ts, frames[f], v, rates[r]);
					check_drops(ts, frames[f], v, rates[r], drop);
					if (drop)
						continue;
					const uint64_t cut = obs_output_raw_audio_cut_ns(v, ts);
					const uint64_t cut_frames = master_frames(cut, rates[r]);
					check_frames(cut, rates[r], cut_frames);
					/* a kept packet's cut is shorter than the packet */
					assert_true(cut_frames < frames[f]);
				}
			}
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(drops_at_48k),  cmocka_unit_test(drops_other_rates),  cmocka_unit_test(drops_edges),
		cmocka_unit_test(frames_at_48k), cmocka_unit_test(frames_other_rates), cmocka_unit_test(frames_edges),
		cmocka_unit_test(trim_grid),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
