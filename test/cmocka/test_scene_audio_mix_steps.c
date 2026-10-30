#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <math.h>
#include <stdio.h>
#include <string.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-scene-audio-mix.h"

/*
 * Characterization of the two mix kernels of scene_audio_render() in
 * libobs/obs-scene.c, moved to libobs/obs-scene-audio-mix.c before their Rust
 * port (#3079):
 * - obs_scene_audio_mix_with_buf(): out[pos + i] += in[i] * buf[i], for an
 *   item whose visibility change falls in the tick (buf is its 0/1 gain);
 * - obs_scene_audio_mix_add(): out[pos + i] += in[i].
 * They pin the current C behaviour.
 *
 * The functions are hidden in libobs, so this test compiles the C source into
 * itself and calls only them, through their header. scene_audio_render()
 * keeps the item, mix and channel loops; test_scene_audio_actions runs the
 * kernels through the real libobs.
 *
 * Every product here is exact (gains of 0, 1, 0.5, 2 and -1, or a special
 * operand), so the expected values hold whether or not the compiler fuses
 * the multiply and the add (clang does on aarch64 by default).
 */

#define FRAMES 1024
/* Room before and after the window, which must keep its values. */
#define GUARD 16
#define LEN (GUARD + FRAMES + GUARD)

static bool same_bits(float a, float b)
{
	uint32_t x, y;
	memcpy(&x, &a, sizeof(x));
	memcpy(&y, &b, sizeof(y));
	return x == y;
}

static void check_floats(const char *what, const float *got, const float *want, size_t n)
{
	for (size_t i = 0; i < n; i++) {
		if (!same_bits(got[i], want[i])) {
			fprintf(stderr, "%s: sample %zu is %a, expected %a\n", what, i, (double)got[i],
				(double)want[i]);
			fail();
		}
	}
}

/* A value per position that is exact in float and differs everywhere. */
static float base(size_t i)
{
	return (float)((int)(i % 97) - 48) / 64.0f;
}

static void fill(float *out, float (*f)(size_t), size_t n)
{
	for (size_t i = 0; i < n; i++)
		out[i] = f(i);
}

static float in_value(size_t i)
{
	return (float)((int)(i % 31) - 15) / 8.0f;
}

/* ------------------------------------------------------------------------ */

/* The plain add at the start of the tick, inside it and at its end, for
 * counts 0, 1, 3 and a whole tick: only out[pos .. pos + count) changes. */
static void test_add_windows(void **state)
{
	(void)state;

	static const size_t cases[][2] = {
		{0, 0}, {0, 1}, {0, 3}, {0, FRAMES}, {300, 0}, {300, 1}, {300, 724}, {FRAMES - 1, 1}, {FRAMES, 0},
	};
	float out[LEN], want[LEN], in[FRAMES];
	fill(in, in_value, FRAMES);

	for (size_t c = 0; c < sizeof(cases) / sizeof(cases[0]); c++) {
		const size_t pos = cases[c][0], count = cases[c][1];
		fill(out, base, LEN);
		fill(want, base, LEN);
		for (size_t i = 0; i < count; i++)
			want[GUARD + pos + i] += in[i];

		obs_scene_audio_mix_add(out + GUARD, in, pos, count);
		check_floats("add", out, want, LEN);
	}
}

/* The add with the gain buffer: a gain of 1 adds the sample, 0 adds
 * nothing (+0.0 to the output, so -0.0 becomes +0.0), and other exact
 * gains scale it. The gain is indexed from the start of the item's buffer,
 * not from pos. */
static void test_with_buf_gains(void **state)
{
	(void)state;

	static const float gains[] = {1.0f, 0.0f, 0.5f, 2.0f, -1.0f};
	float out[LEN], want[LEN], in[FRAMES], buf[FRAMES];
	fill(in, in_value, FRAMES);

	for (size_t g = 0; g < sizeof(gains) / sizeof(gains[0]); g++) {
		for (size_t pos = 0; pos <= 300; pos += 300) {
			const size_t count = FRAMES - pos;
			for (size_t i = 0; i < FRAMES; i++)
				buf[i] = (i % 2) ? gains[g] : 1.0f;
			fill(out, base, LEN);
			fill(want, base, LEN);
			for (size_t i = 0; i < count; i++)
				want[GUARD + pos + i] += in[i] * buf[i];

			obs_scene_audio_mix_with_buf(out + GUARD, in, buf, pos, count);
			check_floats("with_buf", out, want, LEN);
		}
	}
}

/* A hide in the middle of the tick: the gain steps from 1 to 0 at frame
 * 512 of the item's buffer, so the item is heard up to it and not after.
 * With the item's audio from pos 301 on, the step is still at the item's
 * frame 512 (output sample 813): the gain is indexed like the input. */
static void test_with_buf_step(void **state)
{
	(void)state;

	float out[LEN], want[LEN], in[FRAMES], buf[FRAMES];
	fill(in, in_value, FRAMES);
	for (size_t i = 0; i < FRAMES; i++)
		buf[i] = i < 512 ? 1.0f : 0.0f;

	for (size_t pos = 0; pos <= 301; pos += 301) {
		const size_t count = FRAMES - pos;
		fill(out, base, LEN);
		fill(want, base, LEN);
		for (size_t i = 0; i < count; i++)
			want[GUARD + pos + i] = base(GUARD + pos + i) + (i < 512 ? in[i] : 0.0f);

		obs_scene_audio_mix_with_buf(out + GUARD, in, buf, pos, count);
		check_floats("step", out, want, LEN);
	}
}

/* NaN, infinities and -0.0 follow IEEE addition and multiplication: a NaN in
 * any operand makes the sample NaN, inf + -inf and inf * 0 are NaN (a hidden
 * item with an infinite sample still poisons the mix), and -0.0 + -0.0 stays
 * -0.0. */
static void test_specials(void **state)
{
	(void)state;

	static const float specials[] = {0.0f, -0.0f, 1.0f, -1.0f, INFINITY, -INFINITY, NAN};
	enum { N = sizeof(specials) / sizeof(specials[0]) };

	for (size_t a = 0; a < N; a++) {
		for (size_t b = 0; b < N; b++) {
			for (size_t c = 0; c < N; c++) {
				float out = specials[a], in = specials[b], buf = specials[c];
				float want = specials[a] + specials[b] * specials[c];
				obs_scene_audio_mix_with_buf(&out, &in, &buf, 0, 1);
				if (isnan(want) ? !isnan(out) : !same_bits(out, want)) {
					fprintf(stderr, "with_buf %a + %a * %a = %a, expected %a\n",
						(double)specials[a], (double)specials[b], (double)specials[c],
						(double)out, (double)want);
					fail();
				}
			}
			float out = specials[a], in = specials[b];
			float want = specials[a] + specials[b];
			obs_scene_audio_mix_add(&out, &in, 0, 1);
			if (isnan(want) ? !isnan(out) : !same_bits(out, want)) {
				fprintf(stderr, "add %a + %a = %a, expected %a\n", (double)specials[a],
					(double)specials[b], (double)out, (double)want);
				fail();
			}
		}
	}
}

/* A count of 0 writes nothing, at any pos. */
static void test_count_zero(void **state)
{
	(void)state;

	float out[LEN], want[LEN], in[FRAMES], buf[FRAMES];
	fill(in, in_value, FRAMES);
	fill(buf, in_value, FRAMES);
	fill(out, base, LEN);
	fill(want, base, LEN);

	obs_scene_audio_mix_add(out + GUARD, in, 0, 0);
	obs_scene_audio_mix_with_buf(out + GUARD, in, buf, 500, 0);
	obs_scene_audio_mix_with_buf(out + GUARD, in, buf, FRAMES, 0);
	check_floats("count 0", out, want, LEN);
}

/* Mixing the same item twice adds it twice (the C has no state). */
static void test_twice(void **state)
{
	(void)state;

	float out[LEN], want[LEN], in[FRAMES];
	fill(in, in_value, FRAMES);
	fill(out, base, LEN);
	fill(want, base, LEN);
	for (size_t i = 0; i < FRAMES; i++)
		want[GUARD + i] = (want[GUARD + i] + in[i]) + in[i];

	obs_scene_audio_mix_add(out + GUARD, in, 0, FRAMES);
	obs_scene_audio_mix_add(out + GUARD, in, 0, FRAMES);
	check_floats("twice", out, want, LEN);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_add_windows),   cmocka_unit_test(test_with_buf_gains),
		cmocka_unit_test(test_with_buf_step), cmocka_unit_test(test_specials),
		cmocka_unit_test(test_count_zero),    cmocka_unit_test(test_twice),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
