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
#include <float.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs.h"
#include "obs-canvas-draw.h"

/*
 * Characterization of the canvas texture draw and display clear decisions,
 * moved to libobs/obs-canvas-draw.c before their Rust port:
 * - obs_render_canvas_texture_internal() (obs.c): obs_canvas_draw_skip(),
 *   the early return; obs_canvas_draw_technique(), the technique for each
 *   pair of source and current colour spaces;
 *   obs_canvas_draw_reads_white_level() and obs_canvas_draw_multiplier(),
 *   when the SDR white level is read and the multiplier it gives;
 * - render_display_begin() (obs-display.c):
 *   obs_display_clear_reads_space() and obs_display_clear_linear(), how the
 *   background colour converts to the clear colour (87698b1e0 for the
 *   always-sRGB hosts); obs_display_clear_flags() and
 *   obs_display_clear_draws_solid(), the clear and the solid colour
 *   workaround (f88300a61, 37389ff24).
 * Cases marked "characterized, not endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them. No test drives the two callers through
 * a real graphics device; test_canvas_flags, test_scene_private_canvas,
 * test_canvas_destroy_scene_refs and test_shutdown_canvas_weak_ref cover the
 * canvases and their mixes through the real libobs.
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

/* --- constants --------------------------------------------------------- */

/* The Rust port returns these numbers and flags. */
static void constants(void **state)
{
	(void)state;
	CHECK_INT(OBS_SOURCE_DRAW_TECH_DRAW, 0);
	CHECK_INT(OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY, 1);
	CHECK_INT(OBS_SOURCE_DRAW_TECH_DRAW_TONEMAP, 2);
	CHECK_INT(sizeof(enum obs_source_draw_technique), sizeof(int));
	CHECK_INT(sizeof(enum gs_color_space), sizeof(int));
	CHECK_INT(OBS_DISPLAY_CLEAR_COLOR, GS_CLEAR_COLOR);
	CHECK_INT(OBS_DISPLAY_CLEAR_DEPTH, GS_CLEAR_DEPTH);
	CHECK_INT(OBS_DISPLAY_CLEAR_STENCIL, GS_CLEAR_STENCIL);
	CHECK_INT(OBS_DISPLAY_CLEAR_COLOR, 1);
	CHECK_INT(OBS_DISPLAY_CLEAR_DEPTH, 2);
	CHECK_INT(OBS_DISPLAY_CLEAR_STENCIL, 4);
	assert_string_equal(obs_source_draw_technique_name(D), "Draw");
	assert_string_equal(obs_source_draw_technique_name(M), "DrawMultiply");
	assert_string_equal(obs_source_draw_technique_name(T), "DrawTonemap");
}

/* --- canvas: the early return ------------------------------------------ */

/* A canvas draws only when it has a mix that rendered its texture: both
 * early returns (no mix, texture not rendered) skip. obs.c passes
 * texture_rendered false without a mix. Characterized, not endorsed: no mix
 * with texture_rendered true, which obs.c never passes, also skips. */
static void skip_every_input(void **state)
{
	(void)state;
	CHECK_BOOL(obs_canvas_draw_skip(false, false), true);
	CHECK_BOOL(obs_canvas_draw_skip(false, true), true);
	CHECK_BOOL(obs_canvas_draw_skip(true, false), true);
	CHECK_BOOL(obs_canvas_draw_skip(true, true), false);
}

/* --- canvas: technique ------------------------------------------------- */

/* [source][current]: a 709_EXTENDED canvas is tonemapped into SDR; anything
 * into scRGB is multiplied; everything else draws as is (an SDR canvas into
 * 709_EXTENDED, scRGB into SDR included: characterized, not endorsed, the
 * mix renders only SDR and 709_EXTENDED). */
static const enum obs_source_draw_technique TECHNIQUE[4][4] = {
	{D, D, D, M},
	{D, D, D, M},
	{T, T, D, M},
	{D, D, D, M},
};

static void technique_space_pairs(void **state)
{
	(void)state;
	for (size_t s = 0; s < SPACE_COUNT; s++) {
		for (size_t c = 0; c < SPACE_COUNT; c++) {
			CHECK_INT(obs_canvas_draw_technique(SPACES[s], SPACES[c]), TECHNIQUE[s][c]);
		}
	}
}

/* A current space outside the enum draws as is, from any source. A source
 * outside the enum takes the current space's row: Draw into SDR and
 * 709_EXTENDED, DrawMultiply into scRGB. */
static void technique_outside_enum(void **state)
{
	(void)state;
	for (size_t o = 0; o < OUTSIDE_SPACE_COUNT; o++) {
		for (size_t s = 0; s < SPACE_COUNT; s++)
			CHECK_INT(obs_canvas_draw_technique(SPACES[s], OUTSIDE_SPACES[o]), D);
		for (size_t p = 0; p < OUTSIDE_SPACE_COUNT; p++)
			CHECK_INT(obs_canvas_draw_technique(OUTSIDE_SPACES[p], OUTSIDE_SPACES[o]), D);
		CHECK_INT(obs_canvas_draw_technique(OUTSIDE_SPACES[o], GS_CS_SRGB), D);
		CHECK_INT(obs_canvas_draw_technique(OUTSIDE_SPACES[o], GS_CS_SRGB_16F), D);
		CHECK_INT(obs_canvas_draw_technique(OUTSIDE_SPACES[o], GS_CS_709_EXTENDED), D);
		CHECK_INT(obs_canvas_draw_technique(OUTSIDE_SPACES[o], GS_CS_709_SCRGB), M);
	}
}

/* --- canvas: white level and multiplier -------------------------------- */

/* Only a draw into scRGB reads the white level; a space outside the enum
 * does not. */
static void reads_white_level_spaces(void **state)
{
	(void)state;
	CHECK_BOOL(obs_canvas_draw_reads_white_level(GS_CS_SRGB), false);
	CHECK_BOOL(obs_canvas_draw_reads_white_level(GS_CS_SRGB_16F), false);
	CHECK_BOOL(obs_canvas_draw_reads_white_level(GS_CS_709_EXTENDED), false);
	CHECK_BOOL(obs_canvas_draw_reads_white_level(GS_CS_709_SCRGB), true);
	for (size_t o = 0; o < OUTSIDE_SPACE_COUNT; o++)
		CHECK_BOOL(obs_canvas_draw_reads_white_level(OUTSIDE_SPACES[o]), false);
}

/* The multiplier is the white level over scRGB's 80 nits: 1 at OBS's
 * default, 3.75 at 300, 0 at 0. */
static void multiplier_white_levels(void **state)
{
	(void)state;
	CHECK_BITS(obs_canvas_draw_multiplier(80.0f), 0x3f800000);  /* 1 */
	CHECK_BITS(obs_canvas_draw_multiplier(300.0f), 0x40700000); /* 3.75 */
	CHECK_BITS(obs_canvas_draw_multiplier(0.0f), 0x00000000);
	CHECK_BITS(obs_canvas_draw_multiplier(203.0f), float_bits(203.0f / 80.0f));
	CHECK_BITS(obs_canvas_draw_multiplier(10000.0f), 0x42fa0000); /* 125 */
}

/* One float division, whatever the level (obs_set_video_levels() takes any
 * float). Characterized, not endorsed: -0 stays -0, a negative level gives a
 * negative multiplier, the infinities stay infinite, a NaN stays a NaN, and
 * a denormal underflows to 0. */
static void multiplier_any_level(void **state)
{
	(void)state;
	CHECK_BITS(obs_canvas_draw_multiplier(-0.0f), 0x80000000);
	CHECK_BITS(obs_canvas_draw_multiplier(-80.0f), 0xbf800000);
	CHECK_BITS(obs_canvas_draw_multiplier(INFINITY), 0x7f800000);
	CHECK_BITS(obs_canvas_draw_multiplier(-INFINITY), 0xff800000);
	CHECK_BITS(obs_canvas_draw_multiplier(1e-45f), 0x00000000);
	CHECK_BITS(obs_canvas_draw_multiplier(FLT_MAX), float_bits(FLT_MAX / 80.0f));
	CHECK_BOOL(isnan(obs_canvas_draw_multiplier(NAN)), true);
	/* every multiple of 80 up to 80 * 2^16 divides exactly */
	for (uint32_t k = 0; k <= 65536; k++)
		CHECK_BITS(obs_canvas_draw_multiplier(80.0f * (float)k), float_bits((float)k));
}

/* The whole of obs_render_canvas_texture_internal()'s decision, as obs.c
 * asks it: the technique name it loops over, and the multiplier it sets
 * (1 unless the white level is read). */
static void canvas_plan_every_input(void **state)
{
	(void)state;
	static const char *const NAMES[4][4] = {
		{"Draw", "Draw", "Draw", "DrawMultiply"},
		{"Draw", "Draw", "Draw", "DrawMultiply"},
		{"DrawTonemap", "DrawTonemap", "Draw", "DrawMultiply"},
		{"Draw", "Draw", "Draw", "DrawMultiply"},
	};
	static const uint32_t MULTIPLIERS[WHITE_COUNT] = {0x3f800000, 0x40700000, 0x00000000};

	for (size_t s = 0; s < SPACE_COUNT; s++) {
		for (size_t c = 0; c < SPACE_COUNT; c++) {
			for (size_t w = 0; w < WHITE_COUNT; w++) {
				const char *name =
					obs_source_draw_technique_name(obs_canvas_draw_technique(SPACES[s], SPACES[c]));
				float multiplier = 1.f;
				if (obs_canvas_draw_reads_white_level(SPACES[c]))
					multiplier = obs_canvas_draw_multiplier(WHITES[w]);

				assert_string_equal(name, NAMES[s][c]);
				CHECK_BITS(multiplier, SPACES[c] == GS_CS_709_SCRGB ? MULTIPLIERS[w] : 0x3f800000);
			}
		}
	}
}

/* --- display: clear colour --------------------------------------------- */

/* The space is read only when the host does not always clear in sRGB. */
static void clear_reads_space_every_input(void **state)
{
	(void)state;
	CHECK_BOOL(obs_display_clear_reads_space(false), true);
	CHECK_BOOL(obs_display_clear_reads_space(true), false);
}

/* The background colour stays linear only into GS_CS_SRGB; GS_CS_SRGB_16F,
 * HDR and a space outside the enum convert it to sRGB. */
static void clear_linear_spaces(void **state)
{
	(void)state;
	CHECK_BOOL(obs_display_clear_linear(GS_CS_SRGB), true);
	CHECK_BOOL(obs_display_clear_linear(GS_CS_SRGB_16F), false);
	CHECK_BOOL(obs_display_clear_linear(GS_CS_709_EXTENDED), false);
	CHECK_BOOL(obs_display_clear_linear(GS_CS_709_SCRGB), false);
	for (size_t o = 0; o < OUTSIDE_SPACE_COUNT; o++)
		CHECK_BOOL(obs_display_clear_linear(OUTSIDE_SPACES[o]), false);
}

/* As render_display_begin() asks it: a host that always clears in sRGB
 * converts to sRGB in every space (Metal and OpenGL on Apple Silicon,
 * 87698b1e0); otherwise linear into GS_CS_SRGB only. */
static void clear_colour_every_input(void **state)
{
	(void)state;
	for (int always = 0; always < 2; always++) {
		for (size_t c = 0; c < SPACE_COUNT + OUTSIDE_SPACE_COUNT; c++) {
			const int space = c < SPACE_COUNT ? SPACES[c] : OUTSIDE_SPACES[c - SPACE_COUNT];
			const bool linear = obs_display_clear_reads_space(always) && obs_display_clear_linear(space);
			CHECK_BOOL(linear, !always && space == GS_CS_SRGB);
		}
	}
}

/* --- display: clear flags and the workaround ---------------------------- */

/* Depth and stencil are always cleared; colour only without the clear
 * workaround, which draws the colour with the solid effect instead. */
static void clear_every_input(void **state)
{
	(void)state;
	CHECK_INT(obs_display_clear_flags(false), GS_CLEAR_COLOR | GS_CLEAR_DEPTH | GS_CLEAR_STENCIL);
	CHECK_INT(obs_display_clear_flags(true), GS_CLEAR_DEPTH | GS_CLEAR_STENCIL);
	CHECK_BOOL(obs_display_clear_draws_solid(false), false);
	CHECK_BOOL(obs_display_clear_draws_solid(true), true);
	for (int workaround = 0; workaround < 2; workaround++) {
		const uint32_t flags = obs_display_clear_flags(workaround);
		/* the colour is either cleared or drawn, never both or neither */
		CHECK_BOOL(((flags & GS_CLEAR_COLOR) != 0) != obs_display_clear_draws_solid(workaround), true);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(constants),
		cmocka_unit_test(skip_every_input),
		cmocka_unit_test(technique_space_pairs),
		cmocka_unit_test(technique_outside_enum),
		cmocka_unit_test(reads_white_level_spaces),
		cmocka_unit_test(multiplier_white_levels),
		cmocka_unit_test(multiplier_any_level),
		cmocka_unit_test(canvas_plan_every_input),
		cmocka_unit_test(clear_reads_space_every_input),
		cmocka_unit_test(clear_linear_spaces),
		cmocka_unit_test(clear_colour_every_input),
		cmocka_unit_test(clear_every_input),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
