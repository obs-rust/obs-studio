#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <stdlib.h>
#include <cmocka.h>

#ifdef GS_PQ_STEPS_TEST_RUST
/* test_graphics_ffmpeg_pq_steps_rust: the Rust shim, linked from the
 * libobs-rust archive, through the header alone. */
#include <math.h>
#include <string.h>

#include "libobs/graphics/graphics-ffmpeg-host-pq.h"
#include "libobs/graphics/half.h"
#else
#include "libobs/graphics/graphics-ffmpeg-host-pq.c.h"
#endif

/*
 * Characterization of the JPEG XR PQ-to-scRGB conversion of the image
 * loader's Windows host (graphics-ffmpeg-host-windows.c), moved to
 * graphics-ffmpeg-host-pq.c. Each 32bppBGR101010 texel (blue in bits 0-9,
 * green in 10-19, red in 20-29) goes through the SMPTE ST 2084 inverse EOTF,
 * the BT.2020-to-709 matrix and a scale of 125 into four half floats, alpha
 * 1.0. The file includes no Windows header, so this runs on every platform.
 *
 * pq_to_linear() calls powf() twice, whose last bit may differ between
 * C libraries: values that depend on it are checked to within a float's or
 * a half's last place; values from code values 0 and 1023 (0 and 1 exactly)
 * are exact.
 *
 * Built twice: test_graphics_ffmpeg_pq_steps compiles the C into itself, and
 * test_graphics_ffmpeg_pq_steps_rust (GS_PQ_STEPS_TEST_RUST) links the Rust
 * shim instead. The Rust build skips test_pq_to_linear, which calls the C
 * file's static pq_to_linear(); its half_from_float() cases check half.h,
 * which both builds include.
 */

#define ALPHA_ONE 0x3c00

static void convert(uint16_t out[4], uint32_t texel)
{
	uint8_t src[4], dst[8];
	memcpy(src, &texel, sizeof(src));
	memset(dst, 0xa5, sizeof(dst));
	gs_image_pq10_to_scrgb(dst, sizeof(dst), src, sizeof(src));
	memcpy(out, dst, sizeof(dst));
}

static uint32_t texel(uint32_t red, uint32_t green, uint32_t blue)
{
	return red << 20 | green << 10 | blue;
}

static void expect_texel(uint32_t value, uint16_t red, uint16_t green, uint16_t blue)
{
	uint16_t out[4];
	convert(out, value);
	assert_int_equal(out[0], red);
	assert_int_equal(out[1], green);
	assert_int_equal(out[2], blue);
	assert_int_equal(out[3], ALPHA_ONE);
}

/* Within one unit in the last place of the half, with the same sign. */
static void expect_half_near(uint16_t got, uint16_t want)
{
	const int distance = abs((int)got - (int)want);
	if ((got & 0x8000) != (want & 0x8000) || distance > 1) {
		fprintf(stderr, "half 0x%04x, expected 0x%04x within 1\n", got, want);
		fail();
	}
}

static void expect_texel_near(uint32_t value, uint16_t red, uint16_t green, uint16_t blue)
{
	uint16_t out[4];
	convert(out, value);
	expect_half_near(out[0], red);
	expect_half_near(out[1], green);
	expect_half_near(out[2], blue);
	assert_int_equal(out[3], ALPHA_ONE);
}

#ifndef GS_PQ_STEPS_TEST_RUST
/* PQ raises (common - 0.8359375) / (...) to 1 / 0.1593 (about 6.3), so one
 * unit in the last place of the inner powf moves the result by 2e-5 to 4e-5
 * relative. libms that are each within an ulp still differ by that much
 * (macOS gave 4.0422572e-09 for code 1, glibc 4.04224476e-09), so the bound
 * allows a few ulps of the inner powf, not of the result. */
static void expect_float_near(float got, float want)
{
	if (fabsf(got - want) > fabsf(want) * 2e-4f) {
		fprintf(stderr, "%.9g, expected %.9g\n", (double)got, (double)want);
		fail();
	}
}

static void test_pq_to_linear(void **state)
{
	(void)state;
	assert_true(pq_to_linear(0.f / 1023.f) == 0.f);
	expect_float_near(pq_to_linear(1.f / 1023.f), 4.04224476e-09f);
	expect_float_near(pq_to_linear(2.f / 1023.f), 1.31113742e-08f);
	expect_float_near(pq_to_linear(512.f / 1023.f), 0.00926986337f);
	expect_float_near(pq_to_linear(1022.f / 1023.f), 0.990744293f);
	assert_true(pq_to_linear(1023.f / 1023.f) == 1.f);
}
#endif

/* half.h's half_from_float(), which the conversion calls: the signs of
 * zero, the smallest normal and denormal halves, the largest half, the first
 * value that rounds past it, infinity and NaN. */
static uint16_t half_bits(uint32_t float_bits)
{
	float f;
	memcpy(&f, &float_bits, sizeof(f));
	return half_from_float(f).u;
}

static void test_half_from_float(void **state)
{
	(void)state;
	assert_int_equal(half_bits(0x00000000), 0x0000);
	assert_int_equal(half_bits(0x80000000), 0x8000);
	assert_int_equal(half_bits(0x38800000), 0x0400); /* 2^-14 */
	assert_int_equal(half_bits(0x33800000), 0x0001); /* 2^-24 */
	assert_int_equal(half_bits(0xb3800000), 0x8001);
	assert_int_equal(half_bits(0x477fe000), 0x7bff); /* 65504 */
	assert_int_equal(half_bits(0x477ff000), 0x7c00); /* 65520 */
	assert_int_equal(half_bits(0x7f800000), 0x7c00);
	assert_int_equal(half_bits(0xff800000), 0xfc00);
	assert_int_equal(half_bits(0x7fc00000), 0x7fff);
	assert_int_equal(half_bits(0xffc00000), 0xffff);
}

/* 0 and 10 000 nits: 0 and 125.0 (10 000 / 80). */
static void test_black_and_white(void **state)
{
	(void)state;
	expect_texel(texel(0, 0, 0), 0x0000, 0x0000, 0x0000);
	expect_texel(texel(1023, 1023, 1023), 0x57d0, 0x57d0, 0x57d0);
}

static void test_grey_ramp(void **state)
{
	(void)state;
	static const uint16_t halves[] = {0x0000, 0x1f9f, 0x2c23, 0x34ef, 0x3ca3, 0x43c5, 0x4a30, 0x50e1};
	for (uint32_t i = 0; i < 8; i++)
		expect_texel_near(texel(i * 128, i * 128, i * 128), halves[i], halves[i], halves[i]);
}

/* A full-scale BT.2020 primary lies outside BT.709: two of its 709 values
 * are negative, and the texture keeps them (scRGB allows them).
 * Characterized, not endorsed. */
static void test_primaries(void **state)
{
	(void)state;
	expect_texel(texel(1023, 0, 0), 0x5a7c, 0xcbc9, 0xc08a);
	expect_texel(texel(0, 1023, 0), 0xd497, 0x586d, 0xca49);
	expect_texel(texel(0, 0, 1023), 0xc88e, 0xbc2d, 0x585f);
}

/* Code value 1 of one channel: half denormals, and -0 where the 709 value
 * is below the smallest one. */
static void test_primaries_at_code_value_one(void **state)
{
	(void)state;
	expect_texel_near(texel(1, 0, 0), 0x000e, 0x8001, 0x8000);
	expect_texel_near(texel(0, 1, 0), 0x8005, 0x000a, 0x8001);
	expect_texel_near(texel(0, 0, 1), 0x8001, 0x8000, 0x0009);
}

/* A saturated green, outside 709 too. Characterized, not endorsed. */
static void test_negative_709(void **state)
{
	(void)state;
	expect_texel_near(texel(0x100, 0x3c0, 0x080), 0xd11e, 0x54f2, 0xc704);
}

/* Bits 30-31 are not read: every value of them gives the same texel. */
static void test_high_bits_ignored(void **state)
{
	(void)state;
	const uint32_t base = texel(0x123, 0x2ab, 0x077);
	uint16_t want[4];
	convert(want, base);
	expect_texel_near(base, 0xc26c, 0x4681, 0xb897);
	for (uint32_t high = 1; high < 4; high++)
		expect_texel(base | high << 30, want[0], want[1], want[2]);
}

/* Each 4-byte texel gives one 8-byte texel, in order, alpha 1.0 on each. */
static void test_buffer(void **state)
{
	(void)state;
	const uint32_t texels[3] = {texel(1023, 0, 0), texel(0, 0, 0), texel(1023, 1023, 1023)};
	uint8_t src[sizeof(texels)];
	uint8_t dst[3 * 8 + 1];
	uint16_t out[12];
	memcpy(src, texels, sizeof(src));
	memset(dst, 0xa5, sizeof(dst));
	gs_image_pq10_to_scrgb(dst, 3 * 8, src, sizeof(src));
	memcpy(out, dst, sizeof(out));
	const uint16_t want[12] = {0x5a7c, 0xcbc9,    0xc08a, ALPHA_ONE, 0x0000, 0x0000,
				   0x0000, ALPHA_ONE, 0x57d0, 0x57d0,    0x57d0, ALPHA_ONE};
	for (int i = 0; i < 12; i++)
		assert_int_equal(out[i], want[i]);
	assert_int_equal(dst[3 * 8], 0xa5);
}

/* No texels: nothing is written. */
static void test_empty(void **state)
{
	(void)state;
	uint8_t src[4] = {0xff, 0xff, 0xff, 0xff};
	uint8_t dst[8];
	memset(dst, 0xa5, sizeof(dst));
	gs_image_pq10_to_scrgb(dst, 0, src, 0);
	for (size_t i = 0; i < sizeof(dst); i++)
		assert_int_equal(dst[i], 0xa5);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
#ifndef GS_PQ_STEPS_TEST_RUST
		cmocka_unit_test(test_pq_to_linear),
#endif
		cmocka_unit_test(test_half_from_float),
		cmocka_unit_test(test_black_and_white),
		cmocka_unit_test(test_grey_ramp),
		cmocka_unit_test(test_primaries),
		cmocka_unit_test(test_primaries_at_code_value_one),
		cmocka_unit_test(test_negative_709),
		cmocka_unit_test(test_high_bits_ignored),
		cmocka_unit_test(test_buffer),
		cmocka_unit_test(test_empty),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
