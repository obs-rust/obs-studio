#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <limits.h>
#include <cmocka.h>

#include "graphics/effect-param-types.h"

/*
 * Characterization of the number reads of the effect parser,
 * libobs/graphics/effect-parser.c, moved to
 * libobs/graphics/effect-param-numbers.c before their Rust port (#3464):
 * - obs_effect_param_int_value(): an int default value
 *   (ep_parse_param_assign_intfloat()), strtol() of the number token's text,
 *   negated when a '-' token came first, then cast to int;
 * - obs_effect_param_array_count(): the count of "name[count]"
 *   (ep_parse_param_array()), valid_int_str() on the token's length, then
 *   strtol() cast to int.
 * They pin the current C behaviour; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source into
 * itself and calls only them, through their header. effect-parser.c keeps the
 * tokens, the errors and the float values. The effect tests cover the callers
 * through the real libobs.
 *
 * A value outside int range depends on the width of long: strtol() saturates
 * at LONG_MAX, then the (int) cast keeps the low 32 bits where long is 64
 * bits (Linux, macOS), and nothing is cut where long is 32 bits (Windows).
 * Those cases assert the result of the host's width, and are characterized,
 * not endorsed.
 */

static void check_int(const char *str, int positive, int negative)
{
	assert_int_equal(obs_effect_param_int_value(str, false), positive);
	assert_int_equal(obs_effect_param_int_value(str, true), negative);
}

static void check_count(const char *str, size_t len, bool valid, int count)
{
	int got = -12345;

	assert_int_equal(obs_effect_param_array_count(str, len, &got), valid);
	assert_int_equal(got, valid ? count : -12345);
}

/* ------------------------------------------------------------------------ */

static void int_values(void **state)
{
	(void)state;

	check_int("0", 0, 0);
	check_int("5", 5, -5);
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
	check_int("12 34", 12, -12);
}

static void int_value_edges(void **state)
{
	(void)state;

	check_int("2147483647", INT_MAX, -INT_MAX);

	/* Past INT_MAX: characterized, not endorsed. */
#if LONG_MAX > INT_MAX
	check_int("2147483648", INT_MIN, INT_MIN);
	check_int("3000000000", -1294967296, 1294967296);
	check_int("4294967296", 0, 0);
	check_int("99999999999999999999", -1, 1);
#else
	check_int("2147483648", INT_MAX, -INT_MAX);
	check_int("3000000000", INT_MAX, -INT_MAX);
	check_int("4294967296", INT_MAX, -INT_MAX);
	check_int("99999999999999999999", INT_MAX, -INT_MAX);
#endif
}

static void array_counts(void **state)
{
	(void)state;

	check_count("0", 1, true, 0);
	check_count("4", 1, true, 4);
	check_count("4]", 1, true, 4);
	check_count("16", 2, true, 16);
}

static void array_count_rejects(void **state)
{
	(void)state;

	check_count("1.5", 3, false, 0);
	check_count("x", 1, false, 0);
	check_count("4x", 2, false, 0);
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
	/* valid_int_str() takes a sign. */
	check_count("-3", 2, true, -3);
}

static void array_count_edges(void **state)
{
	(void)state;

	check_count("2147483647", 10, true, INT_MAX);

	/* Past INT_MAX: characterized, not endorsed. */
#if LONG_MAX > INT_MAX
	check_count("3000000000", 10, true, -1294967296);
#else
	check_count("3000000000", 10, true, INT_MAX);
#endif
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(int_values),          cmocka_unit_test(int_value_reads_to_the_first_non_digit),
		cmocka_unit_test(int_value_edges),     cmocka_unit_test(array_counts),
		cmocka_unit_test(array_count_rejects), cmocka_unit_test(array_count_odd_lengths),
		cmocka_unit_test(array_count_edges),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
