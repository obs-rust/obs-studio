#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-source-transition-clock.h"
#include "obs-source-transition-sample-time.h"

/*
 * Characterization of the time of each audio sample a transition mixes,
 * get_sample_time() of libobs/obs-source-transition.c, moved to
 * libobs/obs-source-transition-sample-time.c before its Rust port:
 * obs_source_transition_sample_time() adds util_mul_div64(sample,
 * 1000000000, sample_rate) ns to the block's timestamp, in uint64_t, and
 * reads the transition's clock (obs_source_transition_clock_time(),
 * calc_time()) at that time. Cases marked "characterized, not endorsed" are
 * odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C sources
 * (the sample time and the clock it asks) into itself and calls only them.
 * test_transition_audio_rescale, test_group_hide_transition_audio and
 * test_fade_to_color_mix cover the audio mix through the real libobs.
 *
 * A sample rate of 0 is not covered: the C divides by zero there.
 *
 * Every pinned float is an exact ratio, so the clock's long double divide
 * gives the same float on every platform. Every sample index is under
 * AUDIO_OUTPUT_FRAMES, where util_mul_div64()'s per-platform forms agree
 * with sample * 1000000000 / rate in uint64_t.
 */

#define MANUAL OBS_SOURCE_TRANSITION_CLOCK_MODE_MANUAL
#define AUTO 0 /* OBS_TRANSITION_MODE_AUTO */

#define FRAMES 1024 /* AUDIO_OUTPUT_FRAMES */
#define TWO_25 (1ULL << 25)

static uint32_t bits_of(float value)
{
	uint32_t bits;
	memcpy(&bits, &value, sizeof(bits));
	return bits;
}

static float float_of(uint32_t bits)
{
	float value;
	memcpy(&value, &bits, sizeof(value));
	return value;
}

static void check_bits(float got, float want, const char *what, size_t rate, size_t sample)
{
	if (bits_of(got) != bits_of(want)) {
		fprintf(stderr, "%s: rate %zu sample %zu: %08x, want %08x\n", what, rate, sample,
			(unsigned)bits_of(got), (unsigned)bits_of(want));
		fail();
	}
}

/* The clock at the block's time plus sample * 10^9 / rate, the offset
 * computed exactly. */
static float expected(int mode, float manual_val, uint64_t start_time, uint64_t duration, uint64_t ts,
		      size_t sample_rate, size_t sample)
{
	uint64_t offset = (uint64_t)sample * 1000000000ULL / (uint64_t)sample_rate;
	return obs_source_transition_clock_time(mode, manual_val, start_time, duration, ts + offset);
}

static const size_t rates[] = {1, 2, 3, 7, 8000, 11025, 16000, 22050, 32000, 44100, 48000, 88200, 96000, 192000};

static void sample_zero_is_the_block_time(void **state)
{
	(void)state;

	const uint64_t ts_values[] = {0, 1, 999, 1000, 1001, 5000, 1000 + TWO_25, UINT64_MAX};

	for (size_t r = 0; r < sizeof(rates) / sizeof(rates[0]); r++) {
		for (size_t i = 0; i < sizeof(ts_values) / sizeof(ts_values[0]); i++) {
			uint64_t ts = ts_values[i];
			check_bits(obs_source_transition_sample_time(AUTO, 0.25f, 1000, TWO_25, ts, rates[r], 0),
				   obs_source_transition_clock_time(AUTO, 0.25f, 1000, TWO_25, ts), "auto", rates[r],
				   0);
			check_bits(obs_source_transition_sample_time(MANUAL, 0.25f, 1000, TWO_25, ts, rates[r], 0),
				   0.25f, "manual", rates[r], 0);
		}
	}
}

static void every_sample_of_a_block(void **state)
{
	(void)state;

	/* The block starts before the transition and ends inside it for the
	 * slower rates, past its end for rate 1 and 2. */
	const uint64_t ts = 5000000;
	const uint64_t start = 5000000 + 7000000;
	const uint64_t duration = 900000000;

	for (size_t r = 0; r < sizeof(rates) / sizeof(rates[0]); r++)
		for (size_t sample = 0; sample < FRAMES; sample++)
			check_bits(obs_source_transition_sample_time(AUTO, 0.0f, start, duration, ts, rates[r], sample),
				   expected(AUTO, 0.0f, start, duration, ts, rates[r], sample), "block", rates[r],
				   sample);
}

static void last_sample_of_a_block(void **state)
{
	(void)state;

	/* 1023 samples: 21312500 ns at 48000, 23197278.9 (floored) at 44100,
	 * 10656250 at 96000; each over 2^25 is an exact float. */
	assert_true(obs_source_transition_sample_time(AUTO, 0.0f, 1000, TWO_25, 1000, 48000, FRAMES - 1) ==
		    21312500.0f / 33554432.0f);
	assert_true(obs_source_transition_sample_time(AUTO, 0.0f, 1000, TWO_25, 1000, 44100, FRAMES - 1) ==
		    23197278.0f / 33554432.0f);
	assert_true(obs_source_transition_sample_time(AUTO, 0.0f, 1000, TWO_25, 1000, 96000, FRAMES - 1) ==
		    10656250.0f / 33554432.0f);
}

static void offset_is_floored(void **state)
{
	(void)state;

	/* 10^9 / 44100 = 22675.7 ns: sample 1 is at ts + 22675. The clock is
	 * 0 up to and at the start, so a start at ts + 22675 gives 0 and one a
	 * nanosecond earlier gives 1 / duration. */
	assert_true(obs_source_transition_sample_time(AUTO, 0.0f, 22675, TWO_25, 0, 44100, 1) == 0.0f);
	assert_true(obs_source_transition_sample_time(AUTO, 0.0f, 22674, TWO_25, 0, 44100, 1) == 1.0f / 33554432.0f);

	/* 10^9 / 3 = 333333333.3 ns, 2 * 10^9 / 3 = 666666666.7 ns */
	assert_true(obs_source_transition_sample_time(AUTO, 0.0f, 333333333, TWO_25, 0, 3, 1) == 0.0f);
	assert_true(obs_source_transition_sample_time(AUTO, 0.0f, 333333332, TWO_25, 0, 3, 1) == 1.0f / 33554432.0f);
	assert_true(obs_source_transition_sample_time(AUTO, 0.0f, 666666666, TWO_25, 0, 3, 2) == 0.0f);
	assert_true(obs_source_transition_sample_time(AUTO, 0.0f, 666666665, TWO_25, 0, 3, 2) == 1.0f / 33554432.0f);

	/* 1023 * 10^9 / 7 = 146142857142.86 ns */
	assert_true(obs_source_transition_sample_time(AUTO, 0.0f, 146142857142ULL, TWO_25, 0, 7, FRAMES - 1) == 0.0f);
	assert_true(obs_source_transition_sample_time(AUTO, 0.0f, 146142857141ULL, TWO_25, 0, 7, FRAMES - 1) ==
		    1.0f / 33554432.0f);
}

static void manual_mode_does_not_move(void **state)
{
	(void)state;

	const uint32_t manual_bits[] = {0x00000000, 0x80000000, 0x3e800000, 0x3f800000, 0x7f800000,
					0x7fc00001, 0xff800000, 0x00000001, 0x4b000000};

	for (size_t m = 0; m < sizeof(manual_bits) / sizeof(manual_bits[0]); m++) {
		float manual_val = float_of(manual_bits[m]);
		for (size_t sample = 0; sample < FRAMES; sample += 31)
			check_bits(obs_source_transition_sample_time(MANUAL, manual_val, 0, TWO_25, 1000, 48000,
								     sample),
				   manual_val, "manual", 48000, sample);
	}
}

static void other_modes_are_automatic(void **state)
{
	(void)state;

	/* mode 0 (auto), and values outside enum obs_transition_mode, are not
	 * manual: the clock runs and the manual value is ignored
	 * (characterized, not endorsed for the values outside the enum) */
	const int modes[] = {AUTO, 2, 7, -1};

	for (size_t m = 0; m < sizeof(modes) / sizeof(modes[0]); m++) {
		assert_true(obs_source_transition_sample_time(modes[m], 0.75f, 0, TWO_25, 0, 48000, 0) == 0.0f);
		assert_true(obs_source_transition_sample_time(modes[m], 0.75f, 0, TWO_25, 0, 48000, FRAMES - 1) ==
			    21312500.0f / 33554432.0f);
	}
}

static void block_before_straddling_and_after(void **state)
{
	(void)state;

	/* At 48000 Hz a block spans 1023 samples = 21312500 ns. */
	const uint64_t span = 21312500;
	const uint64_t start = 1000000000;
	const uint64_t duration = TWO_25;

	/* before the transition: the last sample is at the start, so 0 */
	for (size_t sample = 0; sample < FRAMES; sample++)
		check_bits(obs_source_transition_sample_time(AUTO, 0.0f, start, duration, start - span, 48000, sample),
			   0.0f, "before", 48000, sample);

	/* straddling the end: under 1 until the end, 1 from it on */
	const uint64_t ts = start + duration - span / 2;
	size_t first_one = FRAMES;
	for (size_t sample = 0; sample < FRAMES; sample++) {
		float t = obs_source_transition_sample_time(AUTO, 0.0f, start, duration, ts, 48000, sample);
		if (t == 1.0f && first_one == FRAMES)
			first_one = sample;
		if (first_one == FRAMES)
			assert_true(t < 1.0f);
		else
			assert_true(t == 1.0f);
	}
	/* the end is 10656250 ns into the block: sample 511.5, so 512. Sample
	 * 511 is 10645833 ns in, 33554432 - 10656250 + 10645833 = 33544015 ns
	 * after the start. */
	assert_int_equal(first_one, 512);
	assert_true(obs_source_transition_sample_time(AUTO, 0.0f, start, duration, ts, 48000, 511) ==
		    (float)33544015ULL / 33554432.0f);

	/* after the transition: 1 */
	for (size_t sample = 0; sample < FRAMES; sample++)
		check_bits(obs_source_transition_sample_time(AUTO, 0.0f, start, duration, start + duration, 48000,
							     sample),
			   1.0f, "after", 48000, sample);

	/* a duration of 0: 1 from the first sample after the start */
	assert_true(obs_source_transition_sample_time(AUTO, 0.0f, start, 0, start, 48000, 0) == 0.0f);
	assert_true(obs_source_transition_sample_time(AUTO, 0.0f, start, 0, start, 48000, 1) == 1.0f);
}

static void timestamp_sum_wraps(void **state)
{
	(void)state;

	/* ts + offset wraps in uint64_t (characterized, not endorsed): sample 1
	 * at 48000 Hz is 20833 ns past UINT64_MAX - 10, which wraps to 20822,
	 * past a start of 0 by 20822 ns; without the wrap the clock would be 1 */
	assert_true(obs_source_transition_sample_time(AUTO, 0.0f, 0, TWO_25, UINT64_MAX - 10, 48000, 1) ==
		    20822.0f / 33554432.0f);
	/* sample 0 stays at UINT64_MAX - 10: past the end */
	assert_true(obs_source_transition_sample_time(AUTO, 0.0f, 0, TWO_25, UINT64_MAX - 10, 48000, 0) == 1.0f);
	/* a block ending exactly at the wrap: UINT64_MAX - 20833 + 20833 */
	assert_true(obs_source_transition_sample_time(AUTO, 0.0f, 0, TWO_25, UINT64_MAX - 20833, 48000, 1) == 1.0f);
	assert_true(obs_source_transition_sample_time(AUTO, 0.0f, 0, TWO_25, UINT64_MAX - 20832, 48000, 1) == 0.0f);
}

static void slow_rates(void **state)
{
	(void)state;

	/* 1 Hz: sample n is n seconds in; 1023 s */
	assert_true(obs_source_transition_sample_time(AUTO, 0.0f, 0, 2048000000000ULL, 0, 1, FRAMES - 1) ==
		    1023.0f / 2048.0f);
	/* 2 Hz: 511.5 s */
	assert_true(obs_source_transition_sample_time(AUTO, 0.0f, 0, 2048000000000ULL, 0, 2, FRAMES - 1) ==
		    511.5f / 2048.0f);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(sample_zero_is_the_block_time),
		cmocka_unit_test(every_sample_of_a_block),
		cmocka_unit_test(last_sample_of_a_block),
		cmocka_unit_test(offset_is_floored),
		cmocka_unit_test(manual_mode_does_not_move),
		cmocka_unit_test(other_modes_are_automatic),
		cmocka_unit_test(block_before_straddling_and_after),
		cmocka_unit_test(timestamp_sum_wraps),
		cmocka_unit_test(slow_rates),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
