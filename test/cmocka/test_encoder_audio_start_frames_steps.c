#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <limits.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-encoder-audio-start.h"
#include "obs-encoder-audio-start-frames.h"
#include "util/util_uint64.h"

/*
 * Characterization of the two sample-rate conversions of the encoder's audio
 * start in libobs/obs-encoder.c, moved to
 * libobs/obs-encoder-audio-start-frames.c before their Rust port:
 * - obs_encoder_audio_start_frames_offset(): calc_offset_size()'s offset
 *   from the audio's start to the video's in frames,
 *   util_mul_div64(offset_ns, samplerate, 1000000000), which it passes to
 *   obs_encoder_audio_start_offset_size();
 * - obs_encoder_audio_start_frames_duration(): buffer_audio()'s packet
 *   length, util_mul_div64(frames, 1000000000, samplerate), which it passes
 *   to obs_encoder_audio_start_packet() (the packet cases run both, as
 *   buffer_audio() does).
 * They pin the current C behaviour; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * Every case is also checked against the expressions obs-encoder.c had
 * before the move, written out here with the real util_mul_div64(). Rates
 * stay between 1 and UINT32_MAX and every result fits in 64 bits, where
 * util_mul_div64() gives the same result on every platform. A zero rate
 * divides by zero in _frames_duration() (a trap on MSVC x64, SIGFPE on x86),
 * so it is not called with one.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them, through their header.
 * test_encoder_audio_start_steps (buffer_audio() copied onto a stand-in
 * encoder) and the real-libobs output tests cover the callers.
 *
 * Nothing here comes from outside OBS: timestamps come from the audio and
 * video threads, the frame count from the audio thread, the rate from the
 * encoder plugin's audio info.
 */

#define NS 1000000000ULL
/* one 1024-frame packet at 48 kHz: util_mul_div64(1024, NS, 48000) */
#define PKT 21333333ULL

static uint64_t master_offset(uint64_t offset_ns, uint32_t samplerate)
{
	return util_mul_div64(offset_ns, samplerate, 1000000000ULL);
}

static uint64_t master_duration(uint32_t frames, uint32_t samplerate)
{
	return util_mul_div64(frames, 1000000000ULL, samplerate);
}

static void check_offset(uint64_t offset_ns, uint32_t samplerate, uint64_t expect)
{
	uint64_t got = obs_encoder_audio_start_frames_offset(offset_ns, samplerate);
	if (got != expect || got != master_offset(offset_ns, samplerate)) {
		fprintf(stderr, "offset(%llu, %u) = %llu, expected %llu\n", (unsigned long long)offset_ns, samplerate,
			(unsigned long long)got, (unsigned long long)expect);
		fail();
	}
}

static void check_duration(uint32_t frames, uint32_t samplerate, uint64_t expect)
{
	uint64_t got = obs_encoder_audio_start_frames_duration(frames, samplerate);
	if (got != expect || got != master_duration(frames, samplerate)) {
		fprintf(stderr, "duration(%u, %u) = %llu, expected %llu\n", frames, samplerate, (unsigned long long)got,
			(unsigned long long)expect);
		fail();
	}
}

/* buffer_audio() once the paired video has started: the length, then the
 * plan, as it runs them */
static struct obs_encoder_audio_start_plan packet(uint64_t timestamp, uint32_t frames, uint32_t samplerate,
						  uint64_t v_start_ts)
{
	return obs_encoder_audio_start_packet(timestamp, obs_encoder_audio_start_frames_duration(frames, samplerate),
					      v_start_ts);
}

/* calc_offset_size() as it runs: the offset, in frames, in bytes */
static size_t offset_size(uint64_t v_start_ts, uint64_t a_start_ts, uint32_t samplerate, size_t blocksize)
{
	uint64_t offset = obs_encoder_audio_start_offset_ns(v_start_ts, a_start_ts);
	offset = obs_encoder_audio_start_frames_offset(offset, samplerate);
	return obs_encoder_audio_start_offset_size(offset, blocksize);
}

/* a * b as 128 bits, high and low, without a 128-bit type (MSVC has none) */
struct u128 {
	uint64_t hi, lo;
};

static struct u128 mul128(uint64_t a, uint64_t b)
{
	uint64_t a_lo = a & 0xffffffffULL, a_hi = a >> 32;
	uint64_t b_lo = b & 0xffffffffULL, b_hi = b >> 32;
	uint64_t ll = a_lo * b_lo, lh = a_lo * b_hi, hl = a_hi * b_lo, hh = a_hi * b_hi;
	uint64_t mid = (ll >> 32) + (lh & 0xffffffffULL) + (hl & 0xffffffffULL);
	struct u128 r;
	r.lo = (mid << 32) | (ll & 0xffffffffULL);
	r.hi = hh + (lh >> 32) + (hl >> 32) + (mid >> 32);
	return r;
}

static bool le128(struct u128 a, struct u128 b)
{
	return a.hi < b.hi || (a.hi == b.hi && a.lo <= b.lo);
}

/* q is a * b / c rounded down: q * c <= a * b < (q + 1) * c */
static bool is_floor(uint64_t q, uint64_t a, uint64_t b, uint64_t c)
{
	struct u128 ab = mul128(a, b);
	struct u128 next = mul128(q, c);
	uint64_t lo = next.lo + c;

	next.hi += lo < next.lo;
	next.lo = lo;
	return le128(mul128(q, c), ab) && !le128(next, ab);
}

/* ------------------------------------------------------------------------ */
/* obs_encoder_audio_start_frames_duration() */

static void duration_common_rates(void **state)
{
	(void)state;

	/* 1024 frames */
	check_duration(1024, 48000, PKT);
	check_duration(1024, 44100, 23219954);
	check_duration(1024, 96000, 10666666);
	/* one frame rounds down */
	check_duration(1, 48000, 20833);
	check_duration(1, 44100, 22675);
	check_duration(1, 96000, 10416);
	/* exact: 10 ms */
	check_duration(480, 48000, 10000000);
	check_duration(441, 44100, 10000000);
	/* no frames */
	check_duration(0, 48000, 0);
}

static void duration_edges(void **state)
{
	(void)state;

	/* the most frames */
	check_duration(UINT32_MAX, 48000, 89478485312500ULL);
	check_duration(UINT32_MAX, 44100, 97391548639455ULL);
	check_duration(UINT32_MAX, 1000000000, UINT32_MAX);
	/* 1 Hz: a frame is a second; UINT32_MAX seconds still fit */
	check_duration(1, 1, NS);
	check_duration(UINT32_MAX, 1, 4294967295000000000ULL);
	/* over 1 GHz a frame lasts 0 ns (characterized, not endorsed) */
	check_duration(1, 1000000000, 1);
	check_duration(1, 1000000001, 0);
	check_duration(2, 2000000000, 1);
	check_duration(1024, 2000000000, 512);
	/* the highest rate */
	check_duration(1, UINT32_MAX, 0);
	check_duration(UINT32_MAX, UINT32_MAX, NS);
	check_duration(UINT32_MAX - 1, UINT32_MAX, NS - 1);
}

static void duration_drives_the_plan(void **state)
{
	(void)state;
	const uint64_t v = 5 * NS;
	struct obs_encoder_audio_start_plan p;

	/* a packet that ends exactly at the video start waits */
	p = packet(v - PKT, 1024, 48000, v);
	assert_true(p.wait);
	/* one ns later it starts, truncated */
	p = packet(v - PKT + 1, 1024, 48000, v);
	assert_false(p.wait);
	assert_true(p.truncate);
	assert_true(p.clear);
	assert_false(p.from_buffer);
	/* the length rounds down: 1 frame at 48 kHz is 20833 ns */
	assert_true(packet(v - 20833, 1, 48000, v).wait);
	assert_false(packet(v - 20832, 1, 48000, v).wait);
	/* 44.1 kHz */
	assert_true(packet(v - 23219954, 1024, 44100, v).wait);
	assert_false(packet(v - 23219953, 1024, 44100, v).wait);
	/* a frame of 0 ns just before the video waits (characterized, not
	 * endorsed) */
	assert_true(packet(v - 1, 1, 2000000000, v).wait);
	/* a packet after the video start starts from the buffer */
	p = packet(v + 1, 1024, 48000, v);
	assert_false(p.wait);
	assert_false(p.truncate);
	assert_false(p.clear);
	assert_true(p.from_buffer);
}

/* ------------------------------------------------------------------------ */
/* obs_encoder_audio_start_frames_offset() */

static void offset_common_rates(void **state)
{
	(void)state;

	/* a second */
	check_offset(NS, 48000, 48000);
	check_offset(NS, 1, 1);
	check_offset(NS, UINT32_MAX, UINT32_MAX);
	/* uneven offsets round down */
	check_offset(PKT, 48000, 1023);
	check_offset(23219954, 44100, 1023);
	check_offset(999999, 48000, 47);
	/* less than a frame is none; a frame plus a ns is one */
	check_offset(20833, 48000, 0);
	check_offset(20834, 48000, 1);
	check_offset(1, 48000, 0);
	check_offset(NS - 1, 1, 0);
	check_offset(1, UINT32_MAX, 4);
	/* none */
	check_offset(0, 48000, 0);
	check_offset(0, UINT32_MAX, 0);
}

static void offset_wide_products(void **state)
{
	(void)state;

	/* offset times rate is past 2^64; the quotient still fits */
	check_offset(UINT64_MAX, 48000, 885443715538058ULL);
	check_offset(UINT64_MAX, 44100, 813501413650591ULL);
	check_offset(UINT64_MAX, 96000, 1770887431076116ULL);
	check_offset(384307168202283ULL, 48000, 18446744073ULL);
	check_offset(4000000000000000000ULL, UINT32_MAX, 17179869180000000000ULL);
	check_offset(UINT64_MAX, 1000000000, UINT64_MAX);
	check_offset(UINT64_MAX, 1, 18446744073ULL);
}

static void offset_drives_the_size(void **state)
{
	(void)state;
	const uint64_t v = 5 * NS;

	/* a packet a whole packet before the video drops 1023 frames of 4 or
	 * 8 bytes, not 1024: the offset rounds down */
	assert_int_equal(offset_size(v, v - PKT, 48000, 4), 1023 * 4);
	assert_int_equal(offset_size(v, v - PKT, 48000, 8), 1023 * 8);
	/* one ns before the video drops nothing */
	assert_int_equal(offset_size(v, v - 1, 48000, 8), 0);
	/* a second before drops a second */
	assert_int_equal(offset_size(v, v - NS, 44100, 8), 44100 * 8);
}

/* ------------------------------------------------------------------------ */
/* both, over a grid, against the expressions before the move */

static void grid(void **state)
{
	(void)state;
	static const uint32_t rates[] = {1,          2,          8000,           11025,     16000,
					 22050,      32000,      44100,          48000,     88200,
					 96000,      192000,     384000,         999999999, 1000000000,
					 1000000001, 2000000000, UINT32_MAX - 1, UINT32_MAX};
	static const uint32_t frames[] = {
		0, 1, 2, 441, 480, 960, 1023, 1024, 1025, 2048, 65535, 65536, UINT32_MAX - 1, UINT32_MAX};
	static const uint64_t offsets[] = {0,
					   1,
					   999,
					   20833,
					   20834,
					   PKT,
					   NS - 1,
					   NS,
					   NS + 1,
					   60 * NS,
					   3600 * NS,
					   UINT32_MAX,
					   (uint64_t)UINT32_MAX + 1,
					   1ULL << 40};

	for (size_t r = 0; r < sizeof(rates) / sizeof(rates[0]); r++) {
		for (size_t f = 0; f < sizeof(frames) / sizeof(frames[0]); f++) {
			uint64_t d = obs_encoder_audio_start_frames_duration(frames[f], rates[r]);
			check_duration(frames[f], rates[r], d);
			assert_true(is_floor(d, frames[f], NS, rates[r]));
		}
		for (size_t o = 0; o < sizeof(offsets) / sizeof(offsets[0]); o++) {
			uint64_t n = obs_encoder_audio_start_frames_offset(offsets[o], rates[r]);
			check_offset(offsets[o], rates[r], n);
			assert_true(is_floor(n, offsets[o], rates[r], NS));
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(duration_common_rates),
		cmocka_unit_test(duration_edges),
		cmocka_unit_test(duration_drives_the_plan),
		cmocka_unit_test(offset_common_rates),
		cmocka_unit_test(offset_wide_products),
		cmocka_unit_test(offset_drives_the_size),
		cmocka_unit_test(grid),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
