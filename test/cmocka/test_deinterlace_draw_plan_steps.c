#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <inttypes.h>
#include <limits.h>
#include <math.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs.h"
#include "obs-source-deinterlace-draw.h"

/*
 * Characterization of the deinterlace draw decisions of
 * libobs/obs-source-deinterlace.c (get_effect(),
 * deinterlace_linear_required(), deinterlace_render() and
 * obs_source_set_deinterlace_mode()), moved to
 * libobs/obs-source-deinterlace-draw.c before its Rust port:
 * - obs_source_deinterlace_draw_effect(): the effect of each mode;
 * - obs_source_deinterlace_draw_linear_required(): the modes that need
 *   linear sRGB input;
 * - obs_source_deinterlace_draw_skip(): the early return of
 *   deinterlace_render();
 * - obs_source_deinterlace_draw_reads_linear_srgb() and
 *   obs_source_deinterlace_draw_linear_srgb(): the linear sRGB flag, and
 *   when gs_get_linear_srgb() is read;
 * - obs_source_deinterlace_draw_technique(),
 *   obs_source_deinterlace_draw_reads_white_level() and
 *   obs_source_deinterlace_draw_multiplier(): the technique and multiplier
 *   for each pair of source and current colour spaces;
 * - obs_source_deinterlace_draw_frame2(): whether the second field is due
 *   (#395, #1995, #2048);
 * - obs_source_deinterlace_draw_mode_change(): what
 *   obs_source_set_deinterlace_mode() does, a mode outside the enum being
 *   DISABLE (#2120).
 * Cases marked "characterized, not endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them. test_deinterlace_2x_field_order (the
 * second field through the real render) and test_deinterlace_mode_load (a
 * mode outside the enum from a scene collection) cover the callers through
 * the real libobs.
 */

#define TWOX 1000000ULL

/* modes outside enum obs_deinterlace_mode */
static const int OUTSIDE_MODES[] = {-1, 99, INT_MIN, INT_MAX, 9};
#define OUTSIDE_MODE_COUNT (sizeof(OUTSIDE_MODES) / sizeof(OUTSIDE_MODES[0]))

/* colour spaces outside enum gs_color_space */
static const int OUTSIDE_SPACES[] = {-1, 4, 99, INT_MIN, INT_MAX};
#define OUTSIDE_SPACE_COUNT (sizeof(OUTSIDE_SPACES) / sizeof(OUTSIDE_SPACES[0]))

static const int SPACES[] = {GS_CS_SRGB, GS_CS_SRGB_16F, GS_CS_709_EXTENDED, GS_CS_709_SCRGB};
#define SPACE_COUNT (sizeof(SPACES) / sizeof(SPACES[0]))

static uint32_t float_bits(float f)
{
	uint32_t u;
	memcpy(&u, &f, sizeof(u));
	return u;
}

static float bits_float(uint32_t u)
{
	float f;
	memcpy(&f, &u, sizeof(f));
	return f;
}

static void check_int(long long actual, long long expected, const char *what, int line)
{
	if (actual != expected) {
		fprintf(stderr, "line %d: %s is %lld, expected %lld\n", line, what, actual, expected);
		fail();
	}
}

static void check_bool(bool actual, bool expected, const char *what, int line)
{
	if (actual != expected) {
		fprintf(stderr, "line %d: %s is %s, expected %s\n", line, what, actual ? "true" : "false",
			expected ? "true" : "false");
		fail();
	}
}

static void check_bits(float actual, uint32_t expected, const char *what, int line)
{
	if (float_bits(actual) != expected) {
		fprintf(stderr, "line %d: %s is %a (0x%08" PRIx32 "), expected %a (0x%08" PRIx32 ")\n", line, what,
			(double)actual, float_bits(actual), (double)bits_float(expected), expected);
		fail();
	}
}

#define CHECK_INT(actual, expected) check_int((long long)(actual), (long long)(expected), #actual, __LINE__)
#define CHECK_BOOL(actual, expected) check_bool((actual), (expected), #actual, __LINE__)
#define CHECK_BITS(actual, expected) check_bits((actual), (expected), #actual, __LINE__)

/* --- enum values ------------------------------------------------------- */

/* The Rust port returns these numbers; the C maps them to effects, names and
 * actions. */
static void enum_values(void **state)
{
	(void)state;
	CHECK_INT(OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_NONE, -1);
	CHECK_INT(OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_DISCARD, 0);
	CHECK_INT(OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_DISCARD_2X, 1);
	CHECK_INT(OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_BLEND, 2);
	CHECK_INT(OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_BLEND_2X, 3);
	CHECK_INT(OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_LINEAR, 4);
	CHECK_INT(OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_LINEAR_2X, 5);
	CHECK_INT(OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_YADIF, 6);
	CHECK_INT(OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_YADIF_2X, 7);
	CHECK_INT(OBS_SOURCE_DRAW_TECH_DRAW, 0);
	CHECK_INT(OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY, 1);
	CHECK_INT(OBS_SOURCE_DRAW_TECH_DRAW_TONEMAP, 2);
	CHECK_INT(OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY_TONEMAP, 3);
	CHECK_INT(OBS_SOURCE_DEINTERLACE_DRAW_MODE_SAME, 0);
	CHECK_INT(OBS_SOURCE_DEINTERLACE_DRAW_MODE_ENABLE, 1);
	CHECK_INT(OBS_SOURCE_DEINTERLACE_DRAW_MODE_DISABLE, 2);
	CHECK_INT(OBS_SOURCE_DEINTERLACE_DRAW_MODE_SWITCH, 3);
	CHECK_INT(sizeof(enum obs_source_deinterlace_draw_effect), sizeof(int));
	CHECK_INT(sizeof(enum obs_source_draw_technique), sizeof(int));
	CHECK_INT(sizeof(enum obs_source_deinterlace_draw_mode_change), sizeof(int));
}

/* --- effect and linear rule per mode ---------------------------------- */

/* Each mode draws with its own effect; RETRO is discard 2x. */
static void effect_every_mode(void **state)
{
	(void)state;
	CHECK_INT(obs_source_deinterlace_draw_effect(OBS_DEINTERLACE_MODE_DISABLE),
		  OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_NONE);
	CHECK_INT(obs_source_deinterlace_draw_effect(OBS_DEINTERLACE_MODE_DISCARD),
		  OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_DISCARD);
	CHECK_INT(obs_source_deinterlace_draw_effect(OBS_DEINTERLACE_MODE_RETRO),
		  OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_DISCARD_2X);
	CHECK_INT(obs_source_deinterlace_draw_effect(OBS_DEINTERLACE_MODE_BLEND),
		  OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_BLEND);
	CHECK_INT(obs_source_deinterlace_draw_effect(OBS_DEINTERLACE_MODE_BLEND_2X),
		  OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_BLEND_2X);
	CHECK_INT(obs_source_deinterlace_draw_effect(OBS_DEINTERLACE_MODE_LINEAR),
		  OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_LINEAR);
	CHECK_INT(obs_source_deinterlace_draw_effect(OBS_DEINTERLACE_MODE_LINEAR_2X),
		  OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_LINEAR_2X);
	CHECK_INT(obs_source_deinterlace_draw_effect(OBS_DEINTERLACE_MODE_YADIF),
		  OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_YADIF);
	CHECK_INT(obs_source_deinterlace_draw_effect(OBS_DEINTERLACE_MODE_YADIF_2X),
		  OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_YADIF_2X);
}

/* A mode outside the enum has no effect (-1, 99, the ends of int, 9: one past
 * YADIF_2X). */
static void effect_outside_enum(void **state)
{
	(void)state;
	for (size_t i = 0; i < OUTSIDE_MODE_COUNT; i++)
		CHECK_INT(obs_source_deinterlace_draw_effect(OUTSIDE_MODES[i]),
			  OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_NONE);
}

/* Blend, linear and yadif (and their 2x) read neighbouring lines, so they
 * need linear input; disable, discard and retro do not. */
static void linear_required_every_mode(void **state)
{
	(void)state;
	CHECK_BOOL(obs_source_deinterlace_draw_linear_required(OBS_DEINTERLACE_MODE_DISABLE), false);
	CHECK_BOOL(obs_source_deinterlace_draw_linear_required(OBS_DEINTERLACE_MODE_DISCARD), false);
	CHECK_BOOL(obs_source_deinterlace_draw_linear_required(OBS_DEINTERLACE_MODE_RETRO), false);
	CHECK_BOOL(obs_source_deinterlace_draw_linear_required(OBS_DEINTERLACE_MODE_BLEND), true);
	CHECK_BOOL(obs_source_deinterlace_draw_linear_required(OBS_DEINTERLACE_MODE_BLEND_2X), true);
	CHECK_BOOL(obs_source_deinterlace_draw_linear_required(OBS_DEINTERLACE_MODE_LINEAR), true);
	CHECK_BOOL(obs_source_deinterlace_draw_linear_required(OBS_DEINTERLACE_MODE_LINEAR_2X), true);
	CHECK_BOOL(obs_source_deinterlace_draw_linear_required(OBS_DEINTERLACE_MODE_YADIF), true);
	CHECK_BOOL(obs_source_deinterlace_draw_linear_required(OBS_DEINTERLACE_MODE_YADIF_2X), true);
	for (size_t i = 0; i < OUTSIDE_MODE_COUNT; i++)
		CHECK_BOOL(obs_source_deinterlace_draw_linear_required(OUTSIDE_MODES[i]), false);
}

/* --- the early return ------------------------------------------------- */

static void skip_without_texture_or_size(void **state)
{
	(void)state;
	const uint32_t sizes[] = {0, 1, 1080, UINT32_MAX};
	for (int cur = 0; cur < 2; cur++)
		for (int prev = 0; prev < 2; prev++)
			for (size_t w = 0; w < 4; w++)
				for (size_t h = 0; h < 4; h++) {
					const bool draws = cur && prev && sizes[w] && sizes[h];
					CHECK_BOOL(obs_source_deinterlace_draw_skip(cur, prev, sizes[w], sizes[h]),
						   !draws);
				}
	CHECK_BOOL(obs_source_deinterlace_draw_skip(true, true, 1920, 1080), false);
	CHECK_BOOL(obs_source_deinterlace_draw_skip(true, true, 1, 1), false);
	CHECK_BOOL(obs_source_deinterlace_draw_skip(false, true, 1920, 1080), true);
	CHECK_BOOL(obs_source_deinterlace_draw_skip(true, false, 1920, 1080), true);
	CHECK_BOOL(obs_source_deinterlace_draw_skip(true, true, 0, 1080), true);
	CHECK_BOOL(obs_source_deinterlace_draw_skip(true, true, 1920, 0), true);
}

/* --- linear sRGB -------------------------------------------------------- */

/* gs_get_linear_srgb() is read for an SDR source only, as the short circuit
 * of the C did; GS_CS_SRGB_16F and HDR sources are linear already. */
static void reads_linear_srgb_every_space(void **state)
{
	(void)state;
	CHECK_BOOL(obs_source_deinterlace_draw_reads_linear_srgb(GS_CS_SRGB), true);
	CHECK_BOOL(obs_source_deinterlace_draw_reads_linear_srgb(GS_CS_SRGB_16F), false);
	CHECK_BOOL(obs_source_deinterlace_draw_reads_linear_srgb(GS_CS_709_EXTENDED), false);
	CHECK_BOOL(obs_source_deinterlace_draw_reads_linear_srgb(GS_CS_709_SCRGB), false);
	for (size_t i = 0; i < OUTSIDE_SPACE_COUNT; i++)
		CHECK_BOOL(obs_source_deinterlace_draw_reads_linear_srgb(OUTSIDE_SPACES[i]), false);
}

static void linear_srgb_every_input(void **state)
{
	(void)state;
	for (int mode = OBS_DEINTERLACE_MODE_DISABLE; mode <= OBS_DEINTERLACE_MODE_YADIF_2X; mode++) {
		const bool required = mode >= OBS_DEINTERLACE_MODE_BLEND;
		for (int gs = 0; gs < 2; gs++) {
			/* SDR: the graphics flag or the mode */
			CHECK_BOOL(obs_source_deinterlace_draw_linear_srgb(GS_CS_SRGB, gs, mode), gs || required);
			/* any other space, in the enum or not: always */
			for (size_t s = 1; s < SPACE_COUNT; s++)
				CHECK_BOOL(obs_source_deinterlace_draw_linear_srgb(SPACES[s], gs, mode), true);
			for (size_t s = 0; s < OUTSIDE_SPACE_COUNT; s++)
				CHECK_BOOL(obs_source_deinterlace_draw_linear_srgb(OUTSIDE_SPACES[s], gs, mode), true);
		}
	}
	/* a mode outside the enum needs nothing */
	for (size_t i = 0; i < OUTSIDE_MODE_COUNT; i++) {
		CHECK_BOOL(obs_source_deinterlace_draw_linear_srgb(GS_CS_SRGB, false, OUTSIDE_MODES[i]), false);
		CHECK_BOOL(obs_source_deinterlace_draw_linear_srgb(GS_CS_SRGB, true, OUTSIDE_MODES[i]), true);
		CHECK_BOOL(obs_source_deinterlace_draw_linear_srgb(GS_CS_709_SCRGB, false, OUTSIDE_MODES[i]), true);
	}
}

/* --- technique and multiplier per space pair ---------------------------- */

/* [source][current], in SPACES order */
static const enum obs_source_draw_technique TECHNIQUES[4][4] = {
	/* GS_CS_SRGB into SRGB, SRGB_16F, 709_EXTENDED, 709_SCRGB */
	{OBS_SOURCE_DRAW_TECH_DRAW, OBS_SOURCE_DRAW_TECH_DRAW, OBS_SOURCE_DRAW_TECH_DRAW,
	 OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY},
	/* GS_CS_SRGB_16F */
	{OBS_SOURCE_DRAW_TECH_DRAW, OBS_SOURCE_DRAW_TECH_DRAW, OBS_SOURCE_DRAW_TECH_DRAW,
	 OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY},
	/* GS_CS_709_EXTENDED */
	{OBS_SOURCE_DRAW_TECH_DRAW_TONEMAP, OBS_SOURCE_DRAW_TECH_DRAW_TONEMAP, OBS_SOURCE_DRAW_TECH_DRAW,
	 OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY},
	/* GS_CS_709_SCRGB */
	{OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY_TONEMAP, OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY_TONEMAP,
	 OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY, OBS_SOURCE_DRAW_TECH_DRAW},
};

/* The multiplier at a white level of 80, 300 and 0, as float bits:
 * [source][current][white]. SDR and 709_EXTENDED into scRGB scale up by
 * white / 80; scRGB into SDR or 709_EXTENDED scales down by 80 / white. */
static const float WHITES[3] = {80.0f, 300.0f, 0.0f};
static const uint32_t MULTIPLIERS[4][4][3] = {
	/* GS_CS_SRGB */
	{{0x3f800000, 0x3f800000, 0x3f800000},
	 {0x3f800000, 0x3f800000, 0x3f800000},
	 {0x3f800000, 0x3f800000, 0x3f800000},
	 /* 80 / 80, 300 / 80 = 3.75, 0 / 80 */
	 {0x3f800000, 0x40700000, 0x00000000}},
	/* GS_CS_SRGB_16F */
	{{0x3f800000, 0x3f800000, 0x3f800000},
	 {0x3f800000, 0x3f800000, 0x3f800000},
	 {0x3f800000, 0x3f800000, 0x3f800000},
	 {0x3f800000, 0x40700000, 0x00000000}},
	/* GS_CS_709_EXTENDED: tonemapped into SDR at 1 */
	{{0x3f800000, 0x3f800000, 0x3f800000},
	 {0x3f800000, 0x3f800000, 0x3f800000},
	 {0x3f800000, 0x3f800000, 0x3f800000},
	 {0x3f800000, 0x40700000, 0x00000000}},
	/* GS_CS_709_SCRGB: 80 / 80, 80 / 300 rounded to float, 80 / 0 = +inf
	 * (characterized, not endorsed: a white level of 0 is not a level
	 * OBS sets, see the case below) */
	{{0x3f800000, 0x3e888889, 0x7f800000},
	 {0x3f800000, 0x3e888889, 0x7f800000},
	 {0x3f800000, 0x3e888889, 0x7f800000},
	 {0x3f800000, 0x3f800000, 0x3f800000}},
};

static void technique_space_pairs(void **state)
{
	(void)state;
	for (size_t s = 0; s < SPACE_COUNT; s++)
		for (size_t c = 0; c < SPACE_COUNT; c++) {
			const enum obs_source_draw_technique t =
				obs_source_deinterlace_draw_technique(SPACES[s], SPACES[c]);
			if (t != TECHNIQUES[s][c]) {
				fprintf(stderr, "source %d into %d: technique %d, expected %d\n", SPACES[s], SPACES[c],
					(int)t, (int)TECHNIQUES[s][c]);
				fail();
			}
			const bool multiplies = TECHNIQUES[s][c] == OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY ||
						TECHNIQUES[s][c] == OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY_TONEMAP;
			CHECK_BOOL(obs_source_deinterlace_draw_reads_white_level(SPACES[s], SPACES[c]), multiplies);
		}
}

/* A space outside the enum, as source or current, draws with Draw at 1 and
 * reads no white level (the C's switches have no default). */
static void technique_outside_enum(void **state)
{
	(void)state;
	for (size_t o = 0; o < OUTSIDE_SPACE_COUNT; o++) {
		for (size_t s = 0; s < SPACE_COUNT; s++) {
			CHECK_INT(obs_source_deinterlace_draw_technique(OUTSIDE_SPACES[o], SPACES[s]),
				  OBS_SOURCE_DRAW_TECH_DRAW);
			CHECK_INT(obs_source_deinterlace_draw_technique(SPACES[s], OUTSIDE_SPACES[o]),
				  OBS_SOURCE_DRAW_TECH_DRAW);
			CHECK_BOOL(obs_source_deinterlace_draw_reads_white_level(OUTSIDE_SPACES[o], SPACES[s]), false);
			CHECK_BOOL(obs_source_deinterlace_draw_reads_white_level(SPACES[s], OUTSIDE_SPACES[o]), false);
			CHECK_BITS(obs_source_deinterlace_draw_multiplier(OUTSIDE_SPACES[o], SPACES[s], 300.0f),
				   0x3f800000);
			CHECK_BITS(obs_source_deinterlace_draw_multiplier(SPACES[s], OUTSIDE_SPACES[o], 300.0f),
				   0x3f800000);
		}
	}
}

static void multiplier_space_pairs(void **state)
{
	(void)state;
	for (size_t s = 0; s < SPACE_COUNT; s++)
		for (size_t c = 0; c < SPACE_COUNT; c++)
			for (size_t w = 0; w < 3; w++) {
				const float m = obs_source_deinterlace_draw_multiplier(SPACES[s], SPACES[c], WHITES[w]);
				if (float_bits(m) != MULTIPLIERS[s][c][w]) {
					fprintf(stderr,
						"source %d into %d at %g: 0x%08" PRIx32 ", expected 0x%08" PRIx32 "\n",
						SPACES[s], SPACES[c], (double)WHITES[w], float_bits(m),
						MULTIPLIERS[s][c][w]);
					fail();
				}
			}
}

/* One float division each way, so the multiplier is the IEEE quotient of
 * whatever level comes in. Characterized, not endorsed: OBS sets levels from
 * 80 to 480 nits, but obs_get_video_sdr_white_level() is passed through
 * unchecked: 0 gives +inf from scRGB, -0 gives -inf, a negative level a
 * negative multiplier, and a NaN stays a NaN. */
static void multiplier_any_level(void **state)
{
	(void)state;
	/* 80 / -0 */
	CHECK_BITS(obs_source_deinterlace_draw_multiplier(GS_CS_709_SCRGB, GS_CS_SRGB, -0.0f), 0xff800000);
	/* -0 / 80 */
	CHECK_BITS(obs_source_deinterlace_draw_multiplier(GS_CS_SRGB, GS_CS_709_SCRGB, -0.0f), 0x80000000);
	/* 80 / inf, inf / 80 */
	CHECK_BITS(obs_source_deinterlace_draw_multiplier(GS_CS_709_SCRGB, GS_CS_709_EXTENDED, INFINITY), 0x00000000);
	CHECK_BITS(obs_source_deinterlace_draw_multiplier(GS_CS_709_EXTENDED, GS_CS_709_SCRGB, INFINITY), 0x7f800000);
	/* -80 */
	CHECK_BITS(obs_source_deinterlace_draw_multiplier(GS_CS_SRGB_16F, GS_CS_709_SCRGB, -80.0f), 0xbf800000);
	CHECK_BITS(obs_source_deinterlace_draw_multiplier(GS_CS_709_SCRGB, GS_CS_SRGB_16F, -80.0f), 0xbf800000);
	/* 480 nits, the largest OBS offers: 6 and 1/6 */
	CHECK_BITS(obs_source_deinterlace_draw_multiplier(GS_CS_SRGB, GS_CS_709_SCRGB, 480.0f), 0x40c00000);
	CHECK_BITS(obs_source_deinterlace_draw_multiplier(GS_CS_709_SCRGB, GS_CS_SRGB, 480.0f), 0x3e2aaaab);
	/* the smallest subnormal: 80 / it overflows to +inf */
	CHECK_BITS(obs_source_deinterlace_draw_multiplier(GS_CS_709_SCRGB, GS_CS_SRGB, bits_float(1)), 0x7f800000);
	/* a NaN stays a NaN either way; pairs that do not multiply give 1 */
	assert_true(isnan(obs_source_deinterlace_draw_multiplier(GS_CS_709_SCRGB, GS_CS_SRGB, NAN)));
	assert_true(isnan(obs_source_deinterlace_draw_multiplier(GS_CS_SRGB, GS_CS_709_SCRGB, NAN)));
	CHECK_BITS(obs_source_deinterlace_draw_multiplier(GS_CS_SRGB, GS_CS_SRGB, NAN), 0x3f800000);
	CHECK_BITS(obs_source_deinterlace_draw_multiplier(GS_CS_709_EXTENDED, GS_CS_SRGB, 0.0f), 0x3f800000);
}

/* --- the second field -------------------------------------------------- */

/* Due from frame_ts + offset + half_duration - TWOX_TOLERANCE on (>=): a
 * 50i field (20 ms) is due 19 ms after the frame on the canvas clock. */
static void frame2_at_threshold(void **state)
{
	(void)state;
	const uint64_t frame_ts = 5000000000ULL;
	const uint64_t offset = 995000000000ULL;
	const uint32_t half = 20000000;
	const uint64_t at = frame_ts + offset + half - TWOX;
	CHECK_BOOL(obs_source_deinterlace_draw_frame2(at - 1, frame_ts, offset, half), false);
	CHECK_BOOL(obs_source_deinterlace_draw_frame2(at, frame_ts, offset, half), true);
	CHECK_BOOL(obs_source_deinterlace_draw_frame2(at + 1, frame_ts, offset, half), true);
	CHECK_BOOL(obs_source_deinterlace_draw_frame2(0, frame_ts, offset, half), false);
	CHECK_BOOL(obs_source_deinterlace_draw_frame2(UINT64_MAX, frame_ts, offset, half), true);
	/* the half duration counts in full: UINT32_MAX is 4.29 s */
	const uint64_t far = frame_ts + offset + UINT32_MAX - TWOX;
	CHECK_BOOL(obs_source_deinterlace_draw_frame2(far - 1, frame_ts, offset, UINT32_MAX), false);
	CHECK_BOOL(obs_source_deinterlace_draw_frame2(far, frame_ts, offset, UINT32_MAX), true);
	/* a half duration of exactly TWOX_TOLERANCE: due at frame_ts + offset */
	CHECK_BOOL(obs_source_deinterlace_draw_frame2(frame_ts + offset - 1, frame_ts, offset, (uint32_t)TWOX), false);
	CHECK_BOOL(obs_source_deinterlace_draw_frame2(frame_ts + offset, frame_ts, offset, (uint32_t)TWOX), true);
}

/* Characterized, not endorsed: the sum wraps in uint64_t. When frame_ts +
 * offset + half_duration passes UINT64_MAX (a buffered offset that wrapped,
 * #395), the threshold lands near 0 and the second field is due at once. */
static void frame2_sum_wraps(void **state)
{
	(void)state;
	const uint64_t frame_ts = UINT64_MAX - 5;
	const uint64_t offset = 10;
	const uint32_t half = (uint32_t)TWOX;
	/* (UINT64_MAX - 5 + 10 + TWOX - TWOX) mod 2^64 = 4 */
	CHECK_BOOL(obs_source_deinterlace_draw_frame2(3, frame_ts, offset, half), false);
	CHECK_BOOL(obs_source_deinterlace_draw_frame2(4, frame_ts, offset, half), true);
	CHECK_BOOL(obs_source_deinterlace_draw_frame2(5, frame_ts, offset, half), true);
	/* the offset alone wraps: frame_ts + offset = 2^64 + 999999 */
	CHECK_BOOL(obs_source_deinterlace_draw_frame2(19999998, 1000000, UINT64_MAX, 20000000), false);
	CHECK_BOOL(obs_source_deinterlace_draw_frame2(19999999, 1000000, UINT64_MAX, 20000000), true);
	/* everything at its end: 2 * (2^64 - 1) + (2^32 - 1) - TWOX */
	const uint64_t at = UINT64_MAX + UINT64_MAX + (uint64_t)UINT32_MAX - TWOX;
	CHECK_BOOL(obs_source_deinterlace_draw_frame2(at - 1, UINT64_MAX, UINT64_MAX, UINT32_MAX), false);
	CHECK_BOOL(obs_source_deinterlace_draw_frame2(at, UINT64_MAX, UINT64_MAX, UINT32_MAX), true);
}

/* Characterized, not endorsed: with half_duration < TWOX_TOLERANCE and
 * frame_ts + offset smaller than the difference, the threshold wraps below 0
 * to near UINT64_MAX, so the second field is never due. A source has
 * frame_ts 0 and offset 0 only before its first frame, when nothing draws. */
static void frame2_below_tolerance_wraps(void **state)
{
	(void)state;
	/* 0 + 0 + 0 - TWOX = 2^64 - TWOX */
	CHECK_BOOL(obs_source_deinterlace_draw_frame2(0, 0, 0, 0), false);
	CHECK_BOOL(obs_source_deinterlace_draw_frame2(UINT64_MAX - TWOX, 0, 0, 0), false);
	CHECK_BOOL(obs_source_deinterlace_draw_frame2(UINT64_MAX - TWOX + 1, 0, 0, 0), true);
	/* half_duration TWOX - 1: the threshold is UINT64_MAX */
	CHECK_BOOL(obs_source_deinterlace_draw_frame2(UINT64_MAX - 1, 0, 0, (uint32_t)TWOX - 1), false);
	CHECK_BOOL(obs_source_deinterlace_draw_frame2(UINT64_MAX, 0, 0, (uint32_t)TWOX - 1), true);
	/* once frame_ts + offset covers the difference, no wrap */
	CHECK_BOOL(obs_source_deinterlace_draw_frame2(0, 1, 0, (uint32_t)TWOX - 1), true);
	CHECK_BOOL(obs_source_deinterlace_draw_frame2(0, 0, 500000, 500000), true);
	CHECK_BOOL(obs_source_deinterlace_draw_frame2(0, 0, 499999, 500000), false);
	/* a half duration of 0 at an ordinary time: due a TWOX_TOLERANCE early */
	CHECK_BOOL(obs_source_deinterlace_draw_frame2(4000000 - TWOX - 1, 1000000, 3000000, 0), false);
	CHECK_BOOL(obs_source_deinterlace_draw_frame2(4000000 - TWOX, 1000000, 3000000, 0), true);
}

/* --- obs_source_set_deinterlace_mode() ---------------------------------- */

static enum obs_source_deinterlace_draw_mode_change expected_change(int current, int mode)
{
	/* a mode outside the enum is DISABLE (#2120) */
	if (mode < OBS_DEINTERLACE_MODE_DISABLE || mode > OBS_DEINTERLACE_MODE_YADIF_2X)
		mode = OBS_DEINTERLACE_MODE_DISABLE;
	if (current == mode)
		return OBS_SOURCE_DEINTERLACE_DRAW_MODE_SAME;
	if (current == OBS_DEINTERLACE_MODE_DISABLE)
		return OBS_SOURCE_DEINTERLACE_DRAW_MODE_ENABLE;
	if (mode == OBS_DEINTERLACE_MODE_DISABLE)
		return OBS_SOURCE_DEINTERLACE_DRAW_MODE_DISABLE;
	return OBS_SOURCE_DEINTERLACE_DRAW_MODE_SWITCH;
}

/* Every mode into every mode: nothing for the same mode, enable from
 * DISABLE, disable to DISABLE, switch between two others. */
static void mode_change_every_pair(void **state)
{
	(void)state;
	for (int cur = OBS_DEINTERLACE_MODE_DISABLE; cur <= OBS_DEINTERLACE_MODE_YADIF_2X; cur++)
		for (int mode = OBS_DEINTERLACE_MODE_DISABLE; mode <= OBS_DEINTERLACE_MODE_YADIF_2X; mode++) {
			const enum obs_source_deinterlace_draw_mode_change c =
				obs_source_deinterlace_draw_mode_change(cur, mode);
			if (c != expected_change(cur, mode)) {
				fprintf(stderr, "%d to %d: %d, expected %d\n", cur, mode, (int)c,
					(int)expected_change(cur, mode));
				fail();
			}
		}
	CHECK_INT(obs_source_deinterlace_draw_mode_change(OBS_DEINTERLACE_MODE_DISABLE, OBS_DEINTERLACE_MODE_DISABLE),
		  OBS_SOURCE_DEINTERLACE_DRAW_MODE_SAME);
	CHECK_INT(obs_source_deinterlace_draw_mode_change(OBS_DEINTERLACE_MODE_DISABLE, OBS_DEINTERLACE_MODE_YADIF_2X),
		  OBS_SOURCE_DEINTERLACE_DRAW_MODE_ENABLE);
	CHECK_INT(obs_source_deinterlace_draw_mode_change(OBS_DEINTERLACE_MODE_YADIF_2X, OBS_DEINTERLACE_MODE_DISABLE),
		  OBS_SOURCE_DEINTERLACE_DRAW_MODE_DISABLE);
	CHECK_INT(obs_source_deinterlace_draw_mode_change(OBS_DEINTERLACE_MODE_BLEND, OBS_DEINTERLACE_MODE_YADIF_2X),
		  OBS_SOURCE_DEINTERLACE_DRAW_MODE_SWITCH);
	CHECK_INT(obs_source_deinterlace_draw_mode_change(OBS_DEINTERLACE_MODE_RETRO, OBS_DEINTERLACE_MODE_RETRO),
		  OBS_SOURCE_DEINTERLACE_DRAW_MODE_SAME);
}

/* A mode outside the enum disables deinterlacing, or does nothing when it is
 * off already (#2120; before it, "enable" with no effect, so the source
 * drew nothing). */
static void mode_change_outside_enum(void **state)
{
	(void)state;
	for (size_t i = 0; i < OUTSIDE_MODE_COUNT; i++) {
		CHECK_INT(obs_source_deinterlace_draw_mode_change(OBS_DEINTERLACE_MODE_DISABLE, OUTSIDE_MODES[i]),
			  OBS_SOURCE_DEINTERLACE_DRAW_MODE_SAME);
		for (int cur = OBS_DEINTERLACE_MODE_DISCARD; cur <= OBS_DEINTERLACE_MODE_YADIF_2X; cur++)
			CHECK_INT(obs_source_deinterlace_draw_mode_change(cur, OUTSIDE_MODES[i]),
				  OBS_SOURCE_DEINTERLACE_DRAW_MODE_DISABLE);
	}
}

/* Characterized, not endorsed: a current mode outside the enum, which
 * obs_source_set_deinterlace_mode() never stores since #2120, is not
 * DISABLE: a mode in the enum switches, and a mode outside it (99 included,
 * the same value) disables. */
static void mode_change_current_outside_enum(void **state)
{
	(void)state;
	for (size_t i = 0; i < OUTSIDE_MODE_COUNT; i++) {
		CHECK_INT(obs_source_deinterlace_draw_mode_change(OUTSIDE_MODES[i], OUTSIDE_MODES[i]),
			  OBS_SOURCE_DEINTERLACE_DRAW_MODE_DISABLE);
		CHECK_INT(obs_source_deinterlace_draw_mode_change(OUTSIDE_MODES[i], OBS_DEINTERLACE_MODE_DISABLE),
			  OBS_SOURCE_DEINTERLACE_DRAW_MODE_DISABLE);
		CHECK_INT(obs_source_deinterlace_draw_mode_change(OUTSIDE_MODES[i], OBS_DEINTERLACE_MODE_YADIF),
			  OBS_SOURCE_DEINTERLACE_DRAW_MODE_SWITCH);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(enum_values),
		cmocka_unit_test(effect_every_mode),
		cmocka_unit_test(effect_outside_enum),
		cmocka_unit_test(linear_required_every_mode),
		cmocka_unit_test(skip_without_texture_or_size),
		cmocka_unit_test(reads_linear_srgb_every_space),
		cmocka_unit_test(linear_srgb_every_input),
		cmocka_unit_test(technique_space_pairs),
		cmocka_unit_test(technique_outside_enum),
		cmocka_unit_test(multiplier_space_pairs),
		cmocka_unit_test(multiplier_any_level),
		cmocka_unit_test(frame2_at_threshold),
		cmocka_unit_test(frame2_sum_wraps),
		cmocka_unit_test(frame2_below_tolerance_wraps),
		cmocka_unit_test(mode_change_every_pair),
		cmocka_unit_test(mode_change_outside_enum),
		cmocka_unit_test(mode_change_current_outside_enum),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
