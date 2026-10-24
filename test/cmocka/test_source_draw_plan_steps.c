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
#include "obs-source-draw-plan.h"

/*
 * Characterization of the source draw decisions of libobs/obs-source.c
 * (rotate_async_video(), obs_source_render_async_video() and
 * source_render()), moved to libobs/obs-source-draw-plan.c before its Rust
 * port:
 * - obs_source_draw_plan_rotation_x() and _y(): the translate that moves a
 *   rotated async frame back into its box;
 * - obs_source_draw_plan_async_linear_srgb() and
 *   obs_source_draw_plan_async_nonlinear_alpha(): the linear sRGB flag and
 *   the non-linear alpha blend of an async frame (792f8473e, 080095786);
 * - obs_source_draw_plan_technique(): the technique of an async frame, and
 *   of source_render()'s colour space conversion;
 * - obs_source_draw_plan_reads_white_level() and
 *   obs_source_draw_plan_multiplier(): when the SDR white level is read and
 *   the multiplier it gives, for both;
 * - obs_source_draw_plan_convert(): whether source_render() draws through
 *   color_space_texrender (#1493 is the colour space that picks it).
 * Cases marked "characterized, not endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them. test_async_rotation_normalize,
 * test_filter_color_space, test_source_color_space_steps,
 * test_color_correction_16f and test_pipewire_cursor_hdr cover the callers
 * through the real libobs.
 */

/* colour spaces outside enum gs_color_space */
static const int OUTSIDE_SPACES[] = {-1, 4, 99, INT_MIN, INT_MAX};
#define OUTSIDE_SPACE_COUNT (sizeof(OUTSIDE_SPACES) / sizeof(OUTSIDE_SPACES[0]))

static const int SPACES[] = {GS_CS_SRGB, GS_CS_SRGB_16F, GS_CS_709_EXTENDED, GS_CS_709_SCRGB};
#define SPACE_COUNT (sizeof(SPACES) / sizeof(SPACES[0]))

/* SDR white levels: OBS's default, a common one, and 0 */
static const float WHITES[] = {80.0f, 300.0f, 0.0f};
#define WHITE_COUNT (sizeof(WHITES) / sizeof(WHITES[0]))

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

#define D OBS_SOURCE_DRAW_TECH_DRAW
#define M OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY
#define T OBS_SOURCE_DRAW_TECH_DRAW_TONEMAP
#define MT OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY_TONEMAP
#define NA OBS_SOURCE_DRAW_TECH_DRAW_NONLINEAR_ALPHA
#define NAM OBS_SOURCE_DRAW_TECH_DRAW_NONLINEAR_ALPHA_MULTIPLY

/* --- enum values ------------------------------------------------------- */

/* The Rust port returns these numbers; the C maps them to names. The first
 * four are the deinterlace draw decisions' too. */
static void enum_values(void **state)
{
	(void)state;
	CHECK_INT(OBS_SOURCE_DRAW_TECH_DRAW, 0);
	CHECK_INT(OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY, 1);
	CHECK_INT(OBS_SOURCE_DRAW_TECH_DRAW_TONEMAP, 2);
	CHECK_INT(OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY_TONEMAP, 3);
	CHECK_INT(OBS_SOURCE_DRAW_TECH_DRAW_NONLINEAR_ALPHA, 4);
	CHECK_INT(OBS_SOURCE_DRAW_TECH_DRAW_NONLINEAR_ALPHA_MULTIPLY, 5);
	CHECK_INT(sizeof(enum obs_source_draw_technique), sizeof(int));
	CHECK_INT(sizeof(enum gs_color_space), sizeof(int));
	assert_string_equal(obs_source_draw_technique_name(D), "Draw");
	assert_string_equal(obs_source_draw_technique_name(M), "DrawMultiply");
	assert_string_equal(obs_source_draw_technique_name(T), "DrawTonemap");
	assert_string_equal(obs_source_draw_technique_name(MT), "DrawMultiplyTonemap");
	assert_string_equal(obs_source_draw_technique_name(NA), "DrawNonlinearAlpha");
	assert_string_equal(obs_source_draw_technique_name(NAM), "DrawNonlinearAlphaMultiply");
	assert_string_equal(obs_source_draw_technique_name((enum obs_source_draw_technique)6), "Draw");
	assert_string_equal(obs_source_draw_technique_name((enum obs_source_draw_technique) - 1), "Draw");
}

/* --- rotation ---------------------------------------------------------- */

/* The frame is rotated about its top left corner, clockwise on screen
 * (gs_matrix_rotaa4f() about -z), then moved back into its box: a 1920 x 1080
 * frame turned by 90 is moved down by its width, by 180 right by its width
 * and down by its height, by 270 right by its height. */
static void rotation_multiples_of_90(void **state)
{
	(void)state;
	CHECK_BITS(obs_source_draw_plan_rotation_x(0, 1920, 1080), 0x00000000);
	CHECK_BITS(obs_source_draw_plan_rotation_y(0, 1920, 1080), 0x00000000);
	CHECK_BITS(obs_source_draw_plan_rotation_x(90, 1920, 1080), 0x00000000);
	CHECK_BITS(obs_source_draw_plan_rotation_y(90, 1920, 1080), 0x44f00000); /* 1920 */
	CHECK_BITS(obs_source_draw_plan_rotation_x(180, 1920, 1080), 0x44f00000);
	CHECK_BITS(obs_source_draw_plan_rotation_y(180, 1920, 1080), 0x44870000); /* 1080 */
	CHECK_BITS(obs_source_draw_plan_rotation_x(270, 1920, 1080), 0x44870000);
	CHECK_BITS(obs_source_draw_plan_rotation_y(270, 1920, 1080), 0x00000000);
	/* a portrait frame: the width and height trade places */
	CHECK_BITS(obs_source_draw_plan_rotation_y(90, 1080, 1920), 0x44870000);
	CHECK_BITS(obs_source_draw_plan_rotation_x(180, 1080, 1920), 0x44870000);
	CHECK_BITS(obs_source_draw_plan_rotation_y(180, 1080, 1920), 0x44f00000);
	CHECK_BITS(obs_source_draw_plan_rotation_x(270, 1080, 1920), 0x44f00000);
}

/* Any other angle is not moved: the frame turns about its corner. In 0-359
 * only 90, 180 and 270 move (obs_source_set_async_rotation(), #2136).
 * Characterized, not endorsed: a rotation outside 0-359, which the setter
 * no longer stores, is not moved either, even a multiple of 90. */
static void rotation_other_angles(void **state)
{
	(void)state;
	const long angles[] = {1,   45,  89,  91,  135,  179,  181,      269,     271,
			       359, 360, 450, -90, -180, -270, LONG_MIN, LONG_MAX};
	for (size_t i = 0; i < sizeof(angles) / sizeof(angles[0]); i++) {
		CHECK_BITS(obs_source_draw_plan_rotation_x(angles[i], 1920, 1080), 0x00000000);
		CHECK_BITS(obs_source_draw_plan_rotation_y(angles[i], 1920, 1080), 0x00000000);
		CHECK_BITS(obs_source_draw_plan_rotation_x(angles[i], 640, 480), 0x00000000);
		CHECK_BITS(obs_source_draw_plan_rotation_y(angles[i], 640, 480), 0x00000000);
	}
}

/* The size converts as (float)uint32_t: 0 is +0, sizes past 2^24 round to
 * nearest even, UINT32_MAX is 2^32. */
static void rotation_size_to_float(void **state)
{
	(void)state;
	CHECK_BITS(obs_source_draw_plan_rotation_y(90, 0, 7), 0x00000000);
	CHECK_BITS(obs_source_draw_plan_rotation_x(270, 7, 0), 0x00000000);
	CHECK_BITS(obs_source_draw_plan_rotation_y(90, 16777217, 3), 0x4b800000);    /* 2^24 */
	CHECK_BITS(obs_source_draw_plan_rotation_x(180, 16777219, 3), 0x4b800002);   /* 2^24 + 4 */
	CHECK_BITS(obs_source_draw_plan_rotation_y(180, 3, UINT32_MAX), 0x4f800000); /* 2^32 */
	CHECK_BITS(obs_source_draw_plan_rotation_x(270, 3, UINT32_MAX), 0x4f800000);
	CHECK_BITS(obs_source_draw_plan_rotation_x(180, 1, 1), 0x3f800000);
}

/* --- async video: linear sRGB and non-linear alpha ------------------------- */

/* With the graphics linear flag off: an SDR (GS_CS_SRGB) frame into anything
 * but GS_CS_SRGB (792f8473e), and a 709_EXTENDED or scRGB frame into SDR,
 * which is tonemapped (080095786). GS_CS_SRGB_16F never forces it. With the
 * flag on, always. */
static const bool ASYNC_LINEAR[4][4] = {
	{false, true, true, true},
	{false, false, false, false},
	{true, true, false, false},
	{true, true, false, false},
};

static void async_linear_srgb_space_pairs(void **state)
{
	(void)state;
	for (size_t s = 0; s < SPACE_COUNT; s++)
		for (size_t c = 0; c < SPACE_COUNT; c++) {
			CHECK_BOOL(obs_source_draw_plan_async_linear_srgb(SPACES[s], SPACES[c], false),
				   ASYNC_LINEAR[s][c]);
			CHECK_BOOL(obs_source_draw_plan_async_linear_srgb(SPACES[s], SPACES[c], true), true);
		}
}

/* A space outside the enum keeps the graphics flag, except that an SDR
 * frame goes linear into a current space outside the enum, as it is not
 * GS_CS_SRGB (characterized, not endorsed). */
static void async_linear_srgb_outside_enum(void **state)
{
	(void)state;
	for (size_t o = 0; o < OUTSIDE_SPACE_COUNT; o++) {
		for (int gs = 0; gs < 2; gs++) {
			for (size_t s = 0; s < SPACE_COUNT; s++)
				CHECK_BOOL(obs_source_draw_plan_async_linear_srgb(OUTSIDE_SPACES[o], SPACES[s], gs),
					   gs);
			CHECK_BOOL(obs_source_draw_plan_async_linear_srgb(GS_CS_SRGB, OUTSIDE_SPACES[o], gs), true);
			CHECK_BOOL(obs_source_draw_plan_async_linear_srgb(GS_CS_SRGB_16F, OUTSIDE_SPACES[o], gs), gs);
			CHECK_BOOL(obs_source_draw_plan_async_linear_srgb(GS_CS_709_EXTENDED, OUTSIDE_SPACES[o], gs),
				   gs);
			CHECK_BOOL(obs_source_draw_plan_async_linear_srgb(GS_CS_709_SCRGB, OUTSIDE_SPACES[o], gs), gs);
		}
	}
}

/* Only an SDR (GS_CS_SRGB) frame drawn with linear sRGB whose alpha is not
 * linear blends with non-linear alpha. */
static void async_nonlinear_alpha_every_input(void **state)
{
	(void)state;
	for (int linear = 0; linear < 2; linear++)
		for (int alpha = 0; alpha < 2; alpha++) {
			CHECK_BOOL(obs_source_draw_plan_async_nonlinear_alpha(GS_CS_SRGB, linear, alpha),
				   linear && !alpha);
			for (size_t s = 1; s < SPACE_COUNT; s++)
				CHECK_BOOL(obs_source_draw_plan_async_nonlinear_alpha(SPACES[s], linear, alpha), false);
			for (size_t o = 0; o < OUTSIDE_SPACE_COUNT; o++)
				CHECK_BOOL(obs_source_draw_plan_async_nonlinear_alpha(OUTSIDE_SPACES[o], linear, alpha),
					   false);
		}
}

/* --- technique ---------------------------------------------------------- */

/* [source][current], without and with non-linear alpha */
static const enum obs_source_draw_technique TECHNIQUES[2][4][4] = {
	{
		{D, D, D, M},
		{D, D, D, M},
		{T, T, D, M},
		{MT, MT, M, D},
	},
	{
		{NA, NA, NA, NAM},
		{D, D, D, M},
		{T, T, D, M},
		{MT, MT, M, D},
	},
};

static void technique_space_pairs(void **state)
{
	(void)state;
	for (int na = 0; na < 2; na++)
		for (size_t s = 0; s < SPACE_COUNT; s++)
			for (size_t c = 0; c < SPACE_COUNT; c++) {
				const enum obs_source_draw_technique t =
					obs_source_draw_plan_technique(SPACES[s], SPACES[c], na);
				if (t != TECHNIQUES[na][s][c]) {
					fprintf(stderr,
						"source %d into %d, non-linear alpha %d: technique %d, expected %d\n",
						SPACES[s], SPACES[c], na, (int)t, (int)TECHNIQUES[na][s][c]);
					fail();
				}
			}
}

/* A space outside the enum, as source or current, draws with Draw, with
 * non-linear alpha too (characterized, not endorsed: an SDR frame into a
 * current space outside the enum still blends with non-linear alpha, but
 * with Draw). */
static void technique_outside_enum(void **state)
{
	(void)state;
	for (size_t o = 0; o < OUTSIDE_SPACE_COUNT; o++)
		for (int na = 0; na < 2; na++)
			for (size_t s = 0; s < SPACE_COUNT; s++) {
				CHECK_INT(obs_source_draw_plan_technique(OUTSIDE_SPACES[o], SPACES[s], na), D);
				CHECK_INT(obs_source_draw_plan_technique(SPACES[s], OUTSIDE_SPACES[o], na), D);
			}
	CHECK_BOOL(obs_source_draw_plan_async_nonlinear_alpha(
			   GS_CS_SRGB, obs_source_draw_plan_async_linear_srgb(GS_CS_SRGB, 4, false), false),
		   true);
}

/* --- white level and multiplier ------------------------------------------ */

static const bool READS_WHITE[4][4] = {
	{false, false, false, true},
	{false, false, false, true},
	{false, false, false, true},
	{true, true, true, false},
};

/* [source][current][white level 80, 300, 0]: the level over 80 into scRGB,
 * 80 over the level out of it (300 / 80 = 3.75, 80 / 300 = 0x3e888889).
 * Characterized, not endorsed: a level of 0 gives 0 into scRGB and +inf out
 * of it (OBS sets levels from 80 to 480 nits). */
static const uint32_t MULTIPLIERS[4][4][3] = {
	{{0x3f800000, 0x3f800000, 0x3f800000},
	 {0x3f800000, 0x3f800000, 0x3f800000},
	 {0x3f800000, 0x3f800000, 0x3f800000},
	 {0x3f800000, 0x40700000, 0x00000000}},
	{{0x3f800000, 0x3f800000, 0x3f800000},
	 {0x3f800000, 0x3f800000, 0x3f800000},
	 {0x3f800000, 0x3f800000, 0x3f800000},
	 {0x3f800000, 0x40700000, 0x00000000}},
	{{0x3f800000, 0x3f800000, 0x3f800000},
	 {0x3f800000, 0x3f800000, 0x3f800000},
	 {0x3f800000, 0x3f800000, 0x3f800000},
	 {0x3f800000, 0x40700000, 0x00000000}},
	{{0x3f800000, 0x3e888889, 0x7f800000},
	 {0x3f800000, 0x3e888889, 0x7f800000},
	 {0x3f800000, 0x3e888889, 0x7f800000},
	 {0x3f800000, 0x3f800000, 0x3f800000}},
};

static void reads_white_level_space_pairs(void **state)
{
	(void)state;
	for (size_t s = 0; s < SPACE_COUNT; s++)
		for (size_t c = 0; c < SPACE_COUNT; c++)
			CHECK_BOOL(obs_source_draw_plan_reads_white_level(SPACES[s], SPACES[c]), READS_WHITE[s][c]);
	for (size_t o = 0; o < OUTSIDE_SPACE_COUNT; o++)
		for (size_t s = 0; s < SPACE_COUNT; s++) {
			CHECK_BOOL(obs_source_draw_plan_reads_white_level(OUTSIDE_SPACES[o], SPACES[s]), false);
			CHECK_BOOL(obs_source_draw_plan_reads_white_level(SPACES[s], OUTSIDE_SPACES[o]), false);
		}
}

static void multiplier_space_pairs(void **state)
{
	(void)state;
	for (size_t s = 0; s < SPACE_COUNT; s++)
		for (size_t c = 0; c < SPACE_COUNT; c++)
			for (size_t w = 0; w < WHITE_COUNT; w++) {
				const float m = obs_source_draw_plan_multiplier(SPACES[s], SPACES[c], WHITES[w]);
				if (float_bits(m) != MULTIPLIERS[s][c][w]) {
					fprintf(stderr,
						"source %d into %d at %g: 0x%08" PRIx32 ", expected 0x%08" PRIx32 "\n",
						SPACES[s], SPACES[c], (double)WHITES[w], float_bits(m),
						MULTIPLIERS[s][c][w]);
					fail();
				}
			}
}

/* A space outside the enum gives 1 at any level. */
static void multiplier_outside_enum(void **state)
{
	(void)state;
	for (size_t o = 0; o < OUTSIDE_SPACE_COUNT; o++)
		for (size_t s = 0; s < SPACE_COUNT; s++)
			for (size_t w = 0; w < WHITE_COUNT; w++) {
				CHECK_BITS(obs_source_draw_plan_multiplier(OUTSIDE_SPACES[o], SPACES[s], WHITES[w]),
					   0x3f800000);
				CHECK_BITS(obs_source_draw_plan_multiplier(SPACES[s], OUTSIDE_SPACES[o], WHITES[w]),
					   0x3f800000);
			}
}

/* One float division each way, so the multiplier is the IEEE quotient of
 * whatever level comes in. Characterized, not endorsed:
 * obs_get_video_sdr_white_level() is passed through unchecked: -0 gives -inf
 * from scRGB, a negative level a negative multiplier, and a NaN stays a
 * NaN. */
static void multiplier_any_level(void **state)
{
	(void)state;
	/* 80 / -0, -0 / 80 */
	CHECK_BITS(obs_source_draw_plan_multiplier(GS_CS_709_SCRGB, GS_CS_SRGB, -0.0f), 0xff800000);
	CHECK_BITS(obs_source_draw_plan_multiplier(GS_CS_SRGB, GS_CS_709_SCRGB, -0.0f), 0x80000000);
	/* 80 / inf, inf / 80 */
	CHECK_BITS(obs_source_draw_plan_multiplier(GS_CS_709_SCRGB, GS_CS_709_EXTENDED, INFINITY), 0x00000000);
	CHECK_BITS(obs_source_draw_plan_multiplier(GS_CS_709_EXTENDED, GS_CS_709_SCRGB, INFINITY), 0x7f800000);
	/* -80 */
	CHECK_BITS(obs_source_draw_plan_multiplier(GS_CS_SRGB_16F, GS_CS_709_SCRGB, -80.0f), 0xbf800000);
	CHECK_BITS(obs_source_draw_plan_multiplier(GS_CS_709_SCRGB, GS_CS_SRGB_16F, -80.0f), 0xbf800000);
	/* 480 nits, the largest OBS offers: 6 and 1/6 */
	CHECK_BITS(obs_source_draw_plan_multiplier(GS_CS_SRGB, GS_CS_709_SCRGB, 480.0f), 0x40c00000);
	CHECK_BITS(obs_source_draw_plan_multiplier(GS_CS_709_SCRGB, GS_CS_SRGB, 480.0f), 0x3e2aaaab);
	/* 203 nits (BT.2408 reference white) */
	CHECK_BITS(obs_source_draw_plan_multiplier(GS_CS_709_EXTENDED, GS_CS_709_SCRGB, 203.0f), 0x40226666);
	/* the smallest subnormal: 80 / it overflows to +inf, it / 80 is 0 */
	CHECK_BITS(obs_source_draw_plan_multiplier(GS_CS_709_SCRGB, GS_CS_SRGB, bits_float(1)), 0x7f800000);
	CHECK_BITS(obs_source_draw_plan_multiplier(GS_CS_SRGB, GS_CS_709_SCRGB, bits_float(1)), 0x00000000);
	/* a NaN stays a NaN either way; pairs that do not multiply give 1 */
	assert_true(isnan(obs_source_draw_plan_multiplier(GS_CS_709_SCRGB, GS_CS_SRGB, NAN)));
	assert_true(isnan(obs_source_draw_plan_multiplier(GS_CS_SRGB, GS_CS_709_SCRGB, NAN)));
	CHECK_BITS(obs_source_draw_plan_multiplier(GS_CS_SRGB, GS_CS_SRGB, NAN), 0x3f800000);
	CHECK_BITS(obs_source_draw_plan_multiplier(GS_CS_709_EXTENDED, GS_CS_SRGB, 0.0f), 0x3f800000);
}

/* --- source_render() colour space conversion ------------------------------ */

/* Converted when the classes differ (SDR, 709_EXTENDED, scRGB); SDR and
 * SDR 16F are one class. */
static const bool CONVERTS[4][4] = {
	{false, false, true, true},
	{false, false, true, true},
	{true, true, false, true},
	{true, true, true, false},
};

static void convert_space_pairs(void **state)
{
	(void)state;
	for (size_t s = 0; s < SPACE_COUNT; s++)
		for (size_t c = 0; c < SPACE_COUNT; c++)
			CHECK_BOOL(obs_source_draw_plan_convert(SPACES[s], SPACES[c]), CONVERTS[s][c]);
}

/* A space outside the enum, as source or current, is drawn directly. */
static void convert_outside_enum(void **state)
{
	(void)state;
	for (size_t o = 0; o < OUTSIDE_SPACE_COUNT; o++) {
		for (size_t s = 0; s < SPACE_COUNT; s++) {
			CHECK_BOOL(obs_source_draw_plan_convert(OUTSIDE_SPACES[o], SPACES[s]), false);
			CHECK_BOOL(obs_source_draw_plan_convert(SPACES[s], OUTSIDE_SPACES[o]), false);
		}
		CHECK_BOOL(obs_source_draw_plan_convert(OUTSIDE_SPACES[o], OUTSIDE_SPACES[o]), false);
	}
}

/* --- the decisions in their callers' order ------------------------------- */

/* What obs_source_render_async_video() draws with for every pair, both
 * graphics linear flags, both alpha flags and white levels of 80, 300 and
 * 0: the linear flag, the blend, the technique, and the multiplier (1 when
 * the level is not read). */
static void async_plan_every_input(void **state)
{
	(void)state;
	for (size_t s = 0; s < SPACE_COUNT; s++)
		for (size_t c = 0; c < SPACE_COUNT; c++)
			for (int gs = 0; gs < 2; gs++)
				for (int alpha = 0; alpha < 2; alpha++)
					for (size_t w = 0; w < WHITE_COUNT; w++) {
						const bool linear = obs_source_draw_plan_async_linear_srgb(
							SPACES[s], SPACES[c], gs);
						const bool na = obs_source_draw_plan_async_nonlinear_alpha(
							SPACES[s], linear, alpha);
						const enum obs_source_draw_technique t =
							obs_source_draw_plan_technique(SPACES[s], SPACES[c], na);
						const float m =
							obs_source_draw_plan_reads_white_level(SPACES[s], SPACES[c])
								? obs_source_draw_plan_multiplier(SPACES[s], SPACES[c],
												  WHITES[w])
								: 1.0f;
						const bool want_linear = gs || ASYNC_LINEAR[s][c];
						const bool want_na = s == 0 && want_linear && !alpha;
						CHECK_BOOL(linear, want_linear);
						CHECK_BOOL(na, want_na);
						CHECK_INT(t, TECHNIQUES[want_na][s][c]);
						CHECK_BITS(m, MULTIPLIERS[s][c][w]);
					}
}

/* What source_render() does for every pair at white levels of 80, 300 and
 * 0: draw directly, or through color_space_texrender with the technique
 * (never with non-linear alpha) and the multiplier. */
static void convert_plan_every_input(void **state)
{
	(void)state;
	for (size_t s = 0; s < SPACE_COUNT; s++)
		for (size_t c = 0; c < SPACE_COUNT; c++)
			for (size_t w = 0; w < WHITE_COUNT; w++) {
				CHECK_BOOL(obs_source_draw_plan_convert(SPACES[s], SPACES[c]), CONVERTS[s][c]);
				if (!CONVERTS[s][c])
					continue;
				CHECK_INT(obs_source_draw_plan_technique(SPACES[s], SPACES[c], false),
					  TECHNIQUES[0][s][c]);
				const float m =
					obs_source_draw_plan_reads_white_level(SPACES[s], SPACES[c])
						? obs_source_draw_plan_multiplier(SPACES[s], SPACES[c], WHITES[w])
						: 1.0f;
				CHECK_BITS(m, MULTIPLIERS[s][c][w]);
			}
	/* SDR into 709_EXTENDED converts with plain Draw at 1 */
	CHECK_INT(obs_source_draw_plan_technique(GS_CS_SRGB, GS_CS_709_EXTENDED, false), D);
	CHECK_BOOL(obs_source_draw_plan_reads_white_level(GS_CS_SRGB_16F, GS_CS_709_EXTENDED), false);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(enum_values),
		cmocka_unit_test(rotation_multiples_of_90),
		cmocka_unit_test(rotation_other_angles),
		cmocka_unit_test(rotation_size_to_float),
		cmocka_unit_test(async_linear_srgb_space_pairs),
		cmocka_unit_test(async_linear_srgb_outside_enum),
		cmocka_unit_test(async_nonlinear_alpha_every_input),
		cmocka_unit_test(technique_space_pairs),
		cmocka_unit_test(technique_outside_enum),
		cmocka_unit_test(reads_white_level_space_pairs),
		cmocka_unit_test(multiplier_space_pairs),
		cmocka_unit_test(multiplier_outside_enum),
		cmocka_unit_test(multiplier_any_level),
		cmocka_unit_test(convert_space_pairs),
		cmocka_unit_test(convert_outside_enum),
		cmocka_unit_test(async_plan_every_input),
		cmocka_unit_test(convert_plan_every_input),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
