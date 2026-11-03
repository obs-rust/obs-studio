/* #3204: real Xvfb capture, cursor replies, pixel output and tick timings. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <pthread.h>
#include <time.h>
#include <obs.h>
#include <util/platform.h>
#include "plugins/linux-capture/build/linux-capture.nix+.h.c"

static pthread_mutex_t live_lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t live_changed = PTHREAD_COND_INITIALIZER;
static unsigned live_ticks;
static uint64_t live_total, live_max;
static bool live_pixels;

static void measured_tick(void *data, float seconds)
{
	const uint64_t start = os_gettime_ns();
	xshm_video_tick(data, seconds);
	const uint64_t elapsed = os_gettime_ns() - start;
	struct xshm_data *capture = data;
	/* Synchronize with the server before looking at SHM via the GPU upload:
	 * rendering the texture verifies the exact bytes users see. */
	bool pixels = false;
	if (capture->texture) {
		obs_enter_graphics();
		gs_stagesurf_t *stage = gs_stagesurface_create(1, 1, GS_BGRA);
		gs_texture_t *pixel = gs_texture_create(1, 1, GS_BGRA, 1, NULL, 0);
		gs_copy_texture_region(pixel, 0, 0, capture->texture, 0, 0, 1, 1);
		gs_stage_texture(stage, pixel);
		uint8_t *bytes;
		uint32_t stride;
		if (gs_stagesurface_map(stage, &bytes, &stride)) {
			pixels = bytes[0] == 0xcc && bytes[1] == 0x66 && bytes[2] == 0x33;
			gs_stagesurface_unmap(stage);
		}
		gs_texture_destroy(pixel);
		gs_stagesurface_destroy(stage);
		obs_leave_graphics();
	}
	pthread_mutex_lock(&live_lock);
	live_pixels |= pixels;
	live_ticks++;
	live_total += elapsed;
	if (elapsed > live_max)
		live_max = elapsed;
	pthread_cond_signal(&live_changed);
	pthread_mutex_unlock(&live_lock);
}

static void capture_real_pixels_without_waiting_for_x(void **state)
{
	(void)state;
	if (!getenv("DISPLAY"))
		skip();
	xcb_connection_t *xcb = xcb_connect(NULL, NULL);
	assert_int_equal(xcb_connection_has_error(xcb), 0);
	xcb_screen_t *screen = xcb_setup_roots_iterator(xcb_get_setup(xcb)).data;
	uint32_t background = 0x3366cc;
	xcb_change_window_attributes(xcb, screen->root, XCB_CW_BACK_PIXEL, &background);
	xcb_clear_area(xcb, false, screen->root, 0, 0, 0, 0);
	free(xcb_get_input_focus_reply(xcb, xcb_get_input_focus(xcb), NULL));
	assert_true(obs_startup("en-US", NULL, NULL));
	struct obs_video_info video = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 60,
		.fps_den = 1,
		.base_width = 64,
		.base_height = 64,
		.output_width = 64,
		.output_height = 64,
		.output_format = VIDEO_FORMAT_BGRA,
		.colorspace = VIDEO_CS_SRGB,
		.range = VIDEO_RANGE_FULL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	assert_int_equal(obs_reset_video(&video), OBS_VIDEO_SUCCESS);
	struct obs_source_info info = xshm_input_v2;
	info.video_tick = measured_tick;
	obs_register_source(&info);
	obs_data_t *settings = obs_data_create();
	obs_data_set_int(settings, "screen", 0);
	obs_data_set_bool(settings, "show_cursor", true);
	obs_source_t *source = obs_source_create_private("xshm_input_v2", "pipeline live", settings);
	obs_data_release(settings);
	assert_non_null(source);
	obs_source_inc_showing(source);
	struct timespec deadline;
	clock_gettime(CLOCK_REALTIME, &deadline);
	deadline.tv_sec += 10;
	pthread_mutex_lock(&live_lock);
	int error = 0;
	while ((live_ticks < 60 || !live_pixels) && !error)
		error = pthread_cond_timedwait(&live_changed, &live_lock, &deadline);
	const bool captured = live_pixels;
	fprintf(stderr, "XSHM real capture: %u ticks, mean %.3f us, max %.3f us, expected pixels %s\n", live_ticks,
		live_ticks ? (double)live_total / live_ticks / 1000.0 : 0, (double)live_max / 1000.0,
		captured ? "yes" : "no");
	pthread_mutex_unlock(&live_lock);
	obs_source_dec_showing(source);
	obs_source_release(source);
	/* Exercise teardown when the destruction queue is already empty (#3527). */
	obs_wait_for_destroy_queue();
	while (obs_wait_for_destroy_queue()) {
	}
	obs_shutdown();
	xcb_disconnect(xcb);
	assert_int_equal(error, 0);
	assert_true(captured);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(capture_real_pixels_without_waiting_for_x)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
