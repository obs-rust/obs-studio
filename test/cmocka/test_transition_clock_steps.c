#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <math.h>
#include <setjmp.h>
#include <cmocka.h>

#include "graphics/math-extra.h"
#include "obs-source-transition-clock.h"

/*
 * Characterization of the clock and fit decisions of the transitions in
 * libobs/obs-source-transition.c, moved to
 * libobs/obs-source-transition-clock.c before their Rust port:
 * - obs_source_transition_clock_time(): calc_time(), the manual value, 0 up
 *   to the start, 1 from the end (and for a duration of 0), the ratio in
 *   between;
 * - obs_source_transition_clock_manual(): the manual step of
 *   obs_transition_tick(), which snaps for a torque of 0 and otherwise steps
 *   with calc_torquef();
 * - obs_source_transition_clock_size() and _fold(): get_cx(), get_cy() and
 *   the size fold of recalculate_transition_size();
 * - obs_source_transition_clock_fit(): the scale of
 *   recalculate_transition_matrix() for max-only, aspect and stretch;
 * - obs_source_transition_clock_mix_spaces(): mix_spaces();
 * - obs_source_transition_clock_min_ts(): transition_calc_min_ts().
 * Cases marked "characterized, not endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them, and calc_torquef() (EXPORT) for the
 * manual step as obs_transition_tick() runs it. test_transition_swap,
 * test_studio_mode_swap, test_transition_recursive_destination,
 * test_fade_to_color_mix, test_transition_audio_rescale and
 * test_stinger_media_duration cover the callers through the real libobs.
 *
 * Every pinned float is chosen to be the same on every platform.
 * calc_time() divides in long double, which is 64 bits on MSVC and Apple
 * silicon, 80 (x87) on x86_64 and 128 on aarch64 Linux, and the widths
 * round a quotient that lies very near a float rounding tie differently;
 * the pinned ratios are exact or far from a tie, so all three give the same
 * float (checked for each). The manual steps use products that are exact,
 * so a fused multiply-add gives the same result.
 */

#define MANUAL OBS_SOURCE_TRANSITION_CLOCK_MODE_MANUAL
#define AUTO 0 /* OBS_TRANSITION_MODE_AUTO */

#define MAX_ONLY OBS_SOURCE_TRANSITION_CLOCK_SCALE_MAX_ONLY
#define ASPECT OBS_SOURCE_TRANSITION_CLOCK_SCALE_ASPECT
#define STRETCH OBS_SOURCE_TRANSITION_CLOCK_SCALE_STRETCH

#define SRGB OBS_SOURCE_TRANSITION_CLOCK_CS_SRGB
#define SRGB_16F OBS_SOURCE_TRANSITION_CLOCK_CS_SRGB_16F
#define EXTENDED OBS_SOURCE_TRANSITION_CLOCK_CS_709_EXTENDED
#define SCRGB OBS_SOURCE_TRANSITION_CLOCK_CS_709_SCRGB

#define HOLD OBS_SOURCE_TRANSITION_CLOCK_HOLD
#define SNAP OBS_SOURCE_TRANSITION_CLOCK_SNAP
#define STEP OBS_SOURCE_TRANSITION_CLOCK_STEP

#define MSEC 1000000ULL
#define TWO_63 (1ULL << 63)

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

static void check_bits_line(float actual, float expected, const char *what, int line)
{
	if (bits_of(actual) != bits_of(expected)) {
		fprintf(stderr, "line %d: %s is %a (0x%08x), expected %a (0x%08x)\n", line, what, (double)actual,
			bits_of(actual), (double)expected, bits_of(expected));
		fail();
	}
}

#define check_bits(actual, expected, what) check_bits_line(actual, expected, what, __LINE__)

static float clock_time(int mode, float manual_val, uint64_t start_time, uint64_t duration, uint64_t ts)
{
	return obs_source_transition_clock_time(mode, manual_val, start_time, duration, ts);
}

/* ---- calc_time() ---- */

static void time_manual_value(void **state)
{
	(void)state;

	/* the manual value as it is, whatever the clock says */
	check_bits(clock_time(MANUAL, 0.25f, 1000, 100, 0), 0.25f, "manual before the start");
	check_bits(clock_time(MANUAL, 0.25f, 1000, 100, 1050), 0.25f, "manual in the middle");
	check_bits(clock_time(MANUAL, 0.25f, 1000, 100, UINT64_MAX), 0.25f, "manual after the end");
	check_bits(clock_time(MANUAL, 0.75f, 0, 0, 5), 0.75f, "manual with no duration");

	/* characterized, not endorsed: not clamped, and -0 and NaN pass through
	 * with their bits (the render then never sees t >= 1 for a NaN) */
	check_bits(clock_time(MANUAL, -0.0f, 0, 0, 5), -0.0f, "manual -0");
	check_bits(clock_time(MANUAL, 2.5f, 0, 0, 5), 2.5f, "manual above 1");
	check_bits(clock_time(MANUAL, -1.0f, 0, 0, 5), -1.0f, "manual below 0");
	check_bits(clock_time(MANUAL, float_of(0x7fc01234), 0, 0, 5), float_of(0x7fc01234), "manual NaN");
	check_bits(clock_time(MANUAL, float_of(0xff800000), 0, 0, 5), float_of(0xff800000), "manual -inf");
}

static void time_around_the_start_and_end(void **state)
{
	(void)state;
	const uint64_t start = 3600000 * MSEC;
	const uint64_t duration = 100;

	check_bits(clock_time(AUTO, 0.5f, start, duration, 0), 0.0f, "at 0");
	check_bits(clock_time(AUTO, 0.5f, start, duration, start - 1), 0.0f, "start - 1");
	check_bits(clock_time(AUTO, 0.5f, start, duration, start), 0.0f, "start");
	check_bits(clock_time(AUTO, 0.5f, start, duration, start + 1), 0.01f, "start + 1");
	check_bits(clock_time(AUTO, 0.5f, start, duration, start + 50), 0.5f, "half way");
	check_bits(clock_time(AUTO, 0.5f, start, duration, start + duration - 1), 0.99f, "end - 1");
	check_bits(clock_time(AUTO, 0.5f, start, duration, start + duration), 1.0f, "end");
	check_bits(clock_time(AUTO, 0.5f, start, duration, start + duration + 1), 1.0f, "end + 1");
	check_bits(clock_time(AUTO, 0.5f, start, duration, UINT64_MAX), 1.0f, "UINT64_MAX");

	/* a 300 ms fade at 1/3 of the way: the ratio is rounded once to float */
	check_bits(clock_time(AUTO, 0.5f, start, 300 * MSEC, start + 100 * MSEC), (float)(1.0 / 3.0), "1/3");

	/* a start of 0: only ts 0 is "up to the start" */
	check_bits(clock_time(AUTO, 0.5f, 0, duration, 0), 0.0f, "start 0, ts 0");
	check_bits(clock_time(AUTO, 0.5f, 0, duration, 1), 0.01f, "start 0, ts 1");

	/* a start just below UINT64_MAX */
	check_bits(clock_time(AUTO, 0.5f, UINT64_MAX - 10, duration, UINT64_MAX), 0.1f, "start UINT64_MAX - 10");
}

static void time_zero_duration(void **state)
{
	(void)state;
	const uint64_t start = 3600000 * MSEC;

	/* ends at once, but still 0 up to the start */
	check_bits(clock_time(AUTO, 0.5f, start, 0, start - 1), 0.0f, "zero duration, start - 1");
	check_bits(clock_time(AUTO, 0.5f, start, 0, start), 0.0f, "zero duration, start");
	check_bits(clock_time(AUTO, 0.5f, start, 0, start + 1), 1.0f, "zero duration, start + 1");
	check_bits(clock_time(AUTO, 0.5f, 0, 0, UINT64_MAX), 1.0f, "zero duration, UINT64_MAX");
}

static void time_huge_durations(void **state)
{
	(void)state;

	/* a duration of 2^63: the quotients are exact in a double too */
	check_bits(clock_time(AUTO, 0.5f, 0, TWO_63, 1), ldexpf(1.0f, -63), "2^63, ts 1");
	check_bits(clock_time(AUTO, 0.5f, 0, TWO_63, 1ULL << 62), 0.5f, "2^63, ts 2^62");
	check_bits(clock_time(AUTO, 0.5f, 0, TWO_63, TWO_63), 1.0f, "2^63, at the end");

	/* characterized, not endorsed: 1 before the end of a long transition
	 * rounds to 1.0f, so t >= 1 a little early: any ratio of 1 - 2^-25 or
	 * more (the tie rounds to even), so the last 2^-25 of any transition,
	 * about 3 us of a 100 s one */
	check_bits(clock_time(AUTO, 0.5f, 0, TWO_63, TWO_63 - 1), 1.0f, "2^63, end - 1");
	check_bits(clock_time(AUTO, 0.5f, 0, 1ULL << 26, (1ULL << 26) - 1), 1.0f, "2^26, end - 1");
	check_bits(clock_time(AUTO, 0.5f, 0, 1ULL << 26, (1ULL << 26) - 2), 1.0f, "2^26, end - 2 (a tie)");
	check_bits(clock_time(AUTO, 0.5f, 0, 1ULL << 26, (1ULL << 26) - 3), 1.0f - ldexpf(1.0f, -24), "2^26, end - 3");
	check_bits(clock_time(AUTO, 0.5f, 0, 1ULL << 26, (1ULL << 26) - 4), 1.0f - ldexpf(1.0f, -24), "2^26, end - 4");

	/* above 2^63 */
	check_bits(clock_time(AUTO, 0.5f, 0, TWO_63 + 2, 1ULL << 62), 0.5f, "2^63 + 2, ts 2^62");
	check_bits(clock_time(AUTO, 0.5f, 0, UINT64_MAX, TWO_63), 0.5f, "UINT64_MAX, ts 2^63");
	check_bits(clock_time(AUTO, 0.5f, 0, UINT64_MAX, UINT64_MAX - 1), 1.0f, "UINT64_MAX, end - 1");
	check_bits(clock_time(AUTO, 0.5f, 0, UINT64_MAX, UINT64_MAX), 1.0f, "UINT64_MAX, at the end");
	check_bits(clock_time(AUTO, 0.5f, 1, UINT64_MAX, UINT64_MAX), 1.0f, "UINT64_MAX, start 1, end - 1");

	/* the longest duration obs_transition_start() can set: UINT32_MAX ms */
	const uint64_t longest = (uint64_t)UINT32_MAX * MSEC;
	check_bits(clock_time(AUTO, 0.5f, 0, longest, longest / 2), 0.5f, "UINT32_MAX ms, half way");
	check_bits(clock_time(AUTO, 0.5f, 0, longest, longest - 1), 1.0f, "UINT32_MAX ms, end - 1");
}

static void time_other_modes_are_automatic(void **state)
{
	(void)state;
	const int modes[] = {AUTO, 2, -1, INT32_MIN, INT32_MAX};

	/* only OBS_TRANSITION_MODE_MANUAL reads the manual value; a mode loaded
	 * from a scene collection outside the enum runs the clock */
	for (size_t i = 0; i < sizeof(modes) / sizeof(modes[0]); i++) {
		check_bits(clock_time(modes[i], 0.25f, 1000, 100, 1000), 0.0f, "other mode, start");
		check_bits(clock_time(modes[i], 0.25f, 1000, 100, 1050), 0.5f, "other mode, half way");
		check_bits(clock_time(modes[i], 0.25f, 1000, 100, 1100), 1.0f, "other mode, end");
	}
}

/* ---- the manual step of obs_transition_tick() ---- */

/* transition_manual_val after one tick, as obs_transition_tick() runs it */
static float manual_step(int mode, float val, float target, float torque, float clamp, float t)
{
	switch (obs_source_transition_clock_manual(mode, torque)) {
	case SNAP:
		return target;
	case STEP:
		return calc_torquef(val, target, torque, clamp, t);
	case HOLD:
		break;
	}
	return val;
}

static void manual_snaps_at_torque_zero(void **state)
{
	(void)state;

	assert_int_equal(obs_source_transition_clock_manual(MANUAL, 0.0f), SNAP);
	assert_int_equal(obs_source_transition_clock_manual(MANUAL, -0.0f), SNAP);
	check_bits(manual_step(MANUAL, 0.0f, 0.75f, 0.0f, 0.0f, 0.5f), 0.75f, "snap");
	check_bits(manual_step(MANUAL, 0.75f, 0.25f, -0.0f, 0.5f, 0.5f), 0.25f, "snap down at -0");

	/* the target as it is, outside 0 to 1 too */
	check_bits(manual_step(MANUAL, 0.0f, 2.0f, 0.0f, 0.0f, 0.5f), 2.0f, "snap above 1");
}

static void manual_steps_with_torque(void **state)
{
	(void)state;
	const float torques[] = {1.0f, 0.5f, -1.0f, 1e-30f, INFINITY, -INFINITY};

	for (size_t i = 0; i < sizeof(torques) / sizeof(torques[0]); i++)
		assert_int_equal(obs_source_transition_clock_manual(MANUAL, torques[i]), STEP);

	/* (target - val) * torque * t, no clamp */
	check_bits(manual_step(MANUAL, 0.0f, 1.0f, 0.5f, 0.0f, 0.5f), 0.25f, "step up");
	check_bits(manual_step(MANUAL, 1.0f, 0.0f, 0.5f, 0.0f, 0.5f), 0.75f, "step down");

	/* the overshoot is cut at the target */
	check_bits(manual_step(MANUAL, 0.0f, 1.0f, 8.0f, 0.0f, 1.0f), 1.0f, "overshoot up");
	check_bits(manual_step(MANUAL, 1.0f, 0.0f, 8.0f, 0.0f, 1.0f), 0.0f, "overshoot down");

	/* within calc_torquef()'s EPSILON: the target */
	check_bits(manual_step(MANUAL, 0.5f, 0.5f + ldexpf(1.0f, -20), 0.5f, 0.0f, 0.5f), 0.5f + ldexpf(1.0f, -20),
		   "close");
}

static void manual_steps_with_a_clamp(void **state)
{
	(void)state;

	/* the clamp is the least step per second: (1 - 0) * 2^-4 = 0.0625 is
	 * below 0.25, so the step is 0.25 * t */
	check_bits(manual_step(MANUAL, 0.0f, 1.0f, 0.0625f, 0.25f, 0.5f), 0.125f, "clamp up");
	check_bits(manual_step(MANUAL, 1.0f, 0.0f, 0.0625f, 0.25f, 0.5f), 0.875f, "clamp down");

	/* above the clamp: the torque's step */
	check_bits(manual_step(MANUAL, 0.0f, 1.0f, 0.5f, 0.25f, 1.0f), 0.5f, "above the clamp");

	/* a clamp that overshoots is cut at the target */
	check_bits(manual_step(MANUAL, 0.0f, 0.0625f, 0.0625f, 0.25f, 1.0f), 0.0625f, "clamp overshoot");
}

static void manual_nan_targets(void **state)
{
	(void)state;
	const float nan = float_of(0x7fc00000);

	/* characterized, not endorsed: a NaN target snaps to NaN at torque 0,
	 * and with a torque the distance is NaN, which no clamp compare
	 * replaces, so the step is NaN too. Either way the manual value stays
	 * NaN (and so does calc_time() in manual mode) until a new target. */
	check_bits(manual_step(MANUAL, 0.5f, nan, 0.0f, 0.25f, 0.5f), nan, "NaN target, snap");
	assert_true(isnan(manual_step(MANUAL, 0.5f, nan, 1.0f, 0.25f, 0.5f)));
	assert_true(isnan(manual_step(MANUAL, 0.5f, nan, 1.0f, 0.0f, 0.5f)));
	assert_true(isnan(manual_step(MANUAL, 0.5f, nan, 1.0f, 0.25f, 0.0f)));

	/* a NaN torque steps (NaN is not 0), to NaN */
	assert_int_equal(obs_source_transition_clock_manual(MANUAL, nan), STEP);
	assert_true(isnan(manual_step(MANUAL, 0.0f, 1.0f, nan, 0.25f, 0.5f)));
	assert_true(isnan(manual_step(MANUAL, 1.0f, 0.0f, nan, 0.25f, 0.5f)));

	/* a NaN value with a target: within EPSILON is false, so it steps, to
	 * NaN; only a snap (torque 0) recovers */
	assert_true(isnan(manual_step(MANUAL, nan, 1.0f, 1.0f, 0.25f, 0.5f)));
	check_bits(manual_step(MANUAL, nan, 1.0f, 0.0f, 0.25f, 0.5f), 1.0f, "NaN value, snap");
}

static void manual_only_in_manual_mode(void **state)
{
	(void)state;
	const int modes[] = {AUTO, 2, -1, INT32_MIN, INT32_MAX};
	const float torques[] = {0.0f, 1.0f, float_of(0x7fc00000)};

	for (size_t i = 0; i < sizeof(modes) / sizeof(modes[0]); i++) {
		for (size_t j = 0; j < sizeof(torques) / sizeof(torques[0]); j++) {
			assert_int_equal(obs_source_transition_clock_manual(modes[i], torques[j]), HOLD);
			check_bits(manual_step(modes[i], 0.5f, 1.0f, torques[j], 0.25f, 0.5f), 0.5f, "hold");
		}
	}
}

/* ---- get_cx(), get_cy() and the size fold ---- */

static void size_set_or_actual(void **state)
{
	(void)state;

	assert_int_equal(obs_source_transition_clock_size(1920, 1280), 1920);
	assert_int_equal(obs_source_transition_clock_size(640, 1280), 640);
	assert_int_equal(obs_source_transition_clock_size(0, 1280), 1280);
	assert_int_equal(obs_source_transition_clock_size(0, 0), 0);
	assert_int_equal(obs_source_transition_clock_size(1, 0), 1);
	assert_int_equal(obs_source_transition_clock_size(UINT32_MAX, 5), UINT32_MAX);
	assert_int_equal(obs_source_transition_clock_size(0, UINT32_MAX), UINT32_MAX);
}

static void fold_larger_child(void **state)
{
	(void)state;

	/* recalculate_transition_size(): from 0, the larger of the children */
	uint32_t cx = 0;
	cx = obs_source_transition_clock_fold(cx, 1280);
	cx = obs_source_transition_clock_fold(cx, 1920);
	assert_int_equal(cx, 1920);

	cx = 0;
	cx = obs_source_transition_clock_fold(cx, 1920);
	cx = obs_source_transition_clock_fold(cx, 1280);
	assert_int_equal(cx, 1920);

	assert_int_equal(obs_source_transition_clock_fold(0, 0), 0);
	assert_int_equal(obs_source_transition_clock_fold(1920, 1920), 1920);
	assert_int_equal(obs_source_transition_clock_fold(1920, 0), 1920);
	assert_int_equal(obs_source_transition_clock_fold(0, UINT32_MAX), UINT32_MAX);
	assert_int_equal(obs_source_transition_clock_fold(UINT32_MAX, 1), UINT32_MAX);
}

/* ---- the fit ---- */

#define UNSET 0x7fc0beefu

struct fit {
	bool matrix;
	float x;
	float y;
};

static struct fit fit_of(float tr_cx, float tr_cy, float source_cx, float source_cy, int scale_type)
{
	struct fit fit;
	fit.x = float_of(UNSET);
	fit.y = float_of(UNSET);
	fit.matrix = obs_source_transition_clock_fit(tr_cx, tr_cy, source_cx, source_cy, scale_type, &fit.x, &fit.y);
	return fit;
}

static void check_fit_line(struct fit fit, bool matrix, float x, float y, int line)
{
	if (fit.matrix != matrix) {
		fprintf(stderr, "line %d: matrix is %d, expected %d\n", line, fit.matrix, matrix);
		fail();
	}
	check_bits_line(fit.x, x, "scale x", line);
	check_bits_line(fit.y, y, "scale y", line);
}

#define check_fit(fit, x, y) check_fit_line(fit, true, x, y, __LINE__)
#define check_no_matrix(fit) check_fit_line(fit, false, float_of(UNSET), float_of(UNSET), __LINE__)

static void fit_max_only(void **state)
{
	(void)state;

	/* a child that fits keeps its size */
	check_fit(fit_of(1920, 1080, 1280, 720, MAX_ONLY), 1.0f, 1.0f);
	check_fit(fit_of(1920, 1080, 1920, 1080, MAX_ONLY), 1.0f, 1.0f);
	check_fit(fit_of(1920, 1080, 100, 1080, MAX_ONLY), 1.0f, 1.0f);

	/* a larger one, on either side, is fitted as aspect */
	check_fit(fit_of(1920, 1080, 3840, 2160, MAX_ONLY), 0.5f, 0.5f);
	check_fit(fit_of(1920, 1080, 2000, 500, MAX_ONLY), 0.96f, 0.96f);
	check_fit(fit_of(1920, 1080, 1000, 2160, MAX_ONLY), 0.5f, 0.5f);
	check_fit(fit_of(1920, 1080, 1921, 1080, MAX_ONLY), 1920.0f / 1921.0f, 1920.0f / 1921.0f);
	check_fit(fit_of(1920, 1080, 1920, 1081, MAX_ONLY), 1080.0f / 1081.0f, 1080.0f / 1081.0f);
}

static void fit_aspect(void **state)
{
	(void)state;

	/* the smaller ratio of the two sides: smaller, equal and larger */
	check_fit(fit_of(1920, 1080, 960, 540, ASPECT), 2.0f, 2.0f);
	check_fit(fit_of(1920, 1080, 1920, 1080, ASPECT), 1.0f, 1.0f);
	check_fit(fit_of(1920, 1080, 3840, 2160, ASPECT), 0.5f, 0.5f);

	/* wider than the transition: by the width; taller: by the height */
	check_fit(fit_of(1920, 1080, 2400, 900, ASPECT), 0.8f, 0.8f);
	check_fit(fit_of(1920, 1080, 1080, 1920, ASPECT), 0.5625f, 0.5625f);
	check_fit(fit_of(1920, 1080, 640, 480, ASPECT), 2.25f, 2.25f);

	/* the same aspect is not "wider": by the height */
	check_fit(fit_of(1920, 1080, 1280, 720, ASPECT), 1.5f, 1.5f);
}

static void fit_stretch(void **state)
{
	(void)state;

	check_fit(fit_of(1920, 1080, 960, 720, STRETCH), 2.0f, 1.5f);
	check_fit(fit_of(1920, 1080, 1920, 1080, STRETCH), 1.0f, 1.0f);
	check_fit(fit_of(1920, 1080, 3840, 1080, STRETCH), 0.5f, 1.0f);
	check_fit(fit_of(1920, 1080, 3840, 4320, STRETCH), 0.5f, 0.25f);
}

static void fit_zero_transition_size(void **state)
{
	(void)state;

	/* characterized, not endorsed: tr_cx / tr_cy is NaN (0 / 0) or inf
	 * (x / 0), so the aspect test is false and the height decides, or 0, so
	 * the width decides; every child is fitted to a scale of 0 on a side of
	 * 0, and a stretch scales the other side alone */
	const int types[] = {MAX_ONLY, ASPECT};
	for (size_t i = 0; i < sizeof(types) / sizeof(types[0]); i++) {
		check_fit(fit_of(0, 0, 1280, 720, types[i]), 0.0f, 0.0f);
		check_fit(fit_of(0, 1080, 1280, 720, types[i]), 0.0f, 0.0f);
		check_fit(fit_of(1920, 0, 1280, 720, types[i]), 0.0f, 0.0f);
	}
	check_fit(fit_of(0, 0, 1280, 720, STRETCH), 0.0f, 0.0f);
	check_fit(fit_of(0, 1080, 1280, 720, STRETCH), 0.0f, 1.5f);
	check_fit(fit_of(1920, 0, 1280, 720, STRETCH), 1.5f, 0.0f);
}

static void fit_zero_child_size(void **state)
{
	(void)state;
	const int types[] = {MAX_ONLY, ASPECT, STRETCH, 3, -1};

	/* no matrix, and the scale is not written */
	for (size_t i = 0; i < sizeof(types) / sizeof(types[0]); i++) {
		check_no_matrix(fit_of(1920, 1080, 0, 1080, types[i]));
		check_no_matrix(fit_of(1920, 1080, 1920, 0, types[i]));
		check_no_matrix(fit_of(1920, 1080, 0, 0, types[i]));
		check_no_matrix(fit_of(0, 0, 0, 0, types[i]));
		check_no_matrix(fit_of(1920, 1080, -0.0f, 1080, types[i]));
	}
}

static void fit_other_scale_types(void **state)
{
	(void)state;
	const int types[] = {3, -1, 100, INT32_MIN, INT32_MAX};

	/* characterized, not endorsed: a scale type outside the enum (a scene
	 * collection's transition_scale_type is loaded as it is) gives a matrix
	 * but writes no scale, so recalculate_transition_matrix() then uses a
	 * scale it never set (reported, not changed here) */
	for (size_t i = 0; i < sizeof(types) / sizeof(types[0]); i++) {
		check_fit(fit_of(1920, 1080, 1280, 720, types[i]), float_of(UNSET), float_of(UNSET));
		check_fit(fit_of(1920, 1080, 3840, 2160, types[i]), float_of(UNSET), float_of(UNSET));
	}
}

/* ---- mix_spaces() ---- */

static void mix_spaces_every_pair(void **state)
{
	(void)state;
	const int spaces[] = {SRGB, SRGB_16F, EXTENDED, SCRGB};
	/* expected[a][b] */
	const int expected[4][4] = {
		{SRGB, SRGB_16F, EXTENDED, EXTENDED},
		{SRGB_16F, SRGB_16F, EXTENDED, EXTENDED},
		{EXTENDED, EXTENDED, EXTENDED, EXTENDED},
		{EXTENDED, EXTENDED, EXTENDED, EXTENDED},
	};

	for (size_t a = 0; a < 4; a++) {
		for (size_t b = 0; b < 4; b++) {
			int got = obs_source_transition_clock_mix_spaces(spaces[a], spaces[b]);
			if (got != expected[a][b]) {
				fprintf(stderr, "mix_spaces(%d, %d) is %d, expected %d\n", spaces[a], spaces[b], got,
					expected[a][b]);
				fail();
			}
		}
	}

	/* values outside the enum count as GS_CS_SRGB */
	assert_int_equal(obs_source_transition_clock_mix_spaces(-1, 4), SRGB);
	assert_int_equal(obs_source_transition_clock_mix_spaces(4, SRGB_16F), SRGB_16F);
	assert_int_equal(obs_source_transition_clock_mix_spaces(INT32_MIN, SCRGB), EXTENDED);
	assert_int_equal(obs_source_transition_clock_mix_spaces(INT32_MAX, INT32_MAX), SRGB);
}

/* ---- transition_calc_min_ts() ---- */

static void min_ts_none_one_two(void **state)
{
	(void)state;

	/* none */
	assert_true(obs_source_transition_clock_min_ts(0, 0) == 0);

	/* one, in either place */
	assert_true(obs_source_transition_clock_min_ts(5000, 0) == 5000);
	assert_true(obs_source_transition_clock_min_ts(0, 5000) == 5000);
	assert_true(obs_source_transition_clock_min_ts(UINT64_MAX, 0) == UINT64_MAX);
	assert_true(obs_source_transition_clock_min_ts(0, 1) == 1);

	/* two: the earlier */
	assert_true(obs_source_transition_clock_min_ts(3000, 5000) == 3000);
	assert_true(obs_source_transition_clock_min_ts(5000, 3000) == 3000);
	assert_true(obs_source_transition_clock_min_ts(5000, 5000) == 5000);
	assert_true(obs_source_transition_clock_min_ts(UINT64_MAX, 1) == 1);
	assert_true(obs_source_transition_clock_min_ts(1, UINT64_MAX) == 1);
	assert_true(obs_source_transition_clock_min_ts(UINT64_MAX, UINT64_MAX - 1) == UINT64_MAX - 1);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(time_manual_value),
		cmocka_unit_test(time_around_the_start_and_end),
		cmocka_unit_test(time_zero_duration),
		cmocka_unit_test(time_huge_durations),
		cmocka_unit_test(time_other_modes_are_automatic),
		cmocka_unit_test(manual_snaps_at_torque_zero),
		cmocka_unit_test(manual_steps_with_torque),
		cmocka_unit_test(manual_steps_with_a_clamp),
		cmocka_unit_test(manual_nan_targets),
		cmocka_unit_test(manual_only_in_manual_mode),
		cmocka_unit_test(size_set_or_actual),
		cmocka_unit_test(fold_larger_child),
		cmocka_unit_test(fit_max_only),
		cmocka_unit_test(fit_aspect),
		cmocka_unit_test(fit_stretch),
		cmocka_unit_test(fit_zero_transition_size),
		cmocka_unit_test(fit_zero_child_size),
		cmocka_unit_test(fit_other_scale_types),
		cmocka_unit_test(mix_spaces_every_pair),
		cmocka_unit_test(min_ts_none_one_two),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
