/* Characterize the obs_canvas_reset_video() entry point before the Rust swap
 * (Refs #5133). test_canvas_rules_callers::reset_rules pins the NULL info,
 * main canvas, active video and success cases; this adds the short-circuit
 * order (NULL info is refused before the canvas is read), that a removed
 * non-main canvas still resets, that the info passed in is what the canvas
 * keeps, and that the internal reset's result is forwarded as is: a refused
 * mix still clears the old one and signals video_reset, then returns false.
 *
 * obs_reset_video() needs a display and OpenGL (run under xvfb-run); without
 * them the cases skip. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "cmocka-compat.h"

#include <stdio.h>
#include <string.h>

#include "obs-internal.h"

#if !__has_include("obs-canvas-reset-video-entry.c.h")
#error "canvas video-reset entry extraction seam is missing"
#endif

#define CANVAS_SIZE 64

static bool obs_up;

#define REQUIRE_OBS()                                                                   \
	do {                                                                            \
		if (!obs_up) {                                                          \
			fprintf(stderr, "skipped: obs_startup or obs_reset_video failed\n"); \
			skip();                                                         \
		}                                                                       \
	} while (false)

static struct obs_video_info video_info(uint32_t width, uint32_t height)
{
	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = width,
		.base_height = height,
		.output_width = width,
		.output_height = height,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	return ovi;
}

static void assert_same_info(const struct obs_video_info *a, const struct obs_video_info *b)
{
	assert_ptr_equal(a->graphics_module, b->graphics_module);
	assert_int_equal(a->fps_num, b->fps_num);
	assert_int_equal(a->fps_den, b->fps_den);
	assert_int_equal(a->base_width, b->base_width);
	assert_int_equal(a->base_height, b->base_height);
	assert_int_equal(a->output_width, b->output_width);
	assert_int_equal(a->output_height, b->output_height);
	assert_int_equal(a->output_format, b->output_format);
	assert_int_equal(a->adapter, b->adapter);
	assert_int_equal(a->gpu_conversion, b->gpu_conversion);
	assert_int_equal(a->colorspace, b->colorspace);
	assert_int_equal(a->range, b->range);
	assert_int_equal(a->scale_type, b->scale_type);
}

struct reset_log {
	int global;
	int local;
	bool had_mix;
};

static void global_cb(void *param, calldata_t *data)
{
	(void)data;
	struct reset_log *log = param;
	log->global++;
}

static void local_cb(void *param, calldata_t *data)
{
	struct reset_log *log = param;
	obs_canvas_t *canvas = calldata_ptr(data, "canvas");
	log->local++;
	log->had_mix = canvas && canvas->mix != NULL;
}

static void connect_log(obs_canvas_t *canvas, struct reset_log *log)
{
	signal_handler_connect(obs_get_signal_handler(), "canvas_video_reset", global_cb, log);
	signal_handler_connect(obs_canvas_get_signal_handler(canvas), "video_reset", local_cb, log);
}

static void disconnect_log(obs_canvas_t *canvas, struct reset_log *log)
{
	signal_handler_disconnect(obs_get_signal_handler(), "canvas_video_reset", global_cb, log);
	signal_handler_disconnect(obs_canvas_get_signal_handler(canvas), "video_reset", local_cb, log);
}

static void null_info_first(void **state)
{
	(void)state;
	REQUIRE_OBS();
	/* The NULL info check comes before any canvas field is read. */
	assert_false(obs_canvas_reset_video(NULL, NULL));
}

static void removed_canvas_resets(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_canvas_t *canvas = obs_canvas_create("reset entry removed", NULL, PROGRAM);
	assert_non_null(canvas);
	obs_canvas_remove(canvas);
	assert_true(obs_canvas_removed(canvas));

	struct reset_log log = {0};
	connect_log(canvas, &log);
	struct obs_video_info ovi = video_info(32, 48);
	assert_true(obs_canvas_reset_video(canvas, &ovi));
	assert_int_equal(log.global, 1);
	assert_int_equal(log.local, 1);
	assert_true(log.had_mix);
	assert_true(obs_canvas_has_video(canvas));
	/* The canvas keeps a copy of the info passed in. */
	assert_same_info(&canvas->ovi, &ovi);

	disconnect_log(canvas, &log);
	obs_canvas_release(canvas);
	obs_wait_for_destroy_queue();
}

static void main_canvas_untouched(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_canvas_t *main_canvas = obs_get_main_canvas();
	assert_non_null(main_canvas);
	struct reset_log log = {0};
	connect_log(main_canvas, &log);
	struct obs_core_video_mix *mix = main_canvas->mix;
	struct obs_video_info before = main_canvas->ovi;
	struct obs_video_info ovi = video_info(32, 48);

	assert_false(obs_canvas_reset_video(main_canvas, &ovi));
	assert_int_equal(log.global, 0);
	assert_int_equal(log.local, 0);
	assert_ptr_equal(main_canvas->mix, mix);
	assert_same_info(&main_canvas->ovi, &before);

	disconnect_log(main_canvas, &log);
	obs_canvas_release(main_canvas);
}

static void failed_mix_result_forwarded(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_canvas_t *canvas = obs_canvas_create("reset entry failed mix", NULL, PROGRAM);
	assert_non_null(canvas);
	struct obs_video_info good = video_info(CANVAS_SIZE, CANVAS_SIZE);
	assert_true(obs_canvas_reset_video(canvas, &good));
	assert_true(obs_canvas_has_video(canvas));

	struct reset_log log = {0};
	connect_log(canvas, &log);
	/* An output width of 0 is refused by the video output: the old mix is
	 * cleared, video_reset still fires, and the false result comes back. */
	struct obs_video_info bad = video_info(CANVAS_SIZE, CANVAS_SIZE);
	bad.output_width = 0;
	assert_false(obs_canvas_reset_video(canvas, &bad));
	assert_int_equal(log.global, 1);
	assert_int_equal(log.local, 1);
	assert_false(log.had_mix);
	assert_false(obs_canvas_has_video(canvas));
	assert_same_info(&canvas->ovi, &bad);

	/* And it can be reset again afterwards. */
	assert_true(obs_canvas_reset_video(canvas, &good));
	assert_int_equal(log.local, 2);
	assert_true(log.had_mix);

	disconnect_log(canvas, &log);
	obs_canvas_release(canvas);
	obs_wait_for_destroy_queue();
}

static int setup(void **state)
{
	(void)state;
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}
	struct obs_video_info ovi = video_info(CANVAS_SIZE, CANVAS_SIZE);
	int ret = obs_reset_video(&ovi);
	if (ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "skipped: obs_reset_video failed: %d\n", ret);
		obs_shutdown();
		obs_up = false;
	}
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (obs_up) {
		obs_wait_for_destroy_queue();
		obs_shutdown();
	}
	obs_up = false;
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_info_first),
		cmocka_unit_test(removed_canvas_resets),
		cmocka_unit_test(main_canvas_untouched),
		cmocka_unit_test(failed_mix_result_forwarded),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
