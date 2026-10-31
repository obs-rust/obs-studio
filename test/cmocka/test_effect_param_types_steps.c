#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <cmocka.h>

#include "graphics/effect-param-types.h"

/*
 * Characterization of the parameter type decisions of the effect parser,
 * libobs/graphics/effect-parser.c, moved to
 * libobs/graphics/effect-param-types.c before their Rust port (#3162):
 * - obs_effect_param_type(): the shader parameter type of a type name
 *   (get_effect_param_type(), for ep_compile_annotations() and
 *   ep_compile_param());
 * - obs_effect_param_route(): the default-value parser a parameter gets
 *   (ep_parse_param_assignment_val());
 * - obs_effect_param_rows_bad(), obs_effect_param_rows(),
 *   obs_effect_param_has_cols(), obs_effect_param_cols_bad() and
 *   obs_effect_param_cols(): the value count of an int or float array type,
 *   and whether its row or column digit is outside 1-4
 *   (ep_parse_param_assign_intfloat_array()), asked in its order by
 *   check_shape().
 * They pin the current C behaviour; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source into
 * itself and calls only them, through their header. effect-parser.c keeps the
 * tokens, the errors and the value parsers. The effect tests cover the
 * callers through the real libobs.
 *
 * Every input is defined in C: an array type name is longer than its prefix,
 * as the route gives it. Non-ASCII digit bytes are left out, since their
 * count follows the target's char signedness.
 */

/* The shape as ep_parse_param_assign_intfloat_array() asks it: the column
 * digit only after an 'x'. */
static void check_shape(const char *type, bool is_float, int count, bool bad_rows, bool bad_cols)
{
	bool cols_bad = false;
	int n;

	assert_int_equal(obs_effect_param_rows_bad(type, is_float), bad_rows);
	n = obs_effect_param_rows(type, is_float);
	if (obs_effect_param_has_cols(type, is_float)) {
		cols_bad = obs_effect_param_cols_bad(type, is_float);
		n = obs_effect_param_cols(type, is_float, n);
	}
	assert_int_equal(cols_bad, bad_cols);
	assert_int_equal(n, count);
}

/* ------------------------------------------------------------------------ */

static void every_exact_type_name(void **state)
{
	(void)state;

	assert_int_equal(obs_effect_param_type("float"), GS_SHADER_PARAM_FLOAT);
	assert_int_equal(obs_effect_param_type("float2"), GS_SHADER_PARAM_VEC2);
	assert_int_equal(obs_effect_param_type("float3"), GS_SHADER_PARAM_VEC3);
	assert_int_equal(obs_effect_param_type("float4"), GS_SHADER_PARAM_VEC4);
	assert_int_equal(obs_effect_param_type("int2"), GS_SHADER_PARAM_INT2);
	assert_int_equal(obs_effect_param_type("int3"), GS_SHADER_PARAM_INT3);
	assert_int_equal(obs_effect_param_type("int4"), GS_SHADER_PARAM_INT4);
	assert_int_equal(obs_effect_param_type("float4x4"), GS_SHADER_PARAM_MATRIX4X4);
	assert_int_equal(obs_effect_param_type("bool"), GS_SHADER_PARAM_BOOL);
	assert_int_equal(obs_effect_param_type("int"), GS_SHADER_PARAM_INT);
	assert_int_equal(obs_effect_param_type("string"), GS_SHADER_PARAM_STRING);
}

static void any_texture_prefix_is_a_texture(void **state)
{
	(void)state;

	assert_int_equal(obs_effect_param_type("texture"), GS_SHADER_PARAM_TEXTURE);
	assert_int_equal(obs_effect_param_type("texture2d"), GS_SHADER_PARAM_TEXTURE);
	assert_int_equal(obs_effect_param_type("texture3d"), GS_SHADER_PARAM_TEXTURE);
	assert_int_equal(obs_effect_param_type("texture_rect"), GS_SHADER_PARAM_TEXTURE);
	/* characterized, not endorsed: any name that starts with it */
	assert_int_equal(obs_effect_param_type("textures"), GS_SHADER_PARAM_TEXTURE);
	assert_int_equal(obs_effect_param_type("texturefloat4"), GS_SHADER_PARAM_TEXTURE);
	/* a cut of the prefix is not */
	assert_int_equal(obs_effect_param_type("textur"), GS_SHADER_PARAM_UNKNOWN);
}

static void other_type_names_are_unknown(void **state)
{
	(void)state;

	assert_int_equal(obs_effect_param_type(""), GS_SHADER_PARAM_UNKNOWN);
	/* case matters */
	assert_int_equal(obs_effect_param_type("Float"), GS_SHADER_PARAM_UNKNOWN);
	assert_int_equal(obs_effect_param_type("BOOL"), GS_SHADER_PARAM_UNKNOWN);
	assert_int_equal(obs_effect_param_type("Texture2d"), GS_SHADER_PARAM_UNKNOWN);
	/* exact names only: a suffix or a cut is unknown */
	assert_int_equal(obs_effect_param_type("floats"), GS_SHADER_PARAM_UNKNOWN);
	assert_int_equal(obs_effect_param_type("float5"), GS_SHADER_PARAM_UNKNOWN);
	assert_int_equal(obs_effect_param_type("floa"), GS_SHADER_PARAM_UNKNOWN);
	assert_int_equal(obs_effect_param_type("int1"), GS_SHADER_PARAM_UNKNOWN);
	assert_int_equal(obs_effect_param_type("float3x3"), GS_SHADER_PARAM_UNKNOWN);
	assert_int_equal(obs_effect_param_type("float4x4 "), GS_SHADER_PARAM_UNKNOWN);
	assert_int_equal(obs_effect_param_type("strings"), GS_SHADER_PARAM_UNKNOWN);
	assert_int_equal(obs_effect_param_type("sampler"), GS_SHADER_PARAM_UNKNOWN);
}

static void every_route(void **state)
{
	(void)state;

	assert_int_equal(obs_effect_param_route("int", false), OBS_EFFECT_PARAM_ROUTE_INT);
	assert_int_equal(obs_effect_param_route("float", false), OBS_EFFECT_PARAM_ROUTE_FLOAT);
	assert_int_equal(obs_effect_param_route("int2", false), OBS_EFFECT_PARAM_ROUTE_INT_ARRAY);
	assert_int_equal(obs_effect_param_route("int4", false), OBS_EFFECT_PARAM_ROUTE_INT_ARRAY);
	assert_int_equal(obs_effect_param_route("float2", false), OBS_EFFECT_PARAM_ROUTE_FLOAT_ARRAY);
	assert_int_equal(obs_effect_param_route("float4x4", false), OBS_EFFECT_PARAM_ROUTE_FLOAT_ARRAY);
	assert_int_equal(obs_effect_param_route("string", false), OBS_EFFECT_PARAM_ROUTE_STRING);
	assert_int_equal(obs_effect_param_route("bool", false), OBS_EFFECT_PARAM_ROUTE_BOOL);
	assert_int_equal(obs_effect_param_route("", false), OBS_EFFECT_PARAM_ROUTE_INVALID);
	assert_int_equal(obs_effect_param_route("sampler", false), OBS_EFFECT_PARAM_ROUTE_INVALID);
	assert_int_equal(obs_effect_param_route("Float", false), OBS_EFFECT_PARAM_ROUTE_INVALID);
	assert_int_equal(obs_effect_param_route("bools", false), OBS_EFFECT_PARAM_ROUTE_INVALID);
	assert_int_equal(obs_effect_param_route("in", false), OBS_EFFECT_PARAM_ROUTE_INVALID);
}

static void the_texture_flag_comes_first(void **state)
{
	(void)state;

	/* the flag decides, whatever the name */
	assert_int_equal(obs_effect_param_route("texture2d", true), OBS_EFFECT_PARAM_ROUTE_TEXTURE);
	assert_int_equal(obs_effect_param_route("int", true), OBS_EFFECT_PARAM_ROUTE_TEXTURE);
	assert_int_equal(obs_effect_param_route("bool", true), OBS_EFFECT_PARAM_ROUTE_TEXTURE);
	assert_int_equal(obs_effect_param_route("", true), OBS_EFFECT_PARAM_ROUTE_TEXTURE);
	/* without it a texture name has no default-value parser */
	assert_int_equal(obs_effect_param_route("texture2d", false), OBS_EFFECT_PARAM_ROUTE_INVALID);
}

static void routes_that_disagree_with_the_type(void **state)
{
	(void)state;

	/* characterized, not endorsed: the string parser, but an unknown type */
	assert_int_equal(obs_effect_param_route("strings", false), OBS_EFFECT_PARAM_ROUTE_STRING);
	assert_int_equal(obs_effect_param_type("strings"), GS_SHADER_PARAM_UNKNOWN);
	/* characterized, not endorsed: array values, but an unknown type */
	assert_int_equal(obs_effect_param_route("int1", false), OBS_EFFECT_PARAM_ROUTE_INT_ARRAY);
	assert_int_equal(obs_effect_param_type("int1"), GS_SHADER_PARAM_UNKNOWN);
	assert_int_equal(obs_effect_param_route("float3x3", false), OBS_EFFECT_PARAM_ROUTE_FLOAT_ARRAY);
	assert_int_equal(obs_effect_param_type("float3x3"), GS_SHADER_PARAM_UNKNOWN);
	/* characterized, not endorsed: any int or float prefix is an array */
	assert_int_equal(obs_effect_param_route("integer", false), OBS_EFFECT_PARAM_ROUTE_INT_ARRAY);
	assert_int_equal(obs_effect_param_route("floatx", false), OBS_EFFECT_PARAM_ROUTE_FLOAT_ARRAY);
}

static void valid_shapes(void **state)
{
	(void)state;

	check_shape("float2", true, 2, false, false);
	check_shape("float4", true, 4, false, false);
	check_shape("int1", false, 1, false, false);
	check_shape("int3", false, 3, false, false);
	check_shape("float4x4", true, 16, false, false);
	check_shape("float3x2", true, 6, false, false);
	check_shape("int1x4", false, 4, false, false);
	/* only the byte after the row digit is looked at for an 'x' */
	check_shape("float4y4", true, 4, false, false);
	check_shape("float4x4x4", true, 16, false, false);
}

static void bad_row_digits(void **state)
{
	(void)state;

	/* characterized, not endorsed: the error is added, the digit still
	 * counts */
	check_shape("float5", true, 5, true, false);
	check_shape("float0", true, 0, true, false);
	check_shape("int9", false, 9, true, false);
	check_shape("float0x4", true, 0, true, false);
	/* characterized, not endorsed: 'x' - '0' */
	check_shape("floatx", true, 72, true, false);
	check_shape("integer", false, 'e' - '0', true, false);
	check_shape("float_", true, '_' - '0', true, false);
}

static void bad_column_digits(void **state)
{
	(void)state;

	/* characterized, not endorsed: the error is added, the digit still
	 * counts, also as the terminator: '\0' - '0' */
	check_shape("float4x", true, 4 * -'0', false, true);
	check_shape("float4x9", true, 36, false, true);
	check_shape("float4x0", true, 0, false, true);
	check_shape("int2x5", false, 10, false, true);
	check_shape("float5x5", true, 25, true, true);
	check_shape("floatxx", true, 72 * -'0', true, true);
	check_shape("floatxxx", true, 72 * 72, true, true);
}

static void each_dimension_alone(void **state)
{
	(void)state;

	/* the column scales whatever count the caller passes */
	assert_int_equal(obs_effect_param_cols("float4x3", true, 5), 15);
	assert_int_equal(obs_effect_param_cols("int2x4", false, -1), -4);
	/* only the byte right after the row digit is an 'x' */
	assert_true(obs_effect_param_has_cols("float4x4", true));
	assert_false(obs_effect_param_has_cols("float4y4", true));
	assert_false(obs_effect_param_has_cols("float4", true));
	/* the prefix length follows is_float, not the name */
	assert_int_equal(obs_effect_param_rows("float4", false), 'a' - '0');
	assert_true(obs_effect_param_rows_bad("float4", false));
	assert_false(obs_effect_param_cols_bad("int1x4", false));
	assert_true(obs_effect_param_cols_bad("int1x5", false));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(every_exact_type_name),
		cmocka_unit_test(any_texture_prefix_is_a_texture),
		cmocka_unit_test(other_type_names_are_unknown),
		cmocka_unit_test(every_route),
		cmocka_unit_test(the_texture_flag_comes_first),
		cmocka_unit_test(routes_that_disagree_with_the_type),
		cmocka_unit_test(valid_shapes),
		cmocka_unit_test(bad_row_digits),
		cmocka_unit_test(bad_column_digits),
		cmocka_unit_test(each_dimension_alone),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
