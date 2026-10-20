#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <cmocka.h>

/*
 * Characterization tests for the scene item blend mode table in
 * libobs/obs-scene-blend.c (#1520): obs_blend_mode_params, which maps each
 * enum obs_blending_type to the blend factors and operation that
 * render_item_texture() in obs-scene.c passes to gs_blend_function_separate()
 * and gs_blend_op(). They pin the current C behavior for the Rust port; cases
 * marked "characterized, not endorsed" are odd but real.
 *
 * The table is static, so its file is compiled into this test. It needs no
 * graphics device and no libobs core. test_darken_transparent covers Darken
 * drawn end to end.
 */

#include "libobs/obs-scene-blend.c.h"

#define NUM_PARAMS (sizeof(obs_blend_mode_params) / sizeof(obs_blend_mode_params[0]))

struct expected_blend {
	enum obs_blending_type mode;
	const char *name;
	enum gs_blend_type src_color;
	enum gs_blend_type src_alpha;
	enum gs_blend_type dst_color;
	enum gs_blend_type dst_alpha;
	enum gs_blend_op_type op;
};

static const struct expected_blend expected[] = {
	{OBS_BLEND_NORMAL, "normal", GS_BLEND_ONE, GS_BLEND_ONE, GS_BLEND_INVSRCALPHA, GS_BLEND_INVSRCALPHA,
	 GS_BLEND_OP_ADD},
	{OBS_BLEND_ADDITIVE, "additive", GS_BLEND_ONE, GS_BLEND_ONE, GS_BLEND_ONE, GS_BLEND_ONE, GS_BLEND_OP_ADD},
	{OBS_BLEND_SUBTRACT, "subtract", GS_BLEND_ONE, GS_BLEND_ONE, GS_BLEND_ONE, GS_BLEND_ONE,
	 GS_BLEND_OP_REVERSE_SUBTRACT},
	{OBS_BLEND_SCREEN, "screen", GS_BLEND_ONE, GS_BLEND_ONE, GS_BLEND_INVSRCCOLOR, GS_BLEND_INVSRCALPHA,
	 GS_BLEND_OP_ADD},
	{OBS_BLEND_MULTIPLY, "multiply", GS_BLEND_DSTCOLOR, GS_BLEND_DSTALPHA, GS_BLEND_INVSRCALPHA,
	 GS_BLEND_INVSRCALPHA, GS_BLEND_OP_ADD},
	{OBS_BLEND_LIGHTEN, "lighten", GS_BLEND_ONE, GS_BLEND_ONE, GS_BLEND_ONE, GS_BLEND_ONE, GS_BLEND_OP_MAX},
	{OBS_BLEND_DARKEN, "darken", GS_BLEND_ONE, GS_BLEND_ONE, GS_BLEND_ONE, GS_BLEND_ONE, GS_BLEND_OP_MIN},
};

#define NUM_EXPECTED (sizeof(expected) / sizeof(expected[0]))

/* The table is indexed by the blend mode itself, so the public enum values are
 * the table's row numbers. */
static void test_blend_modes_are_row_numbers(void **state)
{
	(void)state;

	assert_int_equal(OBS_BLEND_NORMAL, 0);
	assert_int_equal(OBS_BLEND_ADDITIVE, 1);
	assert_int_equal(OBS_BLEND_SUBTRACT, 2);
	assert_int_equal(OBS_BLEND_SCREEN, 3);
	assert_int_equal(OBS_BLEND_MULTIPLY, 4);
	assert_int_equal(OBS_BLEND_LIGHTEN, 5);
	assert_int_equal(OBS_BLEND_DARKEN, 6);
}

/* One row per blend mode and no more. Characterized, not endorsed: the lookup
 * is an unchecked index (obs_blend_mode_params[item->blend_type]), and
 * obs_sceneitem_set_blending_mode() stores any value it is given, so a mode
 * past OBS_BLEND_DARKEN has no row. Scene loading and obs-websocket only
 * produce the seven modes. */
static void test_one_row_per_mode(void **state)
{
	(void)state;

	assert_int_equal(NUM_PARAMS, OBS_BLEND_DARKEN + 1);
	assert_int_equal(NUM_PARAMS, NUM_EXPECTED);
}

static void test_every_mode_factors_and_op(void **state)
{
	(void)state;

	for (size_t i = 0; i < NUM_EXPECTED; i++) {
		const struct expected_blend *e = &expected[i];

		assert_int_equal((size_t)e->mode, i);
		print_message("blend mode %s\n", e->name);
		assert_int_equal(obs_blend_mode_params[e->mode].src_color, e->src_color);
		assert_int_equal(obs_blend_mode_params[e->mode].src_alpha, e->src_alpha);
		assert_int_equal(obs_blend_mode_params[e->mode].dst_color, e->dst_color);
		assert_int_equal(obs_blend_mode_params[e->mode].dst_alpha, e->dst_alpha);
		assert_int_equal(obs_blend_mode_params[e->mode].op, e->op);
	}
}

/* Normal is premultiplied-alpha "over": one, inverse source alpha, add. */
static void test_normal_is_premultiplied_over(void **state)
{
	(void)state;

	assert_int_equal(obs_blend_mode_params[OBS_BLEND_NORMAL].src_color, GS_BLEND_ONE);
	assert_int_equal(obs_blend_mode_params[OBS_BLEND_NORMAL].dst_color, GS_BLEND_INVSRCALPHA);
	assert_int_equal(obs_blend_mode_params[OBS_BLEND_NORMAL].op, GS_BLEND_OP_ADD);
}

/* Additive, Subtract, Lighten and Darken use factors of one for color and
 * alpha alike and differ only in the operation; the min and max operations
 * ignore the factors. Characterized, not endorsed: alpha goes through the same
 * operation as color, so Subtract gives dst_a - src_a (an opaque item over an
 * opaque background leaves alpha 0) and Darken gives min(src_a, dst_a). */
static void test_factor_one_modes_differ_by_op(void **state)
{
	(void)state;

	const enum obs_blending_type modes[] = {OBS_BLEND_ADDITIVE, OBS_BLEND_SUBTRACT, OBS_BLEND_LIGHTEN,
						OBS_BLEND_DARKEN};
	const enum gs_blend_op_type ops[] = {GS_BLEND_OP_ADD, GS_BLEND_OP_REVERSE_SUBTRACT, GS_BLEND_OP_MAX,
					     GS_BLEND_OP_MIN};

	for (size_t i = 0; i < sizeof(modes) / sizeof(modes[0]); i++) {
		assert_int_equal(obs_blend_mode_params[modes[i]].src_color, GS_BLEND_ONE);
		assert_int_equal(obs_blend_mode_params[modes[i]].src_alpha, GS_BLEND_ONE);
		assert_int_equal(obs_blend_mode_params[modes[i]].dst_color, GS_BLEND_ONE);
		assert_int_equal(obs_blend_mode_params[modes[i]].dst_alpha, GS_BLEND_ONE);
		assert_int_equal(obs_blend_mode_params[modes[i]].op, ops[i]);
	}
}

/* Screen and Multiply split color from alpha. Characterized, not endorsed:
 * Multiply's alpha is src_a * dst_a + dst_a * (1 - src_a) = dst_a, so the item
 * adds no alpha of its own (over a transparent background it leaves alpha 0). */
static void test_screen_and_multiply_split_color_and_alpha(void **state)
{
	(void)state;

	assert_int_equal(obs_blend_mode_params[OBS_BLEND_SCREEN].dst_color, GS_BLEND_INVSRCCOLOR);
	assert_int_equal(obs_blend_mode_params[OBS_BLEND_SCREEN].dst_alpha, GS_BLEND_INVSRCALPHA);
	assert_int_equal(obs_blend_mode_params[OBS_BLEND_MULTIPLY].src_color, GS_BLEND_DSTCOLOR);
	assert_int_equal(obs_blend_mode_params[OBS_BLEND_MULTIPLY].src_alpha, GS_BLEND_DSTALPHA);
	assert_int_equal(obs_blend_mode_params[OBS_BLEND_MULTIPLY].dst_alpha, GS_BLEND_INVSRCALPHA);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_blend_modes_are_row_numbers),
		cmocka_unit_test(test_one_row_per_mode),
		cmocka_unit_test(test_every_mode_factors_and_op),
		cmocka_unit_test(test_normal_is_premultiplied_over),
		cmocka_unit_test(test_factor_one_modes_differ_by_op),
		cmocka_unit_test(test_screen_and_multiply_split_color_and_alpha),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
