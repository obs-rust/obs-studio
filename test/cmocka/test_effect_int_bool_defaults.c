/* Regression test for obs-rust/obs-studio#2548: the effect parser stored
 * each `int`/`intN` element and each `bool` default as a `long`, and the
 * shader parser did the same for `int`. `long` is 8 bytes on Linux and
 * macOS, while every backend and gs_effect_set_int()/gs_effect_set_bool()
 * use a 4-byte `int` per element, so the GL backend refused the default
 * ("Size of shader param does not match the size of the input") and the
 * shader drew with 0.
 *
 * The effect case creates an effect with int, int2 and bool defaults,
 * checks the stored sizes and values, then draws them through a pixel shader
 * and reads the pixel back. The shader parser case needs no device.
 *
 * obs_startup() and obs_reset_video() need a display on Linux, so the effect
 * case runs under xvfb-run there. It skips only when no display is set at
 * all; with one set, a failed startup fails the test. */

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
#include <graphics/shader-parser.h>
#include <graphics/vec4.h>
#include <util/base.h>
#include <util/bmem.h>
#include <util/platform.h>

#define CANVAS_SIZE 8

static bool obs_up;

static const char effect_text[] = "uniform float4x4 ViewProj;\n"
				  "uniform int i = -3;\n"
				  "uniform int2 v = {1, 2};\n"
				  "uniform bool b = true;\n"
				  "uniform bool f = false;\n"
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
				  "float4 PSDefaults(VertInOut vert_in) : TARGET\n"
				  "{\n"
				  "\tfloat r = float((i + 10) * 10) / 255.0;\n"
				  "\tfloat g = float(v.x * 20) / 255.0;\n"
				  "\tfloat bl = float(v.y * 40) / 255.0;\n"
				  "\tfloat a = (b && !f) ? 1.0 : (128.0 / 255.0);\n"
				  "\treturn float4(r, g, bl, a);\n"
				  "}\n"
				  "\n"
				  "technique Draw\n"
				  "{\n"
				  "\tpass\n"
				  "\t{\n"
				  "\t\tvertex_shader = VSDefault(vert_in);\n"
				  "\t\tpixel_shader  = PSDefaults(vert_in);\n"
				  "\t}\n"
				  "}\n";

/* "does not match" and "invalid size" errors from the GL backend's size
 * checks; everything is passed on to the previous handler. */
static log_handler_t prev_handler;
static void *prev_param;
static int size_errors;

static void log_handler(int lvl, const char *format, va_list args, void *param)
{
	(void)param;
	if (lvl == LOG_ERROR && (strstr(format, "does not match") || strstr(format, "invalid size")))
		size_errors++;
	if (prev_handler)
		prev_handler(lvl, format, args, prev_param);
}

struct stored_default {
	size_t size;
	int val[2];
};

/* Copies a parameter's default; asserts wait until graphics is left, so a
 * failure cannot leave the context entered. */
static struct stored_default get_default(gs_effect_t *effect, const char *name)
{
	struct stored_default got = {0, {0, 0}};
	gs_eparam_t *param = gs_effect_get_param_by_name(effect, name);
	if (!param) {
		got.size = SIZE_MAX;
		return got;
	}

	got.size = gs_effect_get_default_val_size(param);
	void *val = gs_effect_get_default_val(param);
	if (val)
		memcpy(got.val, val, got.size < sizeof(got.val) ? got.size : sizeof(got.val));
	bfree(val);
	return got;
}

static void assert_default(const char *name, const struct stored_default *got, const int *want, size_t count)
{
	if (got->size != count * sizeof(int))
		fprintf(stderr, "%s: default is %zu bytes, expected %zu\n", name, got->size, count * sizeof(int));
	assert_int_equal(got->size, count * sizeof(int));
	for (size_t k = 0; k < count; k++)
		assert_int_equal(got->val[k], want[k]);
}

static void effect_defaults_reach_the_shader(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	log_handler_t handler;
	void *handler_param;
	base_get_log_handler(&handler, &handler_param);
	prev_handler = handler;
	prev_param = handler_param;
	size_errors = 0;
	base_set_log_handler(log_handler, NULL);

	obs_enter_graphics();
	char *errors = NULL;
	gs_effect_t *effect = gs_effect_create(effect_text, "int_bool_defaults.effect", &errors);
	if (!effect)
		fprintf(stderr, "gs_effect_create failed: %s\n", errors ? errors : "(no errors)");
	bfree(errors);

	struct stored_default got_i = {0}, got_v = {0}, got_b = {0}, got_f = {0};
	/* the centre pixel */
	uint8_t pixel[4] = {0, 0, 0, 0};
	bool mapped = false;
	if (effect) {
		got_i = get_default(effect, "i");
		got_v = get_default(effect, "v");
		got_b = get_default(effect, "b");
		got_f = get_default(effect, "f");

		gs_texrender_t *tr = gs_texrender_create(GS_RGBA, GS_ZS_NONE);
		gs_stagesurf_t *stage = gs_stagesurface_create(CANVAS_SIZE, CANVAS_SIZE, GS_RGBA);
		if (gs_texrender_begin(tr, CANVAS_SIZE, CANVAS_SIZE)) {
			struct vec4 clear;
			vec4_zero(&clear);
			gs_clear(GS_CLEAR_COLOR, &clear, 0.0f, 0);
			gs_ortho(0.0f, (float)CANVAS_SIZE, 0.0f, (float)CANVAS_SIZE, -100.0f, 100.0f);
			gs_blend_state_push();
			gs_reset_blend_state();
			gs_enable_blending(false);
			while (gs_effect_loop(effect, "Draw"))
				gs_draw_sprite(NULL, 0, CANVAS_SIZE, CANVAS_SIZE);
			gs_blend_state_pop();
			gs_texrender_end(tr);

			uint8_t *data;
			uint32_t linesize;
			gs_stage_texture(stage, gs_texrender_get_texture(tr));
			if (gs_stagesurface_map(stage, &data, &linesize)) {
				const uint8_t *p = data + (size_t)(CANVAS_SIZE / 2) * linesize + (CANVAS_SIZE / 2) * 4;
				memcpy(pixel, p, sizeof(pixel));
				mapped = true;
				gs_stagesurface_unmap(stage);
			}
		}
		gs_stagesurface_destroy(stage);
		gs_texrender_destroy(tr);
		gs_effect_destroy(effect);
	}
	obs_leave_graphics();

	base_set_log_handler(handler, handler_param);

	assert_non_null(effect);
	if (size_errors != 0)
		fprintf(stderr, "the backend refused %d parameter values for their size\n", size_errors);
	/* (i + 10) * 10 = 70, v.x * 20 = 20, v.y * 40 = 80, b && !f: opaque.
	 * With every default refused the shader sees 0: 100, 0, 0, 128. */
	fprintf(stderr, "pixel: %u %u %u %u\n", pixel[0], pixel[1], pixel[2], pixel[3]);

	const int want_i[] = {-3};
	const int want_v[] = {1, 2};
	const int want_b[] = {1};
	const int want_f[] = {0};
	assert_default("i", &got_i, want_i, 1);
	assert_default("v", &got_v, want_v, 2);
	assert_default("b", &got_b, want_b, 1);
	assert_default("f", &got_f, want_f, 1);

	assert_true(mapped);
	assert_int_equal(pixel[0], 70);
	assert_int_equal(pixel[1], 20);
	assert_int_equal(pixel[2], 80);
	assert_int_equal(pixel[3], 255);
	assert_int_equal(size_errors, 0);
}

static const char shader_text[] = "uniform int i = -3;\n"
				  "uniform int j = 7;\n"
				  "uniform float x = 2.0;\n"
				  "\n"
				  "float4 main(float4 pos : POSITION) : TARGET\n"
				  "{\n"
				  "\treturn float4(float(i + j), x, 0.0, 1.0);\n"
				  "}\n";

static const struct shader_var *find_var(const struct shader_parser *sp, const char *name)
{
	for (size_t k = 0; k < sp->params.num; k++) {
		if (strcmp(sp->params.array[k].name, name) == 0)
			return sp->params.array + k;
	}
	return NULL;
}

static void assert_int_var(const struct shader_parser *sp, const char *name, int want)
{
	const struct shader_var *var = find_var(sp, name);
	assert_non_null(var);
	if (var->default_val.num != sizeof(int))
		fprintf(stderr, "%s: default is %zu bytes, expected %zu\n", name, var->default_val.num, sizeof(int));
	assert_int_equal(var->default_val.num, sizeof(int));
	int val;
	memcpy(&val, var->default_val.array, sizeof(val));
	assert_int_equal(val, want);
}

static void shader_int_default_is_an_int(void **state)
{
	(void)state;
	struct shader_parser sp;
	shader_parser_init(&sp);
	bool ok = shader_parse(&sp, shader_text, "int_default.shader");
	if (!ok) {
		char *errors = error_data_buildstring(&sp.cfp.error_list);
		fprintf(stderr, "shader_parse failed: %s\n", errors ? errors : "(no errors)");
		bfree(errors);
	}
	assert_true(ok);

	assert_int_var(&sp, "i", -3);
	assert_int_var(&sp, "j", 7);

	/* float defaults were always 4 bytes */
	const struct shader_var *x = find_var(&sp, "x");
	assert_non_null(x);
	assert_int_equal(x->default_val.num, sizeof(float));

	shader_parser_free(&sp);
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

	/* Let the graphics thread get through its first frames before the
	 * case renders on its context: a draw before them lands nowhere. */
	for (int i = 0; i < 500 && obs_get_total_frames() < 3; i++)
		os_sleep_ms(10);
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
		cmocka_unit_test(shader_int_default_is_an_int),
		cmocka_unit_test(effect_defaults_reach_the_shader),
	};
	return cmocka_run_group_tests_name("test_effect_int_bool_defaults", tests, group_setup, group_teardown);
}
