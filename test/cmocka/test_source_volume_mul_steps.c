#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <math.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-source-volume-mul.h"

/*
 * Characterization of the volume multiply kernels of libobs/obs-source.c,
 * moved to libobs/obs-source-volume-mul.c before their Rust port (#3080):
 * - obs_source_volume_mul_block(): multiply_output_audio(), one volume over
 *   a whole mix, which apply_audio_volume() passes as one block of
 *   AUDIO_OUTPUT_FRAMES * channels samples from the mix's first channel;
 * - obs_source_volume_mul_data(): multiply_vol_data(), one channel by a
 *   volume per frame, from apply_audio_actions().
 * They pin the current C behaviour; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source into
 * itself and calls only them, through their header. The buffers are laid out
 * as allocate_audio_output_buffer() lays them out: per mix, MAX_AUDIO_CHANNELS
 * consecutive channels of AUDIO_OUTPUT_FRAMES samples. test_source_volume_mix
 * and test_source_volume_apply_steps see the kernels through the real libobs.
 *
 * Not covered: a NaN sample times a NaN volume. Which of the two payloads the
 * result carries depends on how the C is compiled (operand order, scalar or
 * vector), not on the source.
 */

/* AUDIO_OUTPUT_FRAMES and MAX_AUDIO_CHANNELS. */
#define FRAMES 1024
#define CHANNELS 8
#define MIX (FRAMES * CHANNELS)

/* A value no multiply below gives: the buffer after the samples a call may
 * touch must keep it. */
#define CANARY -123.25f

static float mix_buf[2 * MIX];
static float expected[2 * MIX];

static uint32_t bits(float f)
{
	uint32_t u;
	memcpy(&u, &f, sizeof(u));
	return u;
}

static float from_bits(uint32_t u)
{
	float f;
	memcpy(&f, &u, sizeof(f));
	return f;
}

/* A sample of channel ch, frame i: distinct, signed, some above 1. */
static float sample(size_t ch, size_t i)
{
	return ((float)(i % 97) - 48.0f) / 16.0f + (float)ch * 0.125f;
}

static void fill(void)
{
	for (size_t i = 0; i < 2 * MIX; i++)
		mix_buf[i] = CANARY;
	for (size_t ch = 0; ch < CHANNELS; ch++)
		for (size_t i = 0; i < FRAMES; i++)
			mix_buf[ch * FRAMES + i] = sample(ch, i);
	memcpy(expected, mix_buf, sizeof(expected));
}

static void check(const char *what)
{
	for (size_t i = 0; i < 2 * MIX; i++) {
		if (bits(mix_buf[i]) != bits(expected[i])) {
			fprintf(stderr, "%s: sample %zu is %08x (%g), expected %08x (%g)\n", what, i,
				(unsigned)bits(mix_buf[i]), (double)mix_buf[i], (unsigned)bits(expected[i]),
				(double)expected[i]);
			fail();
		}
	}
}

/* ------------------------------------------------------------------------ */
/* obs_source_volume_mul_block() */

/* One volume over 1, 2 and 8 channels: every sample of those channels is
 * multiplied, the rest of the mix and the next mix keep theirs. */
static void test_block_channels(void **state)
{
	(void)state;

	static const size_t channels[] = {1, 2, 8};
	static const float volumes[] = {0.0f, 1.0f, 0.5f, 20.0f, 0.316227766f};

	for (size_t c = 0; c < sizeof(channels) / sizeof(channels[0]); c++) {
		for (size_t v = 0; v < sizeof(volumes) / sizeof(volumes[0]); v++) {
			fill();
			for (size_t i = 0; i < FRAMES * channels[c]; i++)
				expected[i] *= volumes[v];
			obs_source_volume_mul_block(mix_buf, FRAMES * channels[c], volumes[v]);
			check("block");
		}
	}
}

/* A count of 0 touches nothing; an odd count stops exactly there. */
static void test_block_counts(void **state)
{
	(void)state;

	static const size_t counts[] = {0, 1, 3, 4, 5, 7, 8, 15, 16, 17, 1023, 1025};

	for (size_t c = 0; c < sizeof(counts) / sizeof(counts[0]); c++) {
		fill();
		for (size_t i = 0; i < counts[c]; i++)
			expected[i] *= 0.5f;
		obs_source_volume_mul_block(mix_buf, counts[c], 0.5f);
		check("block count");
	}
}

/* Special values: a volume of 0 turns +/-inf samples into NaN and keeps the
 * sign of the others (-0.0 for negative ones); -0.0 flips signs; a NaN or
 * infinite volume spreads to every sample (characterized, not endorsed:
 * obs_source_volume_snap() keeps those volumes out of the mix since #2108). */
static void test_block_special(void **state)
{
	(void)state;

	static const float specials[] = {0.0f, -0.0f, 1.0f, -1.0f, INFINITY, -INFINITY, NAN, 1e-45f, 3.4e38f};

	for (size_t v = 0; v < sizeof(specials) / sizeof(specials[0]); v++) {
		fill();
		/* The first samples hold every special value too. */
		for (size_t i = 0; i < sizeof(specials) / sizeof(specials[0]); i++)
			mix_buf[i] = expected[i] = specials[i];
		const float vol = specials[v];
		for (size_t i = 0; i < FRAMES; i++) {
			/* A NaN times a NaN carries either payload: not pinned. */
			if (isnan(vol) && isnan(expected[i]))
				continue;
			expected[i] *= vol;
		}
		obs_source_volume_mul_block(mix_buf, FRAMES, vol);
		if (isnan(vol)) {
			/* Skip the NaN sample: index 6. */
			assert_true(isnan(mix_buf[6]));
			mix_buf[6] = expected[6];
		}
		check("block special");
	}

	/* 0 * inf is the default NaN, a NaN sample keeps its payload. */
	fill();
	mix_buf[0] = INFINITY;
	mix_buf[1] = from_bits(0x7fc00123);
	obs_source_volume_mul_block(mix_buf, 2, 0.0f);
	assert_true(isnan(mix_buf[0]));
	assert_int_equal(bits(mix_buf[1]), 0x7fc00123);
}

/* ------------------------------------------------------------------------ */
/* obs_source_volume_mul_data() */

/* A fade across the tick, channel by channel the way multiply_vol_data()
 * calls it: each channel gets the same vol_data, the next channel and mix
 * keep their samples until their own call. */
static void test_data_ramp(void **state)
{
	(void)state;

	static float vol[FRAMES];
	for (size_t i = 0; i < FRAMES; i++)
		vol[i] = (float)i / (float)(FRAMES - 1);

	static const size_t channels[] = {1, 2, 8};
	for (size_t c = 0; c < sizeof(channels) / sizeof(channels[0]); c++) {
		fill();
		for (size_t ch = 0; ch < channels[c]; ch++) {
			for (size_t i = 0; i < FRAMES; i++)
				expected[ch * FRAMES + i] *= vol[i];
			obs_source_volume_mul_data(mix_buf + ch * FRAMES, vol, FRAMES);
		}
		check("data ramp");
	}
}

/* apply_audio_actions()'s shape: the old volume up to an action's frame,
 * then the new one (a mute at frame 300 here). */
static void test_data_step(void **state)
{
	(void)state;

	static float vol[FRAMES];
	for (size_t i = 0; i < FRAMES; i++)
		vol[i] = i < 300 ? 0.75f : 0.0f;

	fill();
	for (size_t i = 0; i < FRAMES; i++)
		expected[i] *= vol[i];
	obs_source_volume_mul_data(mix_buf, vol, FRAMES);
	check("data step");
	/* A negative sample after the mute is -0.0: sample(0, 300) is -2.4375. */
	assert_int_equal(bits(mix_buf[300]), 0x80000000u);
}

/* Counts other than a tick, and special values in either operand. */
static void test_data_counts_and_special(void **state)
{
	(void)state;

	static const size_t counts[] = {0, 1, 3, 4, 5, 8, 17, 1023};
	static const float specials[] = {0.0f, -0.0f, 1.0f, -1.0f, INFINITY, -INFINITY, NAN, 1e-45f, 3.4e38f};
	static float vol[FRAMES];

	for (size_t c = 0; c < sizeof(counts) / sizeof(counts[0]); c++) {
		for (size_t i = 0; i < FRAMES; i++)
			vol[i] = specials[(i * 7 + c) % (sizeof(specials) / sizeof(specials[0]))];
		fill();
		for (size_t i = 0; i < counts[c]; i++)
			expected[i] *= vol[i];
		obs_source_volume_mul_data(mix_buf, vol, counts[c]);
		check("data count");
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_block_channels), cmocka_unit_test(test_block_counts),
		cmocka_unit_test(test_block_special),  cmocka_unit_test(test_data_ramp),
		cmocka_unit_test(test_data_step),      cmocka_unit_test(test_data_counts_and_special),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
