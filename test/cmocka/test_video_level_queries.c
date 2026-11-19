/* C-source characterization for #5168, using real private headers without
 * starting OBS or a GPU. test_no_core_getters and test_video_levels_range
 * retain actual shared-library coverage. The Rust swap must relink this
 * unchanged fixture to Rust and its real private field adapters. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdint.h>
#include <string.h>
#include "obs-internal.h"
#include "graphics/graphics-internal.h"

_Static_assert(sizeof(float) == sizeof(uint32_t), "level queries use binary32 floats");

struct obs_core *obs;
static struct obs_core core;
static graphics_t graphics_token;

static uint32_t float_bits(float value)
{
	uint32_t bits;
	memcpy(&bits, &value, sizeof(bits));
	return bits;
}

static void store_bits(float *value, uint32_t bits)
{
	memcpy(value, &bits, sizeof(bits));
}

static void expect_levels(uint32_t white, uint32_t peak)
{
	unsigned char before[sizeof(core)];
	memcpy(before, &core, sizeof(before));
	struct obs_core *owner = obs;
	assert_int_equal(float_bits(obs_get_video_sdr_white_level()), white);
	assert_memory_equal(&core, before, sizeof(before));
	assert_ptr_equal(obs, owner);
	assert_int_equal(float_bits(obs_get_video_hdr_nominal_peak_level()), peak);
	assert_memory_equal(&core, before, sizeof(before));
	assert_ptr_equal(obs, owner);
}

static int setup(void **state)
{
	(void)state;
	memset(&core, 0, sizeof(core));
	obs = &core;
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	obs = NULL;
	return 0;
}

static void no_core_before_and_after(void **state)
{
	(void)state;
	/* NULL before startup and again after teardown must be checked before
	 * forming &obs->video. The fixture has no resources to destroy. */
	obs = NULL;
	expect_levels(UINT32_C(0x43960000), UINT32_C(0x447a0000));
	obs = &core;
	core.video.graphics = &graphics_token;
	store_bits(&core.video.sdr_white_level, UINT32_C(0x43234567));
	store_bits(&core.video.hdr_nominal_peak_level, UINT32_C(0x45abcdef));
	expect_levels(UINT32_C(0x43234567), UINT32_C(0x45abcdef));
	obs = NULL;
	expect_levels(UINT32_C(0x43960000), UINT32_C(0x447a0000));
}

static void no_graphics_ignores_stored_fields(void **state)
{
	(void)state;
	store_bits(&core.video.sdr_white_level, UINT32_C(0x43234567));
	store_bits(&core.video.hdr_nominal_peak_level, UINT32_C(0x45abcdef));
	expect_levels(UINT32_C(0x43960000), UINT32_C(0x447a0000));
	store_bits(&core.video.sdr_white_level, UINT32_C(0x7fc12345));
	store_bits(&core.video.hdr_nominal_peak_level, UINT32_C(0x80000000));
	expect_levels(UINT32_C(0x43960000), UINT32_C(0x447a0000));
}

static void graphics_returns_exact_field_bits(void **state)
{
	(void)state;
	/* Characterized, not endorsed: getters do no validation, clamping or
	 * arithmetic. Invalid setter inputs placed directly in private storage
	 * are returned verbatim, including signed zero and quiet NaN payloads. */
	static const uint32_t pairs[][2] = {
		{0x43234567, 0x45abcdef}, {0x45abcdef, 0x43234567}, {0x00000000, 0x80000000},
		{0x80000000, 0x00000000}, {0xbf800000, 0xc0200000}, {0x7f800000, 0xff800000},
		{0x7fc12345, 0xffc54321}, {0xffc54321, 0x7fc12345}, {0x00800000, 0x7f7fffff},
	};
	core.video.graphics = &graphics_token;
	for (size_t i = 0; i < sizeof(pairs) / sizeof(pairs[0]); i++) {
		store_bits(&core.video.sdr_white_level, pairs[i][0]);
		store_bits(&core.video.hdr_nominal_peak_level, pairs[i][1]);
		expect_levels(pairs[i][0], pairs[i][1]);
	}
	/* Removing graphics selects defaults even with stored exceptional bits. */
	core.video.graphics = NULL;
	expect_levels(UINT32_C(0x43960000), UINT32_C(0x447a0000));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(no_core_before_and_after, setup, teardown),
		cmocka_unit_test_setup_teardown(no_graphics_ignores_stored_fields, setup, teardown),
		cmocka_unit_test_setup_teardown(graphics_returns_exact_field_bits, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
