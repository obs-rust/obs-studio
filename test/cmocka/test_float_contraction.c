#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <cmocka.h>

/* The test build must not fuse a multiply and an add into one fused
 * multiply-add (#3422). Rust never contracts `a - b * c` on its own, and the
 * differential tests compare C and Rust floats exactly, so a test build
 * whose compiler contracts (the hosted runners' gcc, clang on arm64) gives
 * results that a build without contraction (the Tier 2 image) does not.
 * The build adds -ffp-contract=off to every C and C++ target when
 * ENABLE_UNIT_TESTS is on; this target has no flag of its own, so it fails
 * whenever that build-wide flag is missing on a host that would contract.
 *
 * The case is the one that found it: obs_source_transition_render_offset()
 * computes 1920 - 1000 * 1.08f, which is 840.0 when the product is rounded
 * to float first and 839.99994 when fused. volatile keeps the compiler from
 * folding it at compile time. */

static volatile float base = 1920.0f;
static volatile float step = 1000.0f;
static volatile float scale = 1.08f;

static volatile double base_d = 1.0;
static volatile double third_d = 1.0 / 3.0;
static volatile double three_d = 3.0;

static void float_multiply_subtract_is_not_fused(void **state)
{
	(void)state;

	float product = step * scale;
	float rounded_first = base - product;
	float expression = base - step * scale;

	assert_true(rounded_first == 840.0f);
	assert_true(expression == 840.0f);
	assert_true(expression == rounded_first);
}

static void double_multiply_add_is_not_fused(void **state)
{
	(void)state;

	/* 1/3 rounds down, so 3 * (1/3) rounds to exactly 1.0 and the
	 * difference is 0; fused, the exact product's tail is kept and the
	 * result is -2^-54. */
	double rounded_first = three_d * third_d;
	double expression = three_d * third_d - base_d;

	assert_true(rounded_first - base_d == 0.0);
	assert_true(expression == 0.0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(float_multiply_subtract_is_not_fused),
		cmocka_unit_test(double_multiply_add_is_not_fused),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
