/* SPDX-License-Identifier: GPL-2.0-or-later */
/* The compressor, limiter and expander gain stages stay bit-identical to the
 * per-sample db_to_mul(fminf(0, slope * (threshold - mul_to_db(env))))
 * formula, including the envelopes right at the threshold that a fast
 * below-threshold path must not decide (#3190). */
#include <math.h>
#include <stdio.h>
#include <string.h>

#include "../../plugins/obs-filters/compressor-filter.c.h"
#include "../../plugins/obs-filters/limiter-filter.c.h"
#include "../../plugins/obs-filters/expander-filter.c.h"

const char *obs_module_text(const char *text)
{
	return text;
}

#define FRAMES 4096

static unsigned checks;
static unsigned failures;

static void check(bool passed, const char *test, float env, float threshold)
{
	++checks;
	if (!passed) {
		if (failures < 8)
			fprintf(stderr, "%s: env %.9g threshold %.9g differs from the formula\n", test, env,
				threshold);
		++failures;
	}
}

static bool same_float(float a, float b)
{
	return memcmp(&a, &b, sizeof(a)) == 0;
}

static float formula_gain(float slope, float threshold, float env)
{
	return db_to_mul(fminf(0, slope * (threshold - mul_to_db(env))));
}

/* Envelopes from silence to clipping, dense within +-0.01 dB of the
 * threshold, plus the exact threshold and its float neighbours. */
static void fill_envelopes(float *env, float threshold)
{
	const float at = db_to_mul(threshold);
	size_t i = 0;
	env[i++] = 0.0f;
	env[i++] = at;
	env[i++] = nextafterf(at, 0.0f);
	env[i++] = nextafterf(at, 2.0f);
	env[i++] = at * 0.9999f;
	env[i++] = nextafterf(at * 0.9999f, 0.0f);
	env[i++] = nextafterf(at * 0.9999f, 2.0f);
	while (i < FRAMES / 2) {
		const float db = -140.0f + 146.0f * (float)i / (float)(FRAMES / 2);
		env[i++] = db_to_mul(db);
	}
	while (i < FRAMES) {
		const float db = threshold - 0.01f + 0.02f * (float)(i - FRAMES / 2) / (float)(FRAMES / 2);
		env[i++] = db_to_mul(db);
	}
}

static void compressor_and_limiter_match_formula(void)
{
	static float env[FRAMES];
	static float data[2][FRAMES];
	const float ratios[] = {1.0f, 1.5f, 4.0f, 32.0f};
	const float output_gain = 0.75f;

	for (int t = -60; t <= 0; ++t) {
		const float threshold = (float)t + 0.37f * (t & 1);
		fill_envelopes(env, threshold);

		for (size_t r = 0; r < sizeof(ratios) / sizeof(ratios[0]); ++r) {
			struct compressor_data cd = {0};
			cd.num_channels = 2;
			cd.threshold = threshold;
			cd.slope = 1.0f - (1.0f / ratios[r]);
			cd.output_gain = output_gain;
			cd.envelope_buf = env;
			float *samples[2] = {data[0], data[1]};
			for (size_t i = 0; i < FRAMES; ++i)
				data[0][i] = data[1][i] = 0.5f;
			process_compression(&cd, samples, FRAMES);
			for (size_t i = 0; i < FRAMES; ++i) {
				const float want = 0.5f * (formula_gain(cd.slope, threshold, env[i]) * output_gain);
				check(same_float(data[0][i], want) && same_float(data[1][i], want), "compressor",
				      env[i], threshold);
			}
		}

		struct limiter_data ld = {0};
		ld.num_channels = 2;
		ld.threshold = threshold;
		ld.slope = 1.0f;
		ld.output_gain = output_gain;
		ld.envelope_buf = env;
		float *samples[2] = {data[0], data[1]};
		for (size_t i = 0; i < FRAMES; ++i)
			data[0][i] = data[1][i] = -0.5f;
		limiter_process_compression(&ld, samples, FRAMES);
		for (size_t i = 0; i < FRAMES; ++i) {
			const float want = -0.5f * (formula_gain(1.0f, threshold, env[i]) * output_gain);
			check(same_float(data[0][i], want) && same_float(data[1][i], want), "limiter", env[i],
			      threshold);
		}
	}
}

/* With attack and release coefficients of 0 the expander's gain_db is the
 * target gain itself, so its output stage can be compared directly. */
static void expander_output_stage_matches_formula(void)
{
	static float env[FRAMES];
	const float slopes[] = {0.0f, -1.0f, -3.0f};

	for (int t = -60; t <= 0; t += 3) {
		const float threshold = (float)t;
		fill_envelopes(env, threshold);
		for (size_t s = 0; s < sizeof(slopes) / sizeof(slopes[0]); ++s) {
			for (size_t i = 0; i < FRAMES; ++i) {
				float sample = 0.25f;
				float gain_db = 0.0f;
				process_sample(0, &sample, &env[i], &gain_db, false, 0.0f, threshold, slopes[s], 0.0f,
					       1.0f, 0.0f, 1.0f, 0.5f, 0.0f);
				const float want = 0.25f * (db_to_mul(fminf(0, gain_db)) * 0.5f);
				check(same_float(sample, want), "expander", env[i], threshold);
			}
		}
	}
}

int main(void)
{
	compressor_and_limiter_match_formula();
	expander_output_stage_matches_formula();
	printf("dynamics threshold: checks=%u failures=%u\n", checks, failures);
	return failures ? 1 : 0;
}
