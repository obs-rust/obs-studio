#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <limits.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>

#include <graphics/shader-parser.h>

/*
 * Characterization of the shader parser's number reads through the real
 * libobs (#3468): shader_parse() on a shader's text, then the array count and
 * the default value bytes of each parameter, and the parse errors. They pin
 * the current C behaviour; cases marked "characterized, not endorsed" are odd
 * but real.
 *
 * Only values inside int range: past it the C depends on the width of long
 * and the port changes it (test_shader_param_values_steps pins those against
 * the moved C).
 */

struct parsed {
	struct shader_parser sp;
	bool ok;
};

static void parse(struct parsed *p, const char *shader)
{
	shader_parser_init(&p->sp);
	p->ok = shader_parse(&p->sp, shader, "test_shader_number_defaults");
}

static struct shader_var *param(struct parsed *p, const char *name)
{
	struct shader_var *var = shader_parser_getparam(&p->sp, name);

	if (!var) {
		fprintf(stderr, "no parameter '%s'\n", name);
		fail();
	}
	return var;
}

static void check_errors(struct parsed *p, size_t count, const char *first)
{
	assert_int_equal(p->sp.cfp.error_list.errors.num, count);
	if (first) {
		assert_string_equal(p->sp.cfp.error_list.errors.array[0].error, first);
		assert_int_equal(p->sp.cfp.error_list.errors.array[0].level, LEX_ERROR);
	}
}

static void check_ints(struct shader_var *var, const int *values, size_t count)
{
	assert_int_equal(var->default_val.num, count * sizeof(int));
	for (size_t i = 0; i < count; i++) {
		int got;
		memcpy(&got, var->default_val.array + i * sizeof(int), sizeof(int));
		assert_int_equal(got, values[i]);
	}
}

/* Bit for bit, so a -0 keeps its sign. */
static void check_floats(struct shader_var *var, const float *values, size_t count)
{
	assert_int_equal(var->default_val.num, count * sizeof(float));
	for (size_t i = 0; i < count; i++) {
		if (memcmp(var->default_val.array + i * sizeof(float), values + i, sizeof(float)) != 0) {
			float got;
			memcpy(&got, var->default_val.array + i * sizeof(float), sizeof(float));
			fprintf(stderr, "%s value %zu: got %a, expected %a\n", var->name, i, got, values[i]);
			fail();
		}
	}
}

/* The expected values as a named array: no compound literals (MSVC). */
#define CHECK_INTS(var, ...)                                                \
	do {                                                                \
		static const int expected[] = {__VA_ARGS__};                \
		check_ints(var, expected, sizeof(expected) / sizeof(int));  \
	} while (false)
#define CHECK_FLOATS(var, ...)                                                  \
	do {                                                                    \
		static const float expected[] = {__VA_ARGS__};                  \
		check_floats(var, expected, sizeof(expected) / sizeof(float));  \
	} while (false)

/* ------------------------------------------------------------------------ */

static void array_counts(void **state)
{
	struct parsed p;

	(void)state;

	parse(&p, "uniform float d[4]; uniform int a[0]; uniform int b[007]; uniform float4 c[16]; uniform float e;");
	assert_true(p.ok);
	check_errors(&p, 0, NULL);
	assert_int_equal(p.sp.params.num, 5);
	assert_int_equal(param(&p, "d")->array_count, 4);
	assert_int_equal(param(&p, "a")->array_count, 0);
	assert_int_equal(param(&p, "b")->array_count, 7);
	assert_int_equal(param(&p, "c")->array_count, 16);
	assert_int_equal(param(&p, "e")->array_count, 0);
	assert_int_equal(param(&p, "d")->default_val.num, 0);
	shader_parser_free(&p.sp);

	parse(&p, "uniform int b[2147483647];");
	assert_true(p.ok);
	assert_int_equal(param(&p, "b")->array_count, INT_MAX);
	shader_parser_free(&p.sp);
}

static void array_count_not_an_int(void **state)
{
	struct parsed p;

	(void)state;

	/* The parameter is dropped without an error of its own; the parser then
	 * fails on what follows. */
	parse(&p, "uniform float e[1.5];");
	assert_false(p.ok);
	assert_int_equal(p.sp.params.num, 0);
	check_errors(&p, 1, "Expected 'type'");
	shader_parser_free(&p.sp);
}

static void int_defaults(void **state)
{
	struct parsed p;

	(void)state;

	parse(&p, "uniform int a = 5; uniform int b = -5; uniform int c = 007; uniform int g = 2147483647;"
		  " uniform int h = -2147483647; uniform int z = 0; uniform int m = -0;");
	assert_true(p.ok);
	check_errors(&p, 0, NULL);
	CHECK_INTS(param(&p, "a"), 5);
	CHECK_INTS(param(&p, "b"), -5);
	CHECK_INTS(param(&p, "c"), 7);
	CHECK_INTS(param(&p, "g"), INT_MAX);
	CHECK_INTS(param(&p, "h"), -INT_MAX);
	CHECK_INTS(param(&p, "z"), 0);
	CHECK_INTS(param(&p, "m"), 0);
	shader_parser_free(&p.sp);
}

static void int_default_of_a_float(void **state)
{
	struct parsed p;

	(void)state;

	/* A float token reads as its integer part (characterized, not
	 * endorsed). */
	parse(&p, "uniform int d = 1.5; uniform int e = -1e3;");
	assert_true(p.ok);
	check_errors(&p, 0, NULL);
	CHECK_INTS(param(&p, "d"), 1);
	CHECK_INTS(param(&p, "e"), -1);
	shader_parser_free(&p.sp);
}

static void int_default_not_a_number(void **state)
{
	struct parsed p;

	(void)state;

	parse(&p, "uniform int i = x;");
	assert_false(p.ok);
	assert_int_equal(p.sp.params.num, 0);
	check_errors(&p, 1, "Expected 'numeric value'");
	shader_parser_free(&p.sp);
}

static void float_defaults(void **state)
{
	struct parsed p;

	(void)state;

	parse(&p, "uniform float a = 1.5; uniform float b = -2.5; uniform float c = 1e3; uniform float d = .5;"
		  " uniform float e = -0; uniform float f = 0.1; uniform float g = 7;");
	assert_true(p.ok);
	check_errors(&p, 0, NULL);
	CHECK_FLOATS(param(&p, "a"), 1.5f);
	CHECK_FLOATS(param(&p, "b"), -2.5f);
	CHECK_FLOATS(param(&p, "c"), 1000.0f);
	CHECK_FLOATS(param(&p, "d"), 0.5f);
	CHECK_FLOATS(param(&p, "e"), -0.0f);
	CHECK_FLOATS(param(&p, "f"), (float)0.1);
	CHECK_FLOATS(param(&p, "g"), 7.0f);
	shader_parser_free(&p.sp);
}

static void float_array_defaults(void **state)
{
	struct parsed p;

	(void)state;

	parse(&p, "uniform float2 v = {1, -2}; uniform float2x2 w = {-1, 2.5, -0.5, 4};"
		  " uniform float4x4 m = {1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16};");
	assert_true(p.ok);
	check_errors(&p, 0, NULL);
	CHECK_FLOATS(param(&p, "v"), 1.0f, -2.0f);
	CHECK_FLOATS(param(&p, "w"), -1.0f, 2.5f, -0.5f, 4.0f);
	CHECK_FLOATS(param(&p, "m"), 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16);
	shader_parser_free(&p.sp);
}

static void float_array_bad_counts(void **state)
{
	struct parsed p;

	(void)state;

	/* A row or column digit outside 1-4 adds its error, but the digit still
	 * counts, and the values that many are kept (characterized, not
	 * endorsed). */
	parse(&p, "uniform float5 p = {1, 2, 3, 4, 5};");
	assert_false(p.ok);
	check_errors(&p, 1, "Invalid row count");
	CHECK_FLOATS(param(&p, "p"), 1, 2, 3, 4, 5);
	shader_parser_free(&p.sp);

	parse(&p, "uniform float2x9 q = {1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18};");
	assert_false(p.ok);
	check_errors(&p, 1, "Invalid column count");
	CHECK_FLOATS(param(&p, "q"), 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18);
	shader_parser_free(&p.sp);

	/* "float1x": the column is the terminating NUL, a count of -48, so no
	 * value is read and the parser goes on from the '{' (characterized, not
	 * endorsed). */
	parse(&p, "uniform float1x r = {1}; uniform float s = 2;");
	assert_false(p.ok);
	check_errors(&p, 2, "Invalid column count");
	assert_string_equal(p.sp.cfp.error_list.errors.array[1].error, "Expected 'type'");
	assert_int_equal(p.sp.params.num, 1);
	CHECK_FLOATS(param(&p, "s"), 2.0f);
	shader_parser_free(&p.sp);
}

static void float_array_too_many_values(void **state)
{
	struct parsed p;

	(void)state;

	parse(&p, "uniform float2 t = {1, 2, 3}; uniform float u = 4;");
	assert_false(p.ok);
	check_errors(&p, 1, "Expected '}'");
	assert_int_equal(p.sp.params.num, 1);
	CHECK_FLOATS(param(&p, "u"), 4.0f);
	shader_parser_free(&p.sp);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(array_counts),
		cmocka_unit_test(array_count_not_an_int),
		cmocka_unit_test(int_defaults),
		cmocka_unit_test(int_default_of_a_float),
		cmocka_unit_test(int_default_not_a_number),
		cmocka_unit_test(float_defaults),
		cmocka_unit_test(float_array_defaults),
		cmocka_unit_test(float_array_bad_counts),
		cmocka_unit_test(float_array_too_many_values),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
