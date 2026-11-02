/* Characterization of the effect parser's number reads through the real
 * libobs (#3464): int defaults with and without a '-', the braced and the
 * constructor forms of an int2 and a float2, and an array count, parsed and
 * compiled by gs_effect_create(), then read back as stored defaults. Only
 * values inside int range: past it the C result depends on the width of
 * long (test_effect_param_numbers_steps pins those on the moved C file).
 *
 * obs_startup() and obs_reset_video() need a display on Linux, so the case
 * runs under xvfb-run there. It skips only when no display is set at all;
 * with one set, a failed startup fails the test. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <obs.h>
#include <util/bmem.h>
#include <util/platform.h>

static bool obs_up;

static const char effect_text[] = "uniform float4x4 ViewProj;\n"
				  "uniform int a = 5;\n"
				  "uniform int n = -5;\n"
				  "uniform int z = 007;\n"
				  "uniform float2 b = {1.5, -2};\n"
				  "uniform int2 c = int2(1, -3);\n"
				  "uniform int2 e = {2147483647, -2147483647};\n"
				  "uniform float d[4];\n"
				  "\n"
				  "struct VertInOut {\n"
				  "\tfloat4 pos : POSITION;\n"
				  "};\n"
				  "\n"
				  "VertInOut VSDefault(VertInOut vert_in)\n"
				  "{\n"
				  "\tVertInOut vert_out;\n"
				  "\tvert_out.pos = mul(float4(vert_in.pos.xyz, 1.0), ViewProj);\n"
				  "\treturn vert_out;\n"
				  "}\n"
				  "\n"
				  "float4 PSNumbers(VertInOut vert_in) : TARGET\n"
				  "{\n"
				  "\tfloat r = float(a + n + z + c.x + c.y + e.x + e.y) / 255.0;\n"
				  "\treturn float4(r, b.x, b.y, 1.0);\n"
				  "}\n"
				  "\n"
				  "technique Draw\n"
				  "{\n"
				  "\tpass\n"
				  "\t{\n"
				  "\t\tvertex_shader = VSDefault(vert_in);\n"
				  "\t\tpixel_shader  = PSNumbers(vert_in);\n"
				  "\t}\n"
				  "}\n";

struct stored_default {
	bool found;
	size_t size;
	uint8_t val[16];
};

/* Copies a parameter's default; asserts wait until graphics is left, so a
 * failure cannot leave the context entered. */
static struct stored_default get_default(gs_effect_t *effect, const char *name)
{
	struct stored_default got = {0};
	gs_eparam_t *param = gs_effect_get_param_by_name(effect, name);
	if (!param)
		return got;

	got.found = true;
	got.size = gs_effect_get_default_val_size(param);
	void *val = gs_effect_get_default_val(param);
	if (val)
		memcpy(got.val, val, got.size < sizeof(got.val) ? got.size : sizeof(got.val));
	bfree(val);
	return got;
}

static void assert_ints(const char *name, const struct stored_default *got, const int *want, size_t count)
{
	if (!got->found)
		fprintf(stderr, "%s: no such parameter\n", name);
	assert_true(got->found);
	if (got->size != count * sizeof(int))
		fprintf(stderr, "%s: default is %zu bytes, expected %zu\n", name, got->size, count * sizeof(int));
	assert_int_equal(got->size, count * sizeof(int));
	for (size_t k = 0; k < count; k++) {
		int val;
		memcpy(&val, got->val + k * sizeof(int), sizeof(val));
		assert_int_equal(val, want[k]);
	}
}

static void effect_number_defaults(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_enter_graphics();
	char *errors = NULL;
	gs_effect_t *effect = gs_effect_create(effect_text, "number_defaults.effect", &errors);
	if (!effect)
		fprintf(stderr, "gs_effect_create failed: %s\n", errors ? errors : "(no errors)");
	bfree(errors);

	struct stored_default got_a = {0}, got_n = {0}, got_z = {0}, got_b = {0}, got_c = {0}, got_e = {0}, got_d = {0};
	if (effect) {
		got_a = get_default(effect, "a");
		got_n = get_default(effect, "n");
		got_z = get_default(effect, "z");
		got_b = get_default(effect, "b");
		got_c = get_default(effect, "c");
		got_e = get_default(effect, "e");
		got_d = get_default(effect, "d");
		gs_effect_destroy(effect);
	}
	obs_leave_graphics();

	assert_non_null(effect);

	const int want_a[] = {5};
	const int want_n[] = {-5};
	const int want_z[] = {7};
	const int want_c[] = {1, -3};
	const int want_e[] = {2147483647, -2147483647};
	assert_ints("a", &got_a, want_a, 1);
	assert_ints("n", &got_n, want_n, 1);
	assert_ints("z", &got_z, want_z, 1);
	assert_ints("c", &got_c, want_c, 2);
	assert_ints("e", &got_e, want_e, 2);

	/* The float path stays in effect-parser.c: os_strtod(), then the '-'. */
	assert_true(got_b.found);
	assert_int_equal(got_b.size, 2 * sizeof(float));
	float b[2];
	memcpy(b, got_b.val, sizeof(b));
	assert_true(b[0] == 1.5f);
	assert_true(b[1] == -2.0f);

	/* The array count goes into the shader text only, and only for a
	 * parameter a shader uses (the GL backend does not take an indexed
	 * effect array, so none does here): the parameter parses, with no
	 * default. */
	assert_true(got_d.found);
	assert_int_equal(got_d.size, 0);
}

static int group_setup(void **state)
{
	(void)state;
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		if (getenv("DISPLAY") || getenv("WAYLAND_DISPLAY")) {
			fprintf(stderr, "obs_startup failed with a display set\n");
			return -1;
		}
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = 64,
		.base_height = 64,
		.output_width = 64,
		.output_height = 64,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.gpu_conversion = true,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	int ret = obs_reset_video(&ovi);
	if (ret != OBS_VIDEO_SUCCESS) {
		obs_shutdown();
		obs_up = false;
		if (getenv("DISPLAY") || getenv("WAYLAND_DISPLAY")) {
			fprintf(stderr, "obs_reset_video failed with a display set: %d\n", ret);
			return -1;
		}
		fprintf(stderr, "skipped: obs_reset_video failed: %d\n", ret);
		return 0;
	}
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	obs_up = false;
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(effect_number_defaults),
	};
	return cmocka_run_group_tests_name("test_effect_number_defaults", tests, group_setup, group_teardown);
}
