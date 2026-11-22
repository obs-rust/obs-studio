/* C-only characterization before extracting obs_set_video_levels (Refs
 * #5466).
 *
 * test_video_levels_range (#2209) pins which levels are stored and which are
 * ignored on a live core. This test pins the rest of the public behavior:
 *
 * - each ignored level logs one warning that names the rejected and the kept
 *   value, the SDR white level first;
 * - a valid call logs nothing, and the latest valid call wins;
 * - levels set on one core do not carry into the next obs_startup().
 *
 * Not pinned: a call with a core but no graphics subsystem hits
 * assert(video->graphics), and a call with no core differs on purpose (the
 * C reference reads obs->video through NULL, the Rust core check warns and
 * returns, #3397; Tier 1 and the parity tests of core_setup_rules pin it).
 *
 * obs_startup() and obs_reset_video() need a display and OpenGL (run under
 * xvfb-run); without them the cases that need a core skip. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <util/base.h>
#include <util/threading.h>

#define CANVAS_SIZE 64
#define MAX_MESSAGES 8

#define REQUIRE(cond, why)                                       \
	do {                                                     \
		if (!(cond)) {                                   \
			fprintf(stderr, "skipped: %s\n", (why)); \
			skip();                                  \
		}                                                \
	} while (false)

/* ------------------------------------------------------------------------ */
/* Log capture                                                               */

/* The messages logged while capturing that name obs_set_video_levels, with
 * their text and level. Other threads may log while a core runs, so the
 * others are only passed on. */
static log_handler_t prev_handler;
static void *prev_param;
static pthread_mutex_t capture_mutex = PTHREAD_MUTEX_INITIALIZER;
static int own_messages;
static int own_levels[MAX_MESSAGES];
static char own_texts[MAX_MESSAGES][256];

static void capture_log(int lvl, const char *msg, va_list args, void *p)
{
	char text[256];
	va_list copy;

	va_copy(copy, args);
	vsnprintf(text, sizeof(text), msg, copy);
	va_end(copy);

	pthread_mutex_lock(&capture_mutex);
	if (strstr(text, "obs_set_video_levels")) {
		if (own_messages < MAX_MESSAGES) {
			own_levels[own_messages] = lvl;
			snprintf(own_texts[own_messages], sizeof(own_texts[0]), "%s", text);
		}
		own_messages++;
	}
	pthread_mutex_unlock(&capture_mutex);

	if (prev_handler)
		prev_handler(lvl, msg, args, p);
}

static void start_capture(void)
{
	pthread_mutex_lock(&capture_mutex);
	own_messages = 0;
	memset(own_levels, 0, sizeof(own_levels));
	memset(own_texts, 0, sizeof(own_texts));
	pthread_mutex_unlock(&capture_mutex);
	base_get_log_handler(&prev_handler, &prev_param);
	base_set_log_handler(capture_log, prev_param);
}

static void stop_capture(void)
{
	base_set_log_handler(prev_handler, prev_param);
}

static void expect_own_warning(int i, const char *text)
{
	assert_true(i < own_messages);
	assert_int_equal(own_levels[i], LOG_WARNING);
	assert_string_equal(own_texts[i], text);
}

/* ------------------------------------------------------------------------ */
/* Core lifecycle                                                            */

static bool start_core_with_video(void)
{
	if (!obs_startup("en-US", NULL, NULL))
		return false;

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = CANVAS_SIZE,
		.base_height = CANVAS_SIZE,
		.output_width = CANVAS_SIZE,
		.output_height = CANVAS_SIZE,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.gpu_conversion = true,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	int ret = obs_reset_video(&ovi);
	if (ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "obs_reset_video failed: %d\n", ret);
		obs_shutdown();
		return false;
	}
	return true;
}

static int startup_with_video(void **state)
{
	*state = (void *)(uintptr_t)start_core_with_video();
	return 0;
}

static int shutdown_core(void **state)
{
	(void)state;
	if (obs_initialized())
		obs_shutdown();
	return 0;
}

static void expect_levels(float white, float peak)
{
	assert_float_equal(obs_get_video_sdr_white_level(), white, 0.0f);
	assert_float_equal(obs_get_video_hdr_nominal_peak_level(), peak, 0.0f);
}

/* ------------------------------------------------------------------------ */
/* Cases                                                                     */

static void ignored_levels_name_values(void **state)
{
	REQUIRE(*state, "obs_startup/obs_reset_video failed (no display?)");

	obs_set_video_levels(200.0f, 600.0f);
	expect_levels(200.0f, 600.0f);

	start_capture();
	obs_set_video_levels(-1.0f, 700.0f);
	stop_capture();
	expect_levels(200.0f, 700.0f);
	assert_int_equal(own_messages, 1);
	expect_own_warning(0, "obs_set_video_levels: ignoring SDR white level -1, keeping 200");

	start_capture();
	obs_set_video_levels(250.0f, 0.0f);
	stop_capture();
	expect_levels(250.0f, 700.0f);
	assert_int_equal(own_messages, 1);
	expect_own_warning(0, "obs_set_video_levels: ignoring HDR nominal peak level 0, keeping 700");

	/* Both ignored: one warning each, SDR white first. */
	start_capture();
	obs_set_video_levels(-2.5f, -3.5f);
	stop_capture();
	expect_levels(250.0f, 700.0f);
	assert_int_equal(own_messages, 2);
	expect_own_warning(0, "obs_set_video_levels: ignoring SDR white level -2.5, keeping 250");
	expect_own_warning(1, "obs_set_video_levels: ignoring HDR nominal peak level -3.5, keeping 700");
}

static void valid_calls_log_nothing_and_latest_wins(void **state)
{
	REQUIRE(*state, "obs_startup/obs_reset_video failed (no display?)");

	start_capture();
	obs_set_video_levels(100.0f, 200.0f);
	expect_levels(100.0f, 200.0f);
	obs_set_video_levels(400.0f, 800.0f);
	expect_levels(400.0f, 800.0f);
	obs_set_video_levels(400.0f, 800.0f);
	expect_levels(400.0f, 800.0f);
	obs_set_video_levels(80.0f, 10000.0f);
	stop_capture();

	expect_levels(80.0f, 10000.0f);
	assert_int_equal(own_messages, 0);
}

static void levels_do_not_survive_restart(void **state)
{
	REQUIRE(*state, "obs_startup/obs_reset_video failed (no display?)");

	obs_set_video_levels(123.0f, 456.0f);
	expect_levels(123.0f, 456.0f);
	obs_shutdown();

	REQUIRE(start_core_with_video(), "second obs_startup/obs_reset_video failed");
	expect_levels(300.0f, 1000.0f);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(ignored_levels_name_values, startup_with_video, shutdown_core),
		cmocka_unit_test_setup_teardown(valid_calls_log_nothing_and_latest_wins, startup_with_video,
						shutdown_core),
		cmocka_unit_test_setup_teardown(levels_do_not_survive_restart, startup_with_video, shutdown_core),
	};
	return cmocka_run_group_tests_name("test_set_video_levels", tests, NULL, NULL);
}
