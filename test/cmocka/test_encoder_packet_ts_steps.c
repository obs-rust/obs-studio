#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs.h"
#include "obs-encoder-packet-ts.h"

/*
 * Characterization of the packet timestamp decisions of libobs/obs-encoder.c,
 * moved to libobs/obs-encoder-packet-ts.c before their Rust port (#2798):
 * - obs_encoder_packet_ts_timebase(): the packet's timebase in do_encode()
 *   and in the GPU encode thread;
 * - obs_encoder_packet_ts_dts_usec(): packet_dts_usec(), which
 *   send_off_encoder_packet() also takes as offset_usec on the first
 *   received packet;
 * - obs_encoder_packet_ts_system() and _paused(): the packet's dts_usec on
 *   the system clock and its sys_dts_usec after the pause offset;
 * - obs_encoder_packet_ts_find(): the newest timing entry of a video packet's
 *   PTS.
 * They pin the current C behaviour; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source into
 * itself and calls only them, through their header. obs-encoder.c keeps the
 * encode callbacks, the locks, the callback walk, the darray, the first
 * keyframe test and the frame count. The real-libobs tests
 * (test_encoder_error_stop, test_encoder_join_keyframe,
 * test_multitrack_encoder_override, test_media_packet_eof_leak) cover the
 * callers.
 *
 * libobs sets the encoder's timebase from the video (timebase_num = fps_den,
 * timebase_den = fps_num) or the audio (1 / sample rate), and a packet's DTS
 * counts in units of 1 / timebase_den. Not covered, undefined in C:
 * dts * 1000000 beyond int64_t, a timebase_den of 0, and INT64_MIN / -1.
 */

#define NTSC_NUM 1001
#define NTSC_DEN 30000
#define T0_NS 5000000123ULL /* an encoder start time, in ns */

/* ------------------------------------------------------------------------ */
/* helpers */

static void check_timebase(const char *what, uint32_t num, uint32_t den, uint32_t divisor, int32_t want_num,
			   int32_t want_den)
{
	struct obs_encoder_packet_timebase got = obs_encoder_packet_ts_timebase(num, den, divisor);
	if (got.num != want_num || got.den != want_den) {
		fprintf(stderr, "%s: %d/%d; expected %d/%d\n", what, got.num, got.den, want_num, want_den);
		fail();
	}
}

static void check_i64(const char *what, int64_t got, int64_t want)
{
	if (got != want) {
		fprintf(stderr, "%s: %lld; expected %lld\n", what, (long long)got, (long long)want);
		fail();
	}
}

static void check_idx(const char *what, size_t got, size_t want)
{
	if (got != want) {
		fprintf(stderr, "%s: %zu; expected %zu\n", what, got, want);
		fail();
	}
}

/* ------------------------------------------------------------------------ */
/* the packet timebase */

static void timebase_from_the_encoder(void **state)
{
	(void)state;
	check_timebase("30 fps", 1, 30, 1, 1, 30);
	check_timebase("29.97 fps", NTSC_NUM, NTSC_DEN, 1, 1001, 30000);
	check_timebase("60 fps, divisor 2", 1, 60, 2, 2, 60);
	check_timebase("29.97 fps, divisor 3", NTSC_NUM, NTSC_DEN, 3, 3003, 30000);
	check_timebase("48 kHz audio", 1, 48000, 1, 1, 48000);
}

/* Characterized, not endorsed: the product wraps as uint32_t, and values
 * above INT32_MAX are stored negative in the int32_t fields. */
static void timebase_wraps_at_the_limits(void **state)
{
	(void)state;
	check_timebase("product beyond INT32_MAX", 1001, 30000, 3000000, -1291967296, 30000);
	check_timebase("product wraps to 0", 65536, 30, 65536, 0, 30);
	check_timebase("den beyond INT32_MAX", 1, 0x80000000u, 1, 1, INT32_MIN);
	check_timebase("divisor 0", 1, 30, 0, 0, 30);
}

/* ------------------------------------------------------------------------ */
/* packet_dts_usec() */

static void dts_usec_of_common_rates(void **state)
{
	(void)state;
	check_i64("30 fps, 1 s", obs_encoder_packet_ts_dts_usec(30, 30), 1000000);
	check_i64("29.97 fps, one frame", obs_encoder_packet_ts_dts_usec(NTSC_NUM, NTSC_DEN), 33366);
	check_i64("29.97 fps, 30 frames", obs_encoder_packet_ts_dts_usec(30 * NTSC_NUM, NTSC_DEN), 1001000);
	check_i64("25 fps, frame 3", obs_encoder_packet_ts_dts_usec(3, 25), 120000);
	check_i64("48 kHz, one AAC frame", obs_encoder_packet_ts_dts_usec(1024, 48000), 21333);
	check_i64("dts 0", obs_encoder_packet_ts_dts_usec(0, 30), 0);
}

/* B-frames: the first DTS is below 0. C's division truncates toward 0. */
static void dts_usec_negative_truncates_toward_zero(void **state)
{
	(void)state;
	check_i64("29.97 fps, -1 frame", obs_encoder_packet_ts_dts_usec(-NTSC_NUM, NTSC_DEN), -33366);
	check_i64("29.97 fps, -2 frames", obs_encoder_packet_ts_dts_usec(-2 * NTSC_NUM, NTSC_DEN), -66733);
	check_i64("48 kHz, -1 sample", obs_encoder_packet_ts_dts_usec(-1, 48000), -20);
}

/* Characterized, not endorsed: a negative timebase_den (from a den beyond
 * INT32_MAX) flips the sign; the largest dts that does not overflow. */
static void dts_usec_at_the_limits(void **state)
{
	(void)state;
	check_i64("den -1", obs_encoder_packet_ts_dts_usec(5, -1), -5000000);
	check_i64("INT32_MIN den", obs_encoder_packet_ts_dts_usec(INT64_MAX / 1000000, INT32_MIN), -4294967295LL);
	check_i64("INT64_MAX / 1000000", obs_encoder_packet_ts_dts_usec(INT64_MAX / 1000000, 1),
		  INT64_MAX / 1000000 * 1000000);
	check_i64("INT64_MIN / 1000000", obs_encoder_packet_ts_dts_usec(INT64_MIN / 1000000, 1),
		  INT64_MIN / 1000000 * 1000000);
}

/* ------------------------------------------------------------------------ */
/* dts_usec and sys_dts_usec */

/* The first received packet sets offset_usec; each packet is then placed at
 * the encoder's start time plus its distance from that first packet. */
static void system_time_from_the_first_packet(void **state)
{
	(void)state;
	int64_t offset = obs_encoder_packet_ts_dts_usec(-2 * NTSC_NUM, NTSC_DEN);

	check_i64("first packet", obs_encoder_packet_ts_system(T0_NS, offset, offset), 5000000);
	check_i64("second packet",
		  obs_encoder_packet_ts_system(T0_NS, obs_encoder_packet_ts_dts_usec(-NTSC_NUM, NTSC_DEN), offset),
		  5000000 + 33367);
	check_i64("dts 0", obs_encoder_packet_ts_system(T0_NS, 0, offset), 5000000 + 66733);
	check_i64("start 0, first dts 0", obs_encoder_packet_ts_system(0, 21333, 0), 21333);
	check_i64("start below 1 us", obs_encoder_packet_ts_system(999, 0, 0), 0);
}

/* A packet earlier than the first one (an encoder whose DTS goes back) gives
 * a time before the start; below 0 it is negative. */
static void system_time_before_the_start(void **state)
{
	(void)state;
	check_i64("10 us before", obs_encoder_packet_ts_system(T0_NS, -10, 0), 4999990);
	check_i64("before 0", obs_encoder_packet_ts_system(0, 0, 5), -5);
}

/* Characterized, not endorsed: the unsigned sum wraps, and a sum above
 * INT64_MAX is stored negative. */
static void system_time_wraps_at_the_limits(void **state)
{
	(void)state;
	check_i64("start beyond INT64_MAX ns", obs_encoder_packet_ts_system(UINT64_MAX, 0, 0), 18446744073709551LL);
	check_i64("sum above INT64_MAX", obs_encoder_packet_ts_system(1000, INT64_MAX, 0), INT64_MIN);
	check_i64("offset INT64_MIN", obs_encoder_packet_ts_system(0, 0, INT64_MIN), INT64_MIN);
}

static void paused_adds_the_pause_offset(void **state)
{
	(void)state;
	check_i64("no pause", obs_encoder_packet_ts_paused(5000000, 0), 5000000);
	check_i64("2.5 s paused", obs_encoder_packet_ts_paused(5000000, 2500000000ULL), 7500000);
	check_i64("below 1 us", obs_encoder_packet_ts_paused(5000000, 999), 5000000);
	check_i64("negative time", obs_encoder_packet_ts_paused(-5, 7000), 2);
	/* Characterized, not endorsed: the unsigned sum wraps. */
	check_i64("above INT64_MAX", obs_encoder_packet_ts_paused(INT64_MAX, 1000), INT64_MIN);
	check_i64("largest offset", obs_encoder_packet_ts_paused(0, UINT64_MAX), 18446744073709551LL);
}

/* ------------------------------------------------------------------------ */
/* the timing entry */

static void find_the_newest_entry(void **state)
{
	(void)state;
	struct encoder_packet_time times[5] = {{.pts = 0, .cts = 10},
					       {.pts = 1, .cts = 11},
					       {.pts = 2, .cts = 12},
					       {.pts = 1, .cts = 13},
					       {.pts = 3}};

	check_idx("empty", obs_encoder_packet_ts_find(NULL, 0, 0), OBS_ENCODER_PACKET_TS_NOT_FOUND);
	check_idx("one entry", obs_encoder_packet_ts_find(times, 1, 0), 0);
	check_idx("first entry", obs_encoder_packet_ts_find(times, 5, 0), 0);
	check_idx("last entry", obs_encoder_packet_ts_find(times, 5, 3), 4);
	check_idx("two entries, the newest", obs_encoder_packet_ts_find(times, 5, 1), 3);
	check_idx("only the first num", obs_encoder_packet_ts_find(times, 3, 1), 1);
	check_idx("no entry", obs_encoder_packet_ts_find(times, 5, 7), OBS_ENCODER_PACKET_TS_NOT_FOUND);
	check_idx("negative pts", obs_encoder_packet_ts_find(times, 5, -1), OBS_ENCODER_PACKET_TS_NOT_FOUND);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(timebase_from_the_encoder),
		cmocka_unit_test(timebase_wraps_at_the_limits),
		cmocka_unit_test(dts_usec_of_common_rates),
		cmocka_unit_test(dts_usec_negative_truncates_toward_zero),
		cmocka_unit_test(dts_usec_at_the_limits),
		cmocka_unit_test(system_time_from_the_first_packet),
		cmocka_unit_test(system_time_before_the_start),
		cmocka_unit_test(system_time_wraps_at_the_limits),
		cmocka_unit_test(paused_adds_the_pause_offset),
		cmocka_unit_test(find_the_newest_entry),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
