#ifndef _USE_MATH_DEFINES
#define _USE_MATH_DEFINES
#endif

#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <math.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-source-audio-input.h"

#ifndef M_PI
#define M_PI 3.14159265358979323846
#endif

/*
 * Characterization of the audio input steps of process_audio() and
 * reset_resampler() in libobs/obs-source.c, moved to
 * libobs/obs-source-audio-input.c before their Rust port:
 * - obs_source_audio_input_discontinuous(): with a resampler and
 *   next_resample_in_ts set, a packet 70 ms or more from it starts a new
 *   stream (#546);
 * - obs_source_audio_input_reset_needed(): rate, format, speakers, the
 *   compensation flag, or the discontinuity;
 * - obs_source_audio_input_resampler_needed(): the clock drift fix's "no
 *   resampler needed" rule;
 * - the balance gate and the Force Mono gate;
 * - obs_source_audio_input_balance(): the sine, square and linear laws;
 * - obs_source_audio_input_downmix(): the mean of the planes.
 * Cases marked "characterized, not endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them. test_source_audio_balance_mono covers the
 * gates through the real libobs, and test_audio_restart_resampler_tail and
 * test_audio_clock_drift the resampler rules.
 */

#define MSEC 1000000ULL
#define THRESHOLD OBS_SOURCE_AUDIO_INPUT_SMOOTHING_THRESHOLD

/* enum audio_format, enum speaker_layout (media-io/audio-io.h) and the
 * source flags (obs.h); obs-source.c checks the header's against them */
#define FORMAT_FLOAT_PLANAR 8 /* AUDIO_FORMAT_FLOAT_PLANAR */
#define FORMAT_16BIT 2        /* AUDIO_FORMAT_16BIT */
#define SPEAKERS_MONO 1
#define SPEAKERS_STEREO OBS_SOURCE_AUDIO_INPUT_SPEAKERS_STEREO
#define SPEAKERS_5POINT1 6
#define FORCE_MONO OBS_SOURCE_AUDIO_INPUT_FORCE_MONO
#define SINE OBS_SOURCE_AUDIO_INPUT_SINE_LAW
#define SQUARE OBS_SOURCE_AUDIO_INPUT_SQUARE_LAW
#define LINEAR OBS_SOURCE_AUDIO_INPUT_LINEAR

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

/* The next float up from a positive one, and down. */
static float next_up(float value)
{
	return float_of(bits_of(value) + 1);
}

static float next_down(float value)
{
	return float_of(bits_of(value) - 1);
}

static void check_bits_line(float actual, float expected, const char *what, size_t index, int line)
{
	if (bits_of(actual) != bits_of(expected)) {
		fprintf(stderr, "line %d: %s[%zu] is %a (0x%08x), expected %a (0x%08x)\n", line, what, index,
			(double)actual, bits_of(actual), (double)expected, bits_of(expected));
		fail();
	}
}

#define check_bits(actual, expected, what, index) check_bits_line(actual, expected, what, index, __LINE__)

static struct obs_source_audio_input_format format_of(int format, uint32_t samples_per_sec, int speakers)
{
	struct obs_source_audio_input_format f = {
		.format = format,
		.samples_per_sec = samples_per_sec,
		.speakers = speakers,
	};
	return f;
}

/* ---- the discontinuity (#546) ---- */

static void discontinuity_at_the_threshold(void **state)
{
	(void)state;
	const uint64_t next = 3600000 * MSEC;

	/* later than expected */
	assert_false(obs_source_audio_input_discontinuous(true, next, next));
	assert_false(obs_source_audio_input_discontinuous(true, next, next + THRESHOLD - 1));
	assert_true(obs_source_audio_input_discontinuous(true, next, next + THRESHOLD));
	assert_true(obs_source_audio_input_discontinuous(true, next, next + THRESHOLD + 1));

	/* earlier than expected */
	assert_false(obs_source_audio_input_discontinuous(true, next, next - THRESHOLD + 1));
	assert_true(obs_source_audio_input_discontinuous(true, next, next - THRESHOLD));
	assert_true(obs_source_audio_input_discontinuous(true, next, next - THRESHOLD - 1));

	/* the resampler restart test's gaps: 20 ms continues, 100 ms and 1 s
	 * restart */
	assert_false(obs_source_audio_input_discontinuous(true, next, next + 20 * MSEC));
	assert_true(obs_source_audio_input_discontinuous(true, next, next + 100 * MSEC));
	assert_true(obs_source_audio_input_discontinuous(true, next, next + 1000 * MSEC));
}

static void discontinuity_needs_a_resampler_and_an_end(void **state)
{
	(void)state;
	const uint64_t next = 3600000 * MSEC;

	/* no resampler: nothing is held back */
	assert_false(obs_source_audio_input_discontinuous(false, next, next + THRESHOLD));
	assert_false(obs_source_audio_input_discontinuous(false, next, 0));
	assert_false(obs_source_audio_input_discontinuous(false, next, UINT64_MAX));

	/* next_resample_in_ts == 0: no packet with a rate yet */
	assert_false(obs_source_audio_input_discontinuous(true, 0, 0));
	assert_false(obs_source_audio_input_discontinuous(true, 0, THRESHOLD));
	assert_false(obs_source_audio_input_discontinuous(true, 0, next));
	assert_false(obs_source_audio_input_discontinuous(true, 0, UINT64_MAX));
	assert_false(obs_source_audio_input_discontinuous(false, 0, UINT64_MAX));

	/* next_resample_in_ts == 1 is set */
	assert_true(obs_source_audio_input_discontinuous(true, 1, 1 + THRESHOLD));
	assert_false(obs_source_audio_input_discontinuous(true, 1, 0));
}

/* Characterized, not endorsed: the distance is not modular, so timestamps on
 * either side of UINT64_MAX are far apart. */
static void discontinuity_distance_is_not_modular(void **state)
{
	(void)state;

	assert_true(obs_source_audio_input_discontinuous(true, UINT64_MAX, 0));
	assert_true(obs_source_audio_input_discontinuous(true, 1, UINT64_MAX));
	assert_false(obs_source_audio_input_discontinuous(true, UINT64_MAX, UINT64_MAX - THRESHOLD + 1));
	assert_true(obs_source_audio_input_discontinuous(true, UINT64_MAX, UINT64_MAX - THRESHOLD));
	/* at the low end the expected end is never 0 */
	assert_false(obs_source_audio_input_discontinuous(true, THRESHOLD - 1, 0));
	assert_true(obs_source_audio_input_discontinuous(true, THRESHOLD, 0));
}

/* ---- the reset condition ---- */

static void reset_on_each_trigger_alone(void **state)
{
	(void)state;
	const struct obs_source_audio_input_format current = format_of(FORMAT_FLOAT_PLANAR, 44100, SPEAKERS_STEREO);

	/* nothing changed */
	assert_false(obs_source_audio_input_reset_needed(current, current, false, false, false));
	assert_false(obs_source_audio_input_reset_needed(current, current, true, true, false));

	/* each trigger on its own */
	assert_true(obs_source_audio_input_reset_needed(current, format_of(FORMAT_FLOAT_PLANAR, 48000, SPEAKERS_STEREO),
							false, false, false));
	assert_true(obs_source_audio_input_reset_needed(current, format_of(FORMAT_16BIT, 44100, SPEAKERS_STEREO), false,
							false, false));
	assert_true(obs_source_audio_input_reset_needed(
		current, format_of(FORMAT_FLOAT_PLANAR, 44100, SPEAKERS_5POINT1), false, false, false));
	/* the clock drift compensation turned on, and off */
	assert_true(obs_source_audio_input_reset_needed(current, current, false, true, false));
	assert_true(obs_source_audio_input_reset_needed(current, current, true, false, false));
	/* the discontinuity (#546) */
	assert_true(obs_source_audio_input_reset_needed(current, current, false, false, true));
	assert_true(obs_source_audio_input_reset_needed(current, current, true, true, true));

	/* everything at once */
	assert_true(obs_source_audio_input_reset_needed(current, format_of(FORMAT_16BIT, 48000, SPEAKERS_MONO), true,
							false, true));
}

static void reset_from_the_zeroed_source(void **state)
{
	(void)state;
	/* A new source's sample_info is zeroed: its first packet with a rate,
	 * a format or speakers resets. */
	const struct obs_source_audio_input_format zero = format_of(0, 0, 0);

	assert_true(obs_source_audio_input_reset_needed(zero, format_of(FORMAT_FLOAT_PLANAR, 48000, SPEAKERS_STEREO),
							false, false, false));
	assert_true(obs_source_audio_input_reset_needed(zero, format_of(0, 1, 0), false, false, false));
	assert_true(obs_source_audio_input_reset_needed(zero, format_of(1, 0, 0), false, false, false));
	assert_true(obs_source_audio_input_reset_needed(zero, format_of(0, 0, 1), false, false, false));
	/* characterized, not endorsed: a packet of all zeros does not */
	assert_false(obs_source_audio_input_reset_needed(zero, zero, false, false, false));
	/* values outside the enums are compared as they are */
	assert_true(obs_source_audio_input_reset_needed(zero, format_of(-1, 0, 0), false, false, false));
	assert_true(obs_source_audio_input_reset_needed(zero, format_of(0, UINT32_MAX, 0), false, false, false));
	assert_true(obs_source_audio_input_reset_needed(zero, format_of(0, 0, 7), false, false, false));
}

/* ---- the "no resampler needed" rule (the clock drift fix) ---- */

static void resampler_needed_rule(void **state)
{
	(void)state;
	const struct obs_source_audio_input_format output = format_of(FORMAT_FLOAT_PLANAR, 48000, SPEAKERS_STEREO);

	/* the output's format, no compensation: no resampler */
	assert_false(obs_source_audio_input_resampler_needed(output, output, false));
	/* the clock drift compensation needs one even then */
	assert_true(obs_source_audio_input_resampler_needed(output, output, true));

	/* each difference on its own */
	assert_true(obs_source_audio_input_resampler_needed(format_of(FORMAT_FLOAT_PLANAR, 44100, SPEAKERS_STEREO),
							    output, false));
	assert_true(obs_source_audio_input_resampler_needed(format_of(FORMAT_16BIT, 48000, SPEAKERS_STEREO), output,
							    false));
	assert_true(obs_source_audio_input_resampler_needed(format_of(FORMAT_FLOAT_PLANAR, 48000, SPEAKERS_MONO),
							    output, false));
	assert_true(
		obs_source_audio_input_resampler_needed(format_of(FORMAT_16BIT, 44100, SPEAKERS_MONO), output, true));

	/* characterized, not endorsed: a packet of all zeros against a zeroed
	 * output needs none (libobs's output is never zeroed) */
	assert_false(obs_source_audio_input_resampler_needed(format_of(0, 0, 0), format_of(0, 0, 0), false));
	assert_true(obs_source_audio_input_resampler_needed(format_of(0, 0, 0), output, false));
}

/* ---- the gates ---- */

static void balance_gate_at_the_edges(void **state)
{
	(void)state;

	/* 0.49 to 0.51 is centred */
	assert_false(obs_source_audio_input_balance_needed(false, SPEAKERS_STEREO, 0.49f));
	assert_false(obs_source_audio_input_balance_needed(false, SPEAKERS_STEREO, 0.50f));
	assert_false(obs_source_audio_input_balance_needed(false, SPEAKERS_STEREO, 0.51f));
	assert_false(obs_source_audio_input_balance_needed(false, SPEAKERS_STEREO, 0.495f));
	assert_false(obs_source_audio_input_balance_needed(false, SPEAKERS_STEREO, next_up(0.49f)));
	assert_false(obs_source_audio_input_balance_needed(false, SPEAKERS_STEREO, next_down(0.51f)));

	/* just outside */
	assert_true(obs_source_audio_input_balance_needed(false, SPEAKERS_STEREO, next_down(0.49f)));
	assert_true(obs_source_audio_input_balance_needed(false, SPEAKERS_STEREO, next_up(0.51f)));
	assert_true(obs_source_audio_input_balance_needed(false, SPEAKERS_STEREO, 0.25f));
	assert_true(obs_source_audio_input_balance_needed(false, SPEAKERS_STEREO, 0.75f));

	/* the ends of the slider */
	assert_true(obs_source_audio_input_balance_needed(false, SPEAKERS_STEREO, 0.0f));
	assert_true(obs_source_audio_input_balance_needed(false, SPEAKERS_STEREO, -0.0f));
	assert_true(obs_source_audio_input_balance_needed(false, SPEAKERS_STEREO, 1.0f));
}

/* Characterized, not endorsed: the compares skip a NaN, so it is not
 * balanced; values outside 0 to 1 (only the API can set them) are. */
static void balance_gate_outside_the_range(void **state)
{
	(void)state;

	assert_false(obs_source_audio_input_balance_needed(false, SPEAKERS_STEREO, NAN));
	assert_false(obs_source_audio_input_balance_needed(false, SPEAKERS_STEREO, -NAN));
	assert_false(obs_source_audio_input_balance_needed(false, SPEAKERS_STEREO, float_of(0x7f800001u)));
	assert_true(obs_source_audio_input_balance_needed(false, SPEAKERS_STEREO, -1.0f));
	assert_true(obs_source_audio_input_balance_needed(false, SPEAKERS_STEREO, 2.0f));
	assert_true(obs_source_audio_input_balance_needed(false, SPEAKERS_STEREO, INFINITY));
	assert_true(obs_source_audio_input_balance_needed(false, SPEAKERS_STEREO, -INFINITY));
}

static void balance_gate_needs_stereo_into_more_than_mono(void **state)
{
	(void)state;

	/* a mono output: never */
	assert_false(obs_source_audio_input_balance_needed(true, SPEAKERS_STEREO, 0.25f));
	assert_false(obs_source_audio_input_balance_needed(true, SPEAKERS_STEREO, 0.0f));
	assert_false(obs_source_audio_input_balance_needed(true, SPEAKERS_STEREO, 2.0f));

	/* only a stereo source */
	const int others[] = {0, SPEAKERS_MONO, 3, 4, 5, SPEAKERS_5POINT1, 8, -1, 7};
	for (size_t i = 0; i < sizeof(others) / sizeof(others[0]); i++) {
		assert_false(obs_source_audio_input_balance_needed(false, others[i], 0.25f));
		assert_false(obs_source_audio_input_balance_needed(false, others[i], 0.0f));
	}
}

static void mono_gate(void **state)
{
	(void)state;

	assert_false(obs_source_audio_input_mono_needed(false, 0));
	assert_true(obs_source_audio_input_mono_needed(false, FORCE_MONO));
	assert_true(obs_source_audio_input_mono_needed(false, UINT32_MAX));
	/* only the Force Mono bit counts */
	assert_false(obs_source_audio_input_mono_needed(false, 1u));
	assert_false(obs_source_audio_input_mono_needed(false, ~FORCE_MONO));
	assert_true(obs_source_audio_input_mono_needed(false, FORCE_MONO | 1u | (1u << 31)));
	/* a mono output: never */
	assert_false(obs_source_audio_input_mono_needed(true, FORCE_MONO));
	assert_false(obs_source_audio_input_mono_needed(true, UINT32_MAX));
	assert_false(obs_source_audio_input_mono_needed(true, 0));
}

/* ---- the balance laws ---- */

#define FRAMES 8
#define GUARD 2

static const float left_in[FRAMES] = {1.0f, -1.0f, 0.5f, 0.0f, -0.0f, 0.123456789f, 1e-40f, 3.0e38f};
static const float right_in[FRAMES] = {0.25f, -0.75f, 1.0f, -0.0f, 0.0f, -0.987654321f, -1e-40f, -3.0e38f};
#define GUARD_VALUE 42.0f

struct planes {
	float left[FRAMES + GUARD];
	float right[FRAMES + GUARD];
	float third[FRAMES + GUARD];
	float *data[3];
};

static void fill_planes(struct planes *p)
{
	for (size_t i = 0; i < FRAMES + GUARD; i++) {
		p->left[i] = i < FRAMES ? left_in[i] : GUARD_VALUE;
		p->right[i] = i < FRAMES ? right_in[i] : GUARD_VALUE;
		p->third[i] = GUARD_VALUE;
	}
	p->data[0] = p->left;
	p->data[1] = p->right;
	p->data[2] = p->third;
}

/* The two gains, with the C's mix of float and double. */
static float sine_left_gain(float balance)
{
	return sinf((float)((double)(1.0f - balance) * (M_PI / 2.0)));
}

static float sine_right_gain(float balance)
{
	return sinf((float)((double)balance * (M_PI / 2.0)));
}

static void check_balanced(const struct planes *p, float left_gain, float right_gain, uint32_t frames)
{
	for (size_t i = 0; i < FRAMES + GUARD; i++) {
		const bool done = i < frames;
		const float l = i < FRAMES ? left_in[i] : GUARD_VALUE;
		const float r = i < FRAMES ? right_in[i] : GUARD_VALUE;
		check_bits(p->left[i], done ? l * left_gain : l, "left", i);
		check_bits(p->right[i], done ? r * right_gain : r, "right", i);
		check_bits(p->third[i], GUARD_VALUE, "third", i);
	}
}

static void sine_law(void **state)
{
	(void)state;
	const float balances[] = {0.25f, 0.75f, 0.0f, 1.0f, 0.1f, 0.9f, 0.49f, 0.51f, float_of(0x3a917d6bu)};

	for (size_t i = 0; i < sizeof(balances) / sizeof(balances[0]); i++) {
		struct planes p;
		fill_planes(&p);
		obs_source_audio_input_balance(p.data, FRAMES, balances[i], SINE);
		check_balanced(&p, sine_left_gain(balances[i]), sine_right_gain(balances[i]), FRAMES);
	}

	/* fully left and fully right: the gains are exactly 1 and 0 */
	assert_int_equal(bits_of(sine_left_gain(0.0f)), bits_of(1.0f));
	assert_int_equal(bits_of(sine_right_gain(0.0f)), bits_of(0.0f));
	assert_int_equal(bits_of(sine_left_gain(1.0f)), bits_of(0.0f));
	assert_int_equal(bits_of(sine_right_gain(1.0f)), bits_of(1.0f));

	/* only the first frames */
	struct planes p;
	fill_planes(&p);
	obs_source_audio_input_balance(p.data, 3, 0.25f, SINE);
	check_balanced(&p, sine_left_gain(0.25f), sine_right_gain(0.25f), 3);
}

/* The gain goes through double: (double)(1.0f - balance) * (M_PI / 2.0),
 * then back to float for sinf. For this balance the float-only product gives
 * another argument. */
static void sine_law_keeps_the_double_product(void **state)
{
	(void)state;
	const float balance = float_of(0x3a917d6bu);
	const float through_double = (float)((double)(1.0f - balance) * (M_PI / 2.0));
	const float float_only = (1.0f - balance) * (float)(M_PI / 2.0);

	assert_int_not_equal(bits_of(through_double), bits_of(float_only));

	struct planes p;
	fill_planes(&p);
	obs_source_audio_input_balance(p.data, FRAMES, balance, SINE);
	check_bits(p.left[0], left_in[0] * sinf(through_double), "left", (size_t)0);
}

static void square_law(void **state)
{
	(void)state;
	const float balances[] = {0.25f, 0.75f, 0.0f, 1.0f, 0.1f};

	for (size_t i = 0; i < sizeof(balances) / sizeof(balances[0]); i++) {
		struct planes p;
		fill_planes(&p);
		obs_source_audio_input_balance(p.data, FRAMES, balances[i], SQUARE);
		check_balanced(&p, sqrtf(1.0f - balances[i]), sqrtf(balances[i]), FRAMES);
	}

	/* known samples: sqrtf is exact in IEEE 754 */
	struct planes p;
	fill_planes(&p);
	obs_source_audio_input_balance(p.data, FRAMES, 0.25f, SQUARE);
	check_bits(p.left[0], float_of(0x3f5db3d7u), "left", (size_t)0);   /* sqrt(0.75) */
	check_bits(p.right[0], float_of(0x3e000000u), "right", (size_t)0); /* 0.25 * 0.5 */
	check_bits(p.right[2], 0.5f, "right", (size_t)2);
}

static void linear_law(void **state)
{
	(void)state;
	const float balances[] = {0.25f, 0.75f, 0.0f, 1.0f, 0.1f};

	for (size_t i = 0; i < sizeof(balances) / sizeof(balances[0]); i++) {
		struct planes p;
		fill_planes(&p);
		obs_source_audio_input_balance(p.data, FRAMES, balances[i], LINEAR);
		check_balanced(&p, 1.0f - balances[i], balances[i], FRAMES);
	}

	/* known samples */
	struct planes p;
	fill_planes(&p);
	obs_source_audio_input_balance(p.data, FRAMES, 0.25f, LINEAR);
	check_bits(p.left[0], 0.75f, "left", (size_t)0);
	check_bits(p.left[1], -0.75f, "left", (size_t)1);
	check_bits(p.left[2], 0.375f, "left", (size_t)2);
	check_bits(p.right[0], 0.0625f, "right", (size_t)0);
	check_bits(p.right[2], 0.25f, "right", (size_t)2);
	/* -0.0 * 0.25 stays -0.0 */
	check_bits(p.right[3], -0.0f, "right", (size_t)3);
}

/* Characterized, not endorsed: out-of-range balances (only the API can set
 * them) give negative gains, gains above 1 and NaN. */
static void laws_outside_the_range(void **state)
{
	(void)state;
	const float balances[] = {-1.0f, 2.0f, INFINITY, -INFINITY};
	const int laws[] = {SINE, SQUARE, LINEAR};

	for (size_t l = 0; l < 3; l++) {
		for (size_t i = 0; i < sizeof(balances) / sizeof(balances[0]); i++) {
			const float b = balances[i];
			struct planes p;
			fill_planes(&p);
			obs_source_audio_input_balance(p.data, FRAMES, b, laws[l]);
			for (size_t f = 0; f < FRAMES; f++) {
				float lg, rg;
				if (laws[l] == SINE) {
					lg = sine_left_gain(b);
					rg = sine_right_gain(b);
				} else if (laws[l] == SQUARE) {
					lg = sqrtf(1.0f - b);
					rg = sqrtf(b);
				} else {
					lg = 1.0f - b;
					rg = b;
				}
				const float le = left_in[f] * lg, re = right_in[f] * rg;
				/* NaN payloads are the platform's: compare NaN as NaN */
				if (isnan(le))
					assert_true(isnan(p.left[f]));
				else
					check_bits(p.left[f], le, "left", f);
				if (isnan(re))
					assert_true(isnan(p.right[f]));
				else
					check_bits(p.right[f], re, "right", f);
			}
		}
	}

	/* the linear law at -1: left x2, right x-1 */
	struct planes p;
	fill_planes(&p);
	obs_source_audio_input_balance(p.data, FRAMES, -1.0f, LINEAR);
	check_bits(p.left[0], 2.0f, "left", (size_t)0);
	check_bits(p.right[0], -0.25f, "right", (size_t)0);
	/* the square law at 2: sqrtf(-1) is NaN on the left */
	fill_planes(&p);
	obs_source_audio_input_balance(p.data, FRAMES, 2.0f, SQUARE);
	assert_true(isnan(p.left[0]));
	check_bits(p.right[0], 0.25f * sqrtf(2.0f), "right", (size_t)0);
}

static void unknown_law_changes_nothing(void **state)
{
	(void)state;
	const int laws[] = {3, -1, 100, INT32_MIN, INT32_MAX};

	for (size_t i = 0; i < sizeof(laws) / sizeof(laws[0]); i++) {
		struct planes p;
		fill_planes(&p);
		obs_source_audio_input_balance(p.data, FRAMES, 0.25f, laws[i]);
		check_balanced(&p, 1.0f, 1.0f, 0);
		/* it reads nothing: not even the plane pointers */
		obs_source_audio_input_balance(NULL, FRAMES, 0.25f, laws[i]);
	}
}

static void balance_of_no_frames(void **state)
{
	(void)state;
	const int laws[] = {SINE, SQUARE, LINEAR, 3};

	for (size_t i = 0; i < sizeof(laws) / sizeof(laws[0]); i++) {
		struct planes p;
		fill_planes(&p);
		obs_source_audio_input_balance(p.data, 0, 0.25f, laws[i]);
		check_balanced(&p, 1.0f, 1.0f, 0);
		/* no frames read nothing: not even the plane pointers */
		obs_source_audio_input_balance(NULL, 0, 0.25f, laws[i]);
	}
}

/* ---- the downmix ---- */

#define MAX_CHANNELS 8

struct mix {
	float plane[MAX_CHANNELS][FRAMES + GUARD];
	float *data[MAX_CHANNELS];
};

/* The input samples: distinct, signed, some not exact in float. Stored, so
 * the expected sums below add the same floats the C does (no contraction
 * into a multiply-add). */
static float input[MAX_CHANNELS][FRAMES];

static float sample(size_t channel, size_t frame)
{
	return input[channel][frame];
}

static void fill_mix(struct mix *m)
{
	for (size_t c = 0; c < MAX_CHANNELS; c++) {
		for (size_t f = 0; f < FRAMES; f++) {
			const float magnitude = (float)(c + 1) * 0.1f - (float)f * 0.37f;
			input[c][f] = c % 2 ? -magnitude : magnitude;
		}
	}
	for (size_t c = 0; c < MAX_CHANNELS; c++) {
		for (size_t f = 0; f < FRAMES + GUARD; f++)
			m->plane[c][f] = f < FRAMES ? sample(c, f) : GUARD_VALUE;
		m->data[c] = m->plane[c];
	}
}

/* The C's sum: plane by plane into the first, then times 1 / channels. */
static float mean_of(size_t channels, size_t frame)
{
	float sum = sample(0, frame);
	for (size_t c = 1; c < channels; c++)
		sum += sample(c, frame);
	return sum * (1.0f / (float)channels);
}

static void check_downmix(const struct mix *m, size_t channels, uint32_t frames)
{
	for (size_t c = 0; c < MAX_CHANNELS; c++) {
		for (size_t f = 0; f < FRAMES + GUARD; f++) {
			const float in = f < FRAMES ? sample(c, f) : GUARD_VALUE;
			const bool done = c < channels && f < frames;
			check_bits(m->plane[c][f], done ? mean_of(channels, f) : in, "plane", c * 100 + f);
		}
	}
}

static void downmix_2_6_and_8_channels(void **state)
{
	(void)state;
	const size_t counts[] = {2, 6, 8};

	for (size_t i = 0; i < 3; i++) {
		struct mix m;
		fill_mix(&m);
		obs_source_audio_input_downmix(m.data, counts[i], FRAMES);
		check_downmix(&m, counts[i], FRAMES);
	}

	/* known samples */
	struct mix m;
	fill_mix(&m);
	m.plane[0][0] = 0.5f;
	m.plane[1][0] = 0.25f;
	obs_source_audio_input_downmix(m.data, 2, 1);
	check_bits(m.plane[0][0], 0.375f, "plane", (size_t)0);
	check_bits(m.plane[1][0], 0.375f, "plane", (size_t)100);

	/* only the first frames */
	fill_mix(&m);
	obs_source_audio_input_downmix(m.data, 6, 3);
	check_downmix(&m, 6, 3);
}

/* The planes are summed in order, into the first: ((1 + 1e8) - 1e8) is 0 in
 * float, where 1 + (1e8 - 1e8) would be 1. */
static void downmix_sums_in_plane_order(void **state)
{
	(void)state;
	struct mix m;
	fill_mix(&m);
	for (size_t c = 0; c < 6; c++)
		m.plane[c][0] = 0.0f;
	m.plane[0][0] = 1.0f;
	m.plane[1][0] = 1e8f;
	m.plane[2][0] = -1e8f;

	obs_source_audio_input_downmix(m.data, 6, 1);
	for (size_t c = 0; c < 6; c++)
		check_bits(m.plane[c][0], 0.0f, "plane", c * 100);
}

/* The sum is scaled by the float 1 / channels, not divided: for this sample
 * the two differ in the last bit. */
static void downmix_scales_by_the_reciprocal(void **state)
{
	(void)state;
	struct mix m;
	fill_mix(&m);
	for (size_t c = 0; c < 6; c++)
		m.plane[c][0] = 0.0f;
	m.plane[0][0] = float_of(0x3e124925u);

	obs_source_audio_input_downmix(m.data, 6, 1);
	assert_int_not_equal(bits_of(float_of(0x3e124925u) / 6.0f), 0x3cc30c32u);
	for (size_t c = 0; c < 6; c++)
		check_bits(m.plane[c][0], float_of(0x3cc30c32u), "plane", c * 100);
}

static void downmix_of_no_frames(void **state)
{
	(void)state;
	const size_t counts[] = {0, 1, 2, 6, 8};

	for (size_t i = 0; i < 5; i++) {
		struct mix m;
		fill_mix(&m);
		obs_source_audio_input_downmix(m.data, counts[i], 0);
		check_downmix(&m, counts[i], 0);
		/* no frames read nothing: not even the plane pointers */
		obs_source_audio_input_downmix(NULL, counts[i], 0);
	}
}

/* Characterized, not endorsed (the gate never gets here: libobs's output
 * has at least one channel, and one is a mono output): one channel scales
 * the plane by 1, and no channels scales the first plane by +inf. */
static void downmix_of_one_and_no_channels(void **state)
{
	(void)state;
	struct mix m;
	fill_mix(&m);
	obs_source_audio_input_downmix(m.data, 1, FRAMES);
	check_downmix(&m, 1, FRAMES);

	fill_mix(&m);
	m.plane[0][0] = 1.0f;
	m.plane[0][1] = -2.0f;
	m.plane[0][2] = 0.0f;
	obs_source_audio_input_downmix(m.data, 0, 3);
	check_bits(m.plane[0][0], INFINITY, "plane", (size_t)0);
	check_bits(m.plane[0][1], -INFINITY, "plane", (size_t)1);
	assert_true(isnan(m.plane[0][2]));
	for (size_t f = 3; f < FRAMES; f++)
		check_bits(m.plane[0][f], sample(0, f), "plane", f);
	for (size_t c = 1; c < MAX_CHANNELS; c++) {
		for (size_t f = 0; f < FRAMES; f++)
			check_bits(m.plane[c][f], sample(c, f), "plane", c * 100 + f);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(discontinuity_at_the_threshold),
		cmocka_unit_test(discontinuity_needs_a_resampler_and_an_end),
		cmocka_unit_test(discontinuity_distance_is_not_modular),
		cmocka_unit_test(reset_on_each_trigger_alone),
		cmocka_unit_test(reset_from_the_zeroed_source),
		cmocka_unit_test(resampler_needed_rule),
		cmocka_unit_test(balance_gate_at_the_edges),
		cmocka_unit_test(balance_gate_outside_the_range),
		cmocka_unit_test(balance_gate_needs_stereo_into_more_than_mono),
		cmocka_unit_test(mono_gate),
		cmocka_unit_test(sine_law),
		cmocka_unit_test(sine_law_keeps_the_double_product),
		cmocka_unit_test(square_law),
		cmocka_unit_test(linear_law),
		cmocka_unit_test(laws_outside_the_range),
		cmocka_unit_test(unknown_law_changes_nothing),
		cmocka_unit_test(balance_of_no_frames),
		cmocka_unit_test(downmix_2_6_and_8_channels),
		cmocka_unit_test(downmix_sums_in_plane_order),
		cmocka_unit_test(downmix_scales_by_the_reciprocal),
		cmocka_unit_test(downmix_of_no_frames),
		cmocka_unit_test(downmix_of_one_and_no_channels),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
