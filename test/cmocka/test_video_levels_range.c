/* Regression test for obs-rust/obs-studio#2209: obs_set_video_levels() in
 * libobs/obs.c.h stored the SDR white level and the HDR nominal peak level
 * without checking them, and libobs never gave them their documented
 * defaults (300 and 1000 nits) once graphics were up: they read 0 until the
 * frontend set them. The draw paths divide by the SDR white level
 * (80 / white, white / 80), so 0, a negative value, NaN or an infinity
 * turns every scRGB multiplier into inf, NaN, 0 or a negative number.
 *
 * With graphics up, the levels must read 300 and 1000 before anything sets
 * them. Each level is then checked on its own: a level that is not finite
 * and greater than 0 (0, -1, NaN, +inf, -inf) is ignored with a warning and
 * the previous value stays, while a valid level in the same call is still
 * stored. Valid levels, FLT_MAX and FLT_MIN among them, are stored exactly
 * and log nothing.
 *
 * obs_startup() and obs_reset_video() need a display on Linux, so the test
 * runs under xvfb-run there. It skips only when no display is set at all;
 * with one set, a failed startup fails the test. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <float.h>
#include <math.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <obs.h>
#include <util/base.h>

static bool obs_up;

/* Warnings that obs_set_video_levels() logs; everything is passed on to the
 * previous handler. */
static log_handler_t prev_handler;
static void *prev_param;
static volatile int level_warnings;

static void log_handler(int lvl, const char *format, va_list args, void *param)
{
	(void)param;
	if (lvl == LOG_WARNING && strncmp(format, "obs_set_video_levels", 20) == 0)
		level_warnings++;
	if (prev_handler)
		prev_handler(lvl, format, args, prev_param);
}

static bool same(float a, float b)
{
	return memcmp(&a, &b, sizeof(a)) == 0;
}

static void expect_levels(const char *what, float white, float peak)
{
	float got_white = obs_get_video_sdr_white_level();
	float got_peak = obs_get_video_hdr_nominal_peak_level();
	if (!same(got_white, white) || !same(got_peak, peak))
		fprintf(stderr, "%s: levels %g/%g, expected %g/%g\n", what, got_white, got_peak, white, peak);
	assert_true(same(got_white, white));
	assert_true(same(got_peak, peak));
}

/* Calls obs_set_video_levels(white, peak) and expects the getters to read
 * want_white and want_peak, then the call to have logged `warnings`
 * warnings. */
static void set_levels_expect(float white, float peak, float want_white, float want_peak, int warnings)
{
	int before = level_warnings;
	obs_set_video_levels(white, peak);
	int logged = level_warnings - before;
	char what[80];
	snprintf(what, sizeof(what), "obs_set_video_levels(%g, %g)", white, peak);
	expect_levels(what, want_white, want_peak);
	if (logged != warnings)
		fprintf(stderr, "%s: %d warnings, expected %d\n", what, logged, warnings);
	assert_int_equal(logged, warnings);
}

static void set_levels(float white, float peak)
{
	set_levels_expect(white, peak, white, peak, 0);
}

/* Must run first: nothing has set the levels yet. */
static void defaults_before_any_set(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	expect_levels("after obs_reset_video()", 300.0f, 1000.0f);
}

static const float bad[] = {0.0f, -0.0f, -1.0f, NAN, INFINITY, -INFINITY};
#define NUM_BAD (sizeof(bad) / sizeof(bad[0]))

static void bad_white_keeps_white(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	for (size_t i = 0; i < NUM_BAD; i++) {
		set_levels(200.0f, 600.0f);
		set_levels_expect(bad[i], 700.0f, 200.0f, 700.0f, 1);
	}
}

static void bad_peak_keeps_peak(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	for (size_t i = 0; i < NUM_BAD; i++) {
		set_levels(200.0f, 600.0f);
		set_levels_expect(250.0f, bad[i], 250.0f, 600.0f, 1);
	}
}

static void bad_pair_keeps_both(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	for (size_t i = 0; i < NUM_BAD; i++) {
		for (size_t j = 0; j < NUM_BAD; j++) {
			set_levels(200.0f, 600.0f);
			set_levels_expect(bad[i], bad[j], 200.0f, 600.0f, 2);
		}
	}
}

static void valid_pairs_stored_exactly(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	static const float valid[][2] = {
		{300.0f, 1000.0f},  {80.0f, 80.0f},     {1.0f, 10000.0f},
		{480.0f, 400.0f},   {FLT_MAX, FLT_MAX}, {FLT_MIN, FLT_MIN},
		{FLT_MAX, 1000.0f}, {300.0f, FLT_MAX},  {4294967295.0f, 4294967295.0f},
	};
	for (size_t i = 0; i < sizeof(valid) / sizeof(valid[0]); i++)
		set_levels(valid[i][0], valid[i][1]);
	set_levels(300.0f, 1000.0f);
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

	base_get_log_handler(&prev_handler, &prev_param);
	base_set_log_handler(log_handler, NULL);
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up) {
		base_set_log_handler(prev_handler, prev_param);
		obs_shutdown();
	}
	obs_up = false;
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(defaults_before_any_set),    cmocka_unit_test(bad_white_keeps_white),
		cmocka_unit_test(bad_peak_keeps_peak),        cmocka_unit_test(bad_pair_keeps_both),
		cmocka_unit_test(valid_pairs_stored_exactly),
	};
	return cmocka_run_group_tests_name("test_video_levels_range", tests, group_setup, group_teardown);
}
