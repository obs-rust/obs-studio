#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <setjmp.h>
#include <cmocka.h>

#include "media-io/audio-resampler-drift.h"

/* Characterization of the libswresample resampler's decisions, moved from
 * libobs/media-io/audio-resampler-ffmpeg.c to
 * libobs/media-io/audio-resampler-drift.c: the sample format, the mono upmix
 * row, the clock drift compensation (the dead band, the loop filter, the
 * sample counters and the drift report) and the output frames. No swr
 * context: the test calls only the moved functions, which are hidden in
 * libobs, so this characterizes that C source. The doubles are compared
 * exactly, so the test is built with -ffp-contract=off, as the file is. */

#define MS 1000000LL

/* An action and the delta beside it, asked as audio_resampler_resample()
 * asks them: the delta only when the step is not idle (after the engage log
 * there); DELTA_UNSET otherwise. */
#define DELTA_UNSET 123456

struct step {
	enum audio_resampler_comp_action action;
	int delta;
};

static struct step step(struct audio_resampler_comp *c, uint32_t input_freq, uint32_t in_frames)
{
	struct step s = {AUDIO_RESAMPLER_COMP_IDLE, DELTA_UNSET};
	s.action = audio_resampler_comp_step(c, input_freq, in_frames);
	if (s.action != AUDIO_RESAMPLER_COMP_IDLE)
		s.delta = audio_resampler_comp_delta(c, input_freq);
	return s;
}

/* The drift report, asked as audio_resampler_destroy() asks it: the input
 * rate only when the gate is open; *rate is left alone otherwise. */
static bool drift_report_at(const struct audio_resampler_comp *c, uint32_t output_freq, double *rate)
{
	if (!audio_resampler_comp_reports_drift(c))
		return false;
	*rate = audio_resampler_comp_input_rate(c, output_freq);
	return true;
}

static void same_bits(double got, double want, const char *what, int at)
{
	if (memcmp(&got, &want, sizeof(got)) != 0) {
		fprintf(stderr, "%s at %d: %a, expected %a\n", what, at, got, want);
		fail();
	}
}

static void check_step(struct step got, enum audio_resampler_comp_action action, int delta, int at)
{
	if (got.action != action || got.delta != delta) {
		fprintf(stderr, "step %d: action %d delta %d, expected %d and %d\n", at, (int)got.action, got.delta,
			(int)action, delta);
		fail();
	}
}

static void constants(void **state)
{
	(void)state;
	assert_int_equal(AUDIO_RESAMPLER_COMP_DEAD_BAND_NS, 10000000);
	same_bits(AUDIO_RESAMPLER_COMP_MAX_PPM, 1000.0, "MAX_PPM", 0);
	assert_int_equal(AUDIO_RESAMPLER_COMP_DISTANCE, 16777216);
	same_bits(AUDIO_RESAMPLER_COMP_GAIN, 3.756e-2, "GAIN", 0);
	same_bits(AUDIO_RESAMPLER_COMP_C1, 8.25, "C1", 0);
	same_bits(AUDIO_RESAMPLER_COMP_C2, 107.1, "C2", 0);
	assert_int_equal(AUDIO_RESAMPLER_COMP_IDLE, 0);
	assert_int_equal(AUDIO_RESAMPLER_COMP_ENGAGE, 1);
	assert_int_equal(AUDIO_RESAMPLER_COMP_SET, 2);
	assert_int_equal(MAX_AUDIO_CHANNELS, 8);
}

/* Each audio format to its libavutil sample format; anything else is S16. */
static void sample_formats(void **state)
{
	(void)state;
	assert_int_equal(audio_resampler_sample_format(AUDIO_FORMAT_UNKNOWN), 1);
	assert_int_equal(audio_resampler_sample_format(AUDIO_FORMAT_U8BIT), 0);
	assert_int_equal(audio_resampler_sample_format(AUDIO_FORMAT_16BIT), 1);
	assert_int_equal(audio_resampler_sample_format(AUDIO_FORMAT_32BIT), 2);
	assert_int_equal(audio_resampler_sample_format(AUDIO_FORMAT_FLOAT), 3);
	assert_int_equal(audio_resampler_sample_format(AUDIO_FORMAT_U8BIT_PLANAR), 5);
	assert_int_equal(audio_resampler_sample_format(AUDIO_FORMAT_16BIT_PLANAR), 6);
	assert_int_equal(audio_resampler_sample_format(AUDIO_FORMAT_32BIT_PLANAR), 7);
	assert_int_equal(audio_resampler_sample_format(AUDIO_FORMAT_FLOAT_PLANAR), 8);

	/* characterized, not endorsed: one past the last format is S16 */
	assert_int_equal(audio_resampler_sample_format((enum audio_format)(AUDIO_FORMAT_FLOAT_PLANAR + 1)), 1);
	assert_int_equal(audio_resampler_sample_format((enum audio_format)100), 1);

	assert_int_equal(AUDIO_RESAMPLER_FMT_U8, 0);
	assert_int_equal(AUDIO_RESAMPLER_FMT_S16, 1);
	assert_int_equal(AUDIO_RESAMPLER_FMT_S32, 2);
	assert_int_equal(AUDIO_RESAMPLER_FMT_FLT, 3);
	assert_int_equal(AUDIO_RESAMPLER_FMT_U8P, 5);
	assert_int_equal(AUDIO_RESAMPLER_FMT_S16P, 6);
	assert_int_equal(AUDIO_RESAMPLER_FMT_S32P, 7);
	assert_int_equal(AUDIO_RESAMPLER_FMT_FLTP, 8);
}

/* A mono input to 1 to 8 channels: the front pair and the centre take the
 * signal, the LFE (the 4th channel of 3.1/5.1/7.1 layouts with one) doesn't. */
static void upmix_rows(void **state)
{
	(void)state;
	static const double want[MAX_AUDIO_CHANNELS][MAX_AUDIO_CHANNELS] = {
		{1, 0, 0, 0, 0, 0, 0, 0}, {1, 1, 0, 0, 0, 0, 0, 0}, {1, 1, 0, 0, 0, 0, 0, 0}, {1, 1, 1, 1, 0, 0, 0, 0},
		{1, 1, 1, 0, 1, 0, 0, 0}, {1, 1, 1, 1, 1, 1, 0, 0}, {1, 1, 1, 0, 1, 1, 1, 0}, {1, 1, 1, 0, 1, 1, 1, 1},
	};

	for (uint32_t ch = 1; ch <= MAX_AUDIO_CHANNELS; ch++) {
		double row[MAX_AUDIO_CHANNELS];
		for (size_t i = 0; i < MAX_AUDIO_CHANNELS; i++)
			row[i] = -1.0;
		assert_true(audio_resampler_upmix_row(ch, row));
		for (size_t i = 0; i < MAX_AUDIO_CHANNELS; i++)
			same_bits(row[i], want[ch - 1][i], "upmix row", (int)(ch * 10 + i));
	}

	/* no row for 0 channels or more than 8: nothing written */
	const uint32_t none[] = {0, MAX_AUDIO_CHANNELS + 1, 64, UINT32_MAX};
	for (size_t n = 0; n < sizeof(none) / sizeof(none[0]); n++) {
		double row[MAX_AUDIO_CHANNELS];
		for (size_t i = 0; i < MAX_AUDIO_CHANNELS; i++)
			row[i] = -1.0;
		assert_false(audio_resampler_upmix_row(none[n], row));
		for (size_t i = 0; i < MAX_AUDIO_CHANNELS; i++)
			same_bits(row[i], -1.0, "untouched row", (int)n);
	}
}

/* The first error turns compensation on and clears the filter; later ones
 * keep it. */
static void first_error_enables(void **state)
{
	(void)state;
	struct audio_resampler_comp c;
	memset(&c, 0, sizeof(c));
	c.vc1 = 5.0;
	c.vc2 = -3.0;
	c.input_samples = 7;
	c.output_samples = 9;

	audio_resampler_comp_set_error(&c, 0);
	assert_true(c.enabled);
	assert_false(c.engaged);
	same_bits(c.vc1, 0.0, "vc1 cleared", 0);
	same_bits(c.vc2, 0.0, "vc2 cleared", 0);
	assert_int_equal(c.input_samples, 7);
	assert_int_equal(c.output_samples, 9);

	c.vc1 = 5.0;
	c.vc2 = -3.0;
	audio_resampler_comp_set_error(&c, 20 * MS);
	same_bits(c.vc1, 5.0, "vc1 kept", 1);
	same_bits(c.vc2, -3.0, "vc2 kept", 1);
	assert_int_equal(c.error_ns, 10 * MS);
}

/* Within +-10 ms the error is 0; past it, the error less the band. */
static void dead_band(void **state)
{
	(void)state;
	const int64_t band = AUDIO_RESAMPLER_COMP_DEAD_BAND_NS;
	const int64_t cases[][2] = {
		{0, 0},
		{1, 0},
		{-1, 0},
		{band - 1, 0},
		{-(band - 1), 0},
		{band, 0},
		{-band, 0},
		{band + 1, 1},
		{-(band + 1), -1},
		{50 * MS, 40 * MS},
		{-50 * MS, -40 * MS},
		{INT64_MAX, INT64_MAX - band},
		{INT64_MIN, INT64_MIN + band},
	};

	for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++) {
		struct audio_resampler_comp c;
		memset(&c, 0, sizeof(c));
		c.error_ns = 777;
		audio_resampler_comp_set_error(&c, cases[i][0]);
		if (c.error_ns != cases[i][1]) {
			fprintf(stderr, "error %lld: %lld, expected %lld\n", (long long)cases[i][0],
				(long long)c.error_ns, (long long)cases[i][1]);
			fail();
		}
	}
}

/* An error that stays 0 never engages: no delta, the filter stays at rest. */
static void zero_error_never_engages(void **state)
{
	(void)state;
	struct audio_resampler_comp c;
	memset(&c, 0, sizeof(c));
	audio_resampler_comp_set_error(&c, 5 * MS);
	for (int i = 0; i < 10000; i++)
		check_step(step(&c, 48000, 480), AUDIO_RESAMPLER_COMP_IDLE, DELTA_UNSET, i);
	assert_false(c.engaged);
	same_bits(c.vc1, 0.0, "vc1", 0);
	same_bits(c.vc2, 0.0, "vc2", 0);
}

struct checkpoint {
	int at;
	int delta;
	double vc1;
	double vc2;
};

/* 10,000 steps of 480 frames at 48 kHz with a constant error: it engages on
 * the first step, the correction grows to the 1000 ppm clamp (48 samples per
 * second, a delta of 16,777 per 2^24) at step 880 and stays there while the
 * second state keeps integrating. Every delta goes into an FNV-1a hash. */
static void constant_error_run(int64_t error_ns, const struct checkpoint *points, size_t n_points, long long want_sum,
			       uint64_t want_hash)
{
	struct audio_resampler_comp c;
	memset(&c, 0, sizeof(c));
	audio_resampler_comp_set_error(&c, error_ns);

	uint64_t hash = 1469598103934665603ULL;
	long long sum = 0;
	int clamp_at = -1;
	size_t next = 0;
	for (int i = 0; i < 10000; i++) {
		struct step s = step(&c, 48000, 480);
		assert_int_equal(s.action, i == 0 ? AUDIO_RESAMPLER_COMP_ENGAGE : AUDIO_RESAMPLER_COMP_SET);
		sum += s.delta;
		hash = (hash ^ (uint32_t)s.delta) * 1099511628211ULL;
		if (clamp_at < 0 && (c.vc1 == 48.0 || c.vc1 == -48.0))
			clamp_at = i;
		if (next < n_points && points[next].at == i) {
			check_step(s, i == 0 ? AUDIO_RESAMPLER_COMP_ENGAGE : AUDIO_RESAMPLER_COMP_SET,
				   points[next].delta, i);
			same_bits(c.vc1, points[next].vc1, "vc1", i);
			same_bits(c.vc2, points[next].vc2, "vc2", i);
			next++;
		}
	}
	assert_true(c.engaged);
	assert_int_equal(next, n_points);
	assert_int_equal(clamp_at, 880);
	assert_int_equal(sum, want_sum);
	assert_int_equal(hash, want_hash);
}

static void constant_positive_error(void **state)
{
	(void)state;
	static const struct checkpoint points[] = {
		{0, -31, 0x1.660a81b1590dbp-4, 0x0p+0},
		{1, -61, 0x1.65d2f4bc91025p-3, 0x1.11dcea6553d84p-17},
		{2, -92, 0x1.0c34971e33e22p-2, 0x1.9a9d9c2fac973p-16},
		{3, -122, 0x1.6563fff15b5d6p-2, 0x1.9a6fe06d09508p-15},
		{10, -334, 0x1.e9559c012e8ep-1, 0x1.d4dcfe9941a86p-12},
		{100, -2907, 0x1.0a1bf29233af3p+3, 0x1.438e4a3cc7684p-5},
		{500, -11531, 0x1.07ed687373662p+5, 0x1.aa0e12d3ec92ep-1},
		{1000, -16777, 0x1.8p+5, 0x1.5f39337ed4d1p+1},
		{2000, -16777, 0x1.8p+5, 0x1.b1d1c6a0b7072p+2},
		{5000, -16777, 0x1.8p+5, 0x1.0d96c056b1acdp+4},
		{9999, -16777, 0x1.8p+5, 0x1.c77ce15cb794fp+4},
	};
	constant_error_run(50 * MS, points, sizeof(points) / sizeof(points[0]), -161617939LL, 0x96bfc7d0d527cf2aULL);
}

static void constant_negative_error(void **state)
{
	(void)state;
	static const struct checkpoint points[] = {
		{0, 31, -0x1.660a81b1590dbp-4, 0x0p+0},
		{1, 61, -0x1.65d2f4bc91025p-3, -0x1.11dcea6553d84p-17},
		{2, 92, -0x1.0c34971e33e22p-2, -0x1.9a9d9c2fac973p-16},
		{3, 122, -0x1.6563fff15b5d6p-2, -0x1.9a6fe06d09508p-15},
		{10, 334, -0x1.e9559c012e8ep-1, -0x1.d4dcfe9941a86p-12},
		{100, 2907, -0x1.0a1bf29233af3p+3, -0x1.438e4a3cc7684p-5},
		{500, 11531, -0x1.07ed687373662p+5, -0x1.aa0e12d3ec92ep-1},
		{1000, 16777, -0x1.8p+5, -0x1.5f39337ed4d1p+1},
		{2000, 16777, -0x1.8p+5, -0x1.b1d1c6a0b7072p+2},
		{5000, 16777, -0x1.8p+5, -0x1.0d96c056b1acdp+4},
		{9999, 16777, -0x1.8p+5, -0x1.c77ce15cb794fp+4},
	};
	constant_error_run(-50 * MS, points, sizeof(points) / sizeof(points[0]), 161617939LL, 0xa7e0e03c5ad74c7cULL);
}

/* Steps of 1024 frames reach the clamp sooner, at step 412. */
static void larger_steps(void **state)
{
	(void)state;
	struct audio_resampler_comp c;
	memset(&c, 0, sizeof(c));
	audio_resampler_comp_set_error(&c, 50 * MS);
	int clamp_at = -1;
	uint64_t hash = 1469598103934665603ULL;
	for (int i = 0; i < 10000; i++) {
		struct step s = step(&c, 48000, 1024);
		hash = (hash ^ (uint32_t)s.delta) * 1099511628211ULL;
		if (i == 0)
			check_step(s, AUDIO_RESAMPLER_COMP_ENGAGE, -65, i);
		if (i == 100) {
			check_step(s, AUDIO_RESAMPLER_COMP_SET, -5805, i);
			same_bits(c.vc1, 0x1.09bce35ad5489p+4, "vc1", i);
			same_bits(c.vc2, 0x1.5f25528e90579p-3, "vc2", i);
		}
		if (clamp_at < 0 && c.vc1 == 48.0)
			clamp_at = i;
	}
	assert_int_equal(clamp_at, 412);
	same_bits(c.vc2, 0x1.49c5f876d8e6p+5, "vc2", 9999);
	assert_int_equal(hash, 0x80b12b6647e3dd07ULL);
}

/* An error just past the band at 44.1 kHz engages, but 10,000 steps of it
 * never make a delta of even one sample per 2^24. */
static void tiny_error(void **state)
{
	(void)state;
	struct audio_resampler_comp c;
	memset(&c, 0, sizeof(c));
	audio_resampler_comp_set_error(&c, AUDIO_RESAMPLER_COMP_DEAD_BAND_NS + 1);
	for (int i = 0; i < 10000; i++)
		check_step(step(&c, 44100, 441), i == 0 ? AUDIO_RESAMPLER_COMP_ENGAGE : AUDIO_RESAMPLER_COMP_SET, 0, i);
	same_bits(c.vc1, 0x1.8063002b97c4bp-19, "vc1", 9999);
	same_bits(c.vc2, 0x1.63f09bac9c1dep-20, "vc2", 9999);
}

/* The filter runs while the error is 0, but nothing is set until the error
 * leaves the band; it engages then, once. */
static void engages_late(void **state)
{
	(void)state;
	struct audio_resampler_comp c;
	memset(&c, 0, sizeof(c));
	audio_resampler_comp_set_error(&c, 5 * MS);
	for (int i = 0; i < 100; i++)
		check_step(step(&c, 48000, 1024), AUDIO_RESAMPLER_COMP_IDLE, DELTA_UNSET, i);
	audio_resampler_comp_set_error(&c, 30 * MS);
	check_step(step(&c, 48000, 1024), AUDIO_RESAMPLER_COMP_ENGAGE, -33, 100);
	same_bits(c.vc1, 0x1.7de912df4dec7p-4, "vc1", 100);
	check_step(step(&c, 48000, 1024), AUDIO_RESAMPLER_COMP_SET, -65, 101);
	same_bits(c.vc2, 0x1.37985dbf84f16p-16, "vc2", 101);
	struct step s = {AUDIO_RESAMPLER_COMP_IDLE, DELTA_UNSET};
	for (int i = 102; i < 300; i++)
		s = step(&c, 48000, 1024);
	check_step(s, AUDIO_RESAMPLER_COMP_SET, -5111, 299);
	same_bits(c.vc1, 0x1.d3eff6fece1fbp+3, "vc1", 299);
	same_bits(c.vc2, 0x1.3d7c0afc0d9dap-2, "vc2", 299);
}

/* Once engaged it stays engaged: an error back in the band still sets a delta
 * every step, and the correction decays from where it was. */
static void stays_engaged(void **state)
{
	(void)state;
	struct audio_resampler_comp c;
	memset(&c, 0, sizeof(c));
	audio_resampler_comp_set_error(&c, 50 * MS);
	uint64_t hash = 1469598103934665603ULL;
	for (int i = 0; i < 3000; i++) {
		if (i == 200)
			audio_resampler_comp_set_error(&c, 0);
		struct step s = step(&c, 48000, 480);
		hash = (hash ^ (uint32_t)s.delta) * 1099511628211ULL;
		if (i > 0)
			assert_int_equal(s.action, AUDIO_RESAMPLER_COMP_SET);
		if (i == 199)
			check_step(s, AUDIO_RESAMPLER_COMP_SET, -5433, i);
		if (i == 200)
			check_step(s, AUDIO_RESAMPLER_COMP_SET, -5427, i);
		if (i == 1000) {
			check_step(s, AUDIO_RESAMPLER_COMP_SET, -2192, i);
			same_bits(c.vc1, 0x1.91510ee1dfb19p+2, "vc1", i);
			same_bits(c.vc2, 0x1.ba31e0cfb7284p-1, "vc2", i);
		}
		if (i == 2999)
			check_step(s, AUDIO_RESAMPLER_COMP_SET, -566, i);
	}
	assert_true(c.engaged);
	assert_int_equal(c.error_ns, 0);
	assert_int_equal(hash, 0x2c65bfa850c59e6bULL);
}

/* Zero frames move nothing, but an error out of the band still engages. */
static void zero_frames(void **state)
{
	(void)state;
	struct audio_resampler_comp c;
	memset(&c, 0, sizeof(c));
	audio_resampler_comp_set_error(&c, 50 * MS);
	check_step(step(&c, 48000, 0), AUDIO_RESAMPLER_COMP_ENGAGE, 0, 0);
	same_bits(c.vc1, 0.0, "vc1", 0);
	same_bits(c.vc2, 0.0, "vc2", 0);
}

/* The delta is lround() of the correction per 2^24 samples, negated: half
 * away from zero. At 2^24 Hz the delta is the correction itself. */
static void delta_rounding(void **state)
{
	(void)state;
	const double cases[][2] = {
		{2.5, -3}, {-2.5, 3}, {0.5, -1}, {-0.5, 1}, {1.5, -2}, {2.4999, -2}, {0.0, 0}, {48.0, -48},
	};

	for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++) {
		struct audio_resampler_comp c;
		memset(&c, 0, sizeof(c));
		c.vc1 = cases[i][0];
		assert_int_equal(audio_resampler_comp_delta(&c, 16777216), (int)cases[i][1]);
	}
	/* at 48 kHz, the 1000 ppm clamp */
	struct audio_resampler_comp c;
	memset(&c, 0, sizeof(c));
	c.vc1 = 48.0;
	assert_int_equal(audio_resampler_comp_delta(&c, 48000), -16777);
}

/* Room for the samples compensation adds, only while engaged. */
static void output_frames(void **state)
{
	(void)state;
	const int cases[][2] = {
		{0, 2},       {1, 3},       {511, 513},     {512, 515},
		{1023, 1026}, {1024, 1028}, {48000, 48095}, {1 << 20, (1 << 20) + 2050},
	};

	for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++) {
		assert_int_equal(audio_resampler_output_frames(cases[i][0], false), cases[i][0]);
		assert_int_equal(audio_resampler_output_frames(cases[i][0], true), cases[i][1]);
	}
	/* characterized, not endorsed: a negative estimate (av_rescale_rnd's
	 * INT64_MIN on overflow, cast) is padded toward 0 */
	assert_int_equal(audio_resampler_output_frames(-1024, true), -1024);
	assert_int_equal(audio_resampler_output_frames(-1, true), 1);
}

/* The sample counters count only while engaged. */
static void counters(void **state)
{
	(void)state;
	struct audio_resampler_comp c;
	memset(&c, 0, sizeof(c));
	c.enabled = true;
	audio_resampler_comp_count(&c, 480, 470);
	assert_int_equal(c.input_samples, 0);
	assert_int_equal(c.output_samples, 0);

	c.engaged = true;
	audio_resampler_comp_count(&c, 480, 470);
	audio_resampler_comp_count(&c, 1024, 0);
	assert_int_equal(c.input_samples, 1504);
	assert_int_equal(c.output_samples, 470);

	c.input_samples = UINT64_MAX;
	audio_resampler_comp_count(&c, 2, 1);
	assert_int_equal(c.input_samples, 1);
	assert_int_equal(c.output_samples, 471);
}

/* The drift report: only engaged, with output counted; the input rate the
 * counts give at the output rate. */
static void drift_report(void **state)
{
	(void)state;
	struct audio_resampler_comp c;
	double rate = -1.0;
	memset(&c, 0, sizeof(c));

	c.input_samples = 48000;
	c.output_samples = 48000;
	assert_false(drift_report_at(&c, 48000, &rate));
	same_bits(rate, -1.0, "rate untouched", 0);

	c.engaged = true;
	c.output_samples = 0;
	assert_false(drift_report_at(&c, 48000, &rate));
	same_bits(rate, -1.0, "rate untouched", 1);

	c.output_samples = 48000;
	assert_true(drift_report_at(&c, 48000, &rate));
	same_bits(rate, 48000.0, "same rate", 2);

	c.input_samples = 48048;
	assert_true(drift_report_at(&c, 48000, &rate));
	same_bits(rate, (double)48048 / (double)48000 * 48000, "fast device", 3);
	/* divided first, so a hair under 48048 */
	same_bits(rate, 0x1.775ffffffffffp+15, "fast device", 3);

	c.input_samples = 0;
	assert_true(drift_report_at(&c, 44100, &rate));
	same_bits(rate, 0.0, "no input", 4);

	c.input_samples = 44101;
	c.output_samples = 48000;
	assert_true(drift_report_at(&c, 48000, &rate));
	same_bits(rate, (double)44101 / (double)48000 * 48000, "rate change", 5);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(constants),
		cmocka_unit_test(sample_formats),
		cmocka_unit_test(upmix_rows),
		cmocka_unit_test(first_error_enables),
		cmocka_unit_test(dead_band),
		cmocka_unit_test(zero_error_never_engages),
		cmocka_unit_test(constant_positive_error),
		cmocka_unit_test(constant_negative_error),
		cmocka_unit_test(larger_steps),
		cmocka_unit_test(tiny_error),
		cmocka_unit_test(engages_late),
		cmocka_unit_test(stays_engaged),
		cmocka_unit_test(zero_frames),
		cmocka_unit_test(delta_rounding),
		cmocka_unit_test(output_frames),
		cmocka_unit_test(counters),
		cmocka_unit_test(drift_report),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
