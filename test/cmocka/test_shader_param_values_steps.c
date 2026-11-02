#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <limits.h>
#include <math.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>

#include "graphics/shader-param-values.h"

/*
 * Characterization of the number reads of the shader parser,
 * libobs/graphics/shader-parser.c, moved to
 * libobs/graphics/shader-param-values.c before their Rust port (#3468):
 * - shader_param_array_count(): the count of "name[count]"
 *   (sp_parse_param_array()), valid_int_str() on the token's length, then
 *   strtol() cast to int;
 * - shader_param_int_value(): an int default value
 *   (sp_parse_param_assign_intfloat()), strtol() of the number token's text,
 *   negated when a '-' token came first, then cast to int;
 * - shader_param_float_value(): a float default value, os_strtod() of the
 *   text cast to float, negated when a '-' token came first.
 * They pin the current C behaviour; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source into
 * itself and calls only them, through their header. shader-parser.c keeps the
 * tokens, the errors and the value pushes; test_shader_number_defaults covers
 * the callers through the real libobs.
 *
 * A value outside int range depends on the width of long: strtol() saturates
 * at LONG_MAX, then the (int) cast keeps the low 32 bits where long is 64
 * bits (Linux, macOS), and nothing is cut where long is 32 bits (Windows).
 * Those cases assert the result of the host's width, and are characterized,
 * not endorsed.
 */

static void check_count(const char *str, size_t len, bool valid, int count)
{
	int got = -12345;

	assert_int_equal(shader_param_array_count(str, len, &got), valid);
	assert_int_equal(got, valid ? count : -12345);
}

static void check_int(const char *str, int positive, int negative)
{
	assert_int_equal(shader_param_int_value(str, false), positive);
	assert_int_equal(shader_param_int_value(str, true), negative);
}

/* The value and its negation, bit for bit (a -0 keeps its sign). */
static void check_float(const char *str, float positive)
{
	float negative = -positive;
	float got = shader_param_float_value(str, false);
	float got_negative = shader_param_float_value(str, true);

	if (memcmp(&got, &positive, sizeof(float)) != 0 || memcmp(&got_negative, &negative, sizeof(float)) != 0) {
		fprintf(stderr, "\"%s\": got %a and %a, expected %a and %a\n", str, got, got_negative, positive,
			negative);
		fail();
	}
}

/* ------------------------------------------------------------------------ */

static void array_counts(void **state)
{
	(void)state;

	check_count("0", 1, true, 0);
	check_count("7", 1, true, 7);
	check_count("007", 3, true, 7);
	check_count("16", 2, true, 16);
	check_count("4]", 1, true, 4);
	/* valid_int_str() takes a sign (characterized, not endorsed: the shader
	 * tokens never hold one). */
	check_count("+7", 2, true, 7);
	check_count("-3", 2, true, -3);
}

static void array_count_rejects(void **state)
{
	(void)state;

	check_count("1.5", 3, false, 0);
	check_count("1e3", 3, false, 0);
	check_count("x", 1, false, 0);
	check_count("4x", 2, false, 0);
	check_count("+", 1, false, 0);
	check_count("", 1, false, 0);
}

static void array_count_odd_lengths(void **state)
{
	(void)state;

	/* Characterized, not endorsed: valid_int_str() checks only len bytes,
	 * and strtol() reads every digit after them. */
	check_count("4567", 1, true, 4567);
	check_count("4x", 1, true, 4);
	/* A zero length checks the whole string. */
	check_count("12", 0, true, 12);
	check_count("1x", 0, false, 0);
}

static void array_count_edges(void **state)
{
	(void)state;

	check_count("2147483647", 10, true, INT_MAX);

	/* Past INT_MAX: characterized, not endorsed. */
#if LONG_MAX > INT_MAX
	check_count("2147483648", 10, true, INT_MIN);
	check_count("4294967297", 10, true, 1);
	check_count("99999999999999999999", 20, true, -1);
#else
	check_count("2147483648", 10, true, INT_MAX);
	check_count("4294967297", 10, true, INT_MAX);
	check_count("99999999999999999999", 20, true, INT_MAX);
#endif
}

static void int_values(void **state)
{
	(void)state;

	check_int("0", 0, 0);
	check_int("7", 7, -7);
	check_int("+7", 7, -7);
	check_int("007", 7, -7);
	check_int("123456", 123456, -123456);
}

static void int_value_reads_to_the_first_non_digit(void **state)
{
	(void)state;

	/* The token's text runs on into the source: strtol() stops at the first
	 * byte that is not a digit, so a float token reads as its integer part
	 * (characterized, not endorsed). */
	check_int("1.5", 1, -1);
	check_int("1e3", 1, -1);
	check_int("0x10", 0, 0);
	check_int("12;", 12, -12);
	check_int("4}", 4, -4);
	check_int("x", 0, 0);
}

static void int_value_edges(void **state)
{
	(void)state;

	check_int("2147483647", INT_MAX, -INT_MAX);

	/* Past INT_MAX: characterized, not endorsed. */
#if LONG_MAX > INT_MAX
	check_int("2147483648", INT_MIN, INT_MIN);
	check_int("4294967297", 1, -1);
	check_int("99999999999999999999", -1, 1);
#else
	check_int("2147483648", INT_MAX, -INT_MAX);
	check_int("4294967297", INT_MAX, -INT_MAX);
	check_int("99999999999999999999", INT_MAX, -INT_MAX);
#endif
}

static void float_values(void **state)
{
	(void)state;

	check_float("1.5", 1.5f);
	check_float("1e3", 1000.0f);
	check_float(".5", 0.5f);
	check_float("0.1", (float)0.1);
	check_float("7", 7.0f);
	check_float("2.5}", 2.5f);
	check_float("0x10", 16.0f);
	/* A '-' token before a zero gives -0. */
	check_float("0", 0.0f);
	check_float("x", 0.0f);
}

static void float_value_edges(void **state)
{
	(void)state;

	/* Past FLT_MAX the double becomes an infinity, and below the smallest
	 * float a zero (characterized, not endorsed). */
	check_float("3.4e39", INFINITY);
	check_float("1e-50", 0.0f);
}

static void float_value_reads_63_bytes(void **state)
{
	char text[80];

	(void)state;

	/* os_strtod() copies at most 63 bytes before it converts: "1.000...e1"
	 * reads as 10 while its '1' is byte 63, and as 1 once it is byte 64
	 * (characterized, not endorsed). */
	memset(text, '0', sizeof(text));
	memcpy(text, "1.", 2);
	memcpy(text + 61, "e1", 3);
	assert_int_equal(strlen(text), 63);
	check_float(text, 10.0f);

	memcpy(text + 61, "0e1", 4);
	assert_int_equal(strlen(text), 64);
	check_float(text, 1.0f);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(array_counts),
		cmocka_unit_test(array_count_rejects),
		cmocka_unit_test(array_count_odd_lengths),
		cmocka_unit_test(array_count_edges),
		cmocka_unit_test(int_values),
		cmocka_unit_test(int_value_reads_to_the_first_non_digit),
		cmocka_unit_test(int_value_edges),
		cmocka_unit_test(float_values),
		cmocka_unit_test(float_value_edges),
		cmocka_unit_test(float_value_reads_63_bytes),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
