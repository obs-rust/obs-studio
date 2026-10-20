/* linux-capture: the XSHM screen capture (Screen Capture (XSHM)) follows a
 * change of the X screen's resolution (upstream obsproject/obs-studio
 * PR #12937). It read the screen's geometry once, when it started, and
 * kept capturing the old area: after the screen shrank its frames stopped
 * (the X server refuses an image larger than the screen), and after it
 * grew the capture stayed cropped to the old size.
 *
 * This drives the real source (the linux-capture unity file is compiled
 * in) against the Xvfb display ctest runs under: it shrinks the screen
 * through RandR, as xrandr --output ... --mode ... --fb ... does, checks the
 * source's size follows, then restores the screen and checks again. It
 * changes the screen everyone on that display sees, so the test runs alone
 * (RUN_SERIAL). Skips without a display, RandR or MIT-SHM. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <xcb/xcb.h>
#include <xcb/randr.h>
#include <xcb/shm.h>

#include <obs.h>
#include <util/platform.h>

extern struct obs_source_info xshm_input_v2;

static bool obs_up;
static xcb_connection_t *conn;
static xcb_window_t root;
static xcb_randr_crtc_t crtc;
static xcb_randr_output_t output;
static uint16_t orig_w, orig_h;

static void root_size(uint16_t *w, uint16_t *h)
{
	xcb_get_geometry_reply_t *g = xcb_get_geometry_reply(conn, xcb_get_geometry(conn, root), NULL);
	*w = g ? g->width : 0;
	*h = g ? g->height : 0;
	free(g);
}

/* The output's mode of w x h, created and added to the output if needed. */
static xcb_randr_mode_t find_or_create_mode(uint16_t w, uint16_t h)
{
	xcb_randr_get_screen_resources_reply_t *res =
		xcb_randr_get_screen_resources_reply(conn, xcb_randr_get_screen_resources(conn, root), NULL);
	xcb_randr_mode_t mode = XCB_NONE;

	if (!res)
		return XCB_NONE;

	xcb_randr_mode_info_t *modes = xcb_randr_get_screen_resources_modes(res);
	for (int i = 0; i < res->num_modes; i++) {
		if (modes[i].width == w && modes[i].height == h)
			mode = modes[i].id;
	}
	free(res);
	if (mode != XCB_NONE)
		return mode;

	char name[32];
	snprintf(name, sizeof(name), "%ux%u", w, h);
	xcb_randr_mode_info_t info = {0};
	info.width = w;
	info.height = h;
	info.dot_clock = (uint32_t)w * h * 60;
	info.hsync_start = info.hsync_end = info.htotal = w;
	info.vsync_start = info.vsync_end = info.vtotal = h;
	info.name_len = (uint16_t)strlen(name);

	xcb_randr_create_mode_reply_t *created =
		xcb_randr_create_mode_reply(conn, xcb_randr_create_mode(conn, root, info, info.name_len, name), NULL);
	if (!created)
		return XCB_NONE;
	mode = created->mode;
	free(created);

	xcb_generic_error_t *err = xcb_request_check(conn, xcb_randr_add_output_mode_checked(conn, output, mode));
	if (err) {
		free(err);
		return XCB_NONE;
	}
	return mode;
}

static bool set_screen_size_only(uint16_t w, uint16_t h)
{
	/* Keep 96 DPI. */
	xcb_generic_error_t *err =
		xcb_request_check(conn, xcb_randr_set_screen_size_checked(conn, root, w, h, (uint32_t)w * 254 / 960,
									  (uint32_t)h * 254 / 960));
	free(err);
	return !err;
}

/* Resize the screen to w x h with its CRTC showing all of it. */
static bool set_screen_size(uint16_t w, uint16_t h)
{
	uint16_t cur_w, cur_h;
	root_size(&cur_w, &cur_h);
	const bool grow = w > cur_w || h > cur_h;

	xcb_randr_mode_t mode = find_or_create_mode(w, h);
	if (mode == XCB_NONE)
		return false;

	if (grow && !set_screen_size_only(w, h))
		return false;

	xcb_randr_set_crtc_config_reply_t *set = xcb_randr_set_crtc_config_reply(
		conn,
		xcb_randr_set_crtc_config(conn, crtc, XCB_CURRENT_TIME, XCB_CURRENT_TIME, 0, 0, mode,
					  XCB_RANDR_ROTATION_ROTATE_0, 1, &output),
		NULL);
	const bool ok = set && set->status == XCB_RANDR_SET_CONFIG_SUCCESS;
	free(set);
	if (!ok)
		return false;

	if (!grow && !set_screen_size_only(w, h))
		return false;

	root_size(&cur_w, &cur_h);
	return cur_w == w && cur_h == h;
}

static int group_setup(void **state)
{
	(void)state;

	const char *display = getenv("DISPLAY");
	if (!display || !*display)
		return 0;

	conn = xcb_connect(NULL, NULL);
	if (xcb_connection_has_error(conn)) {
		fprintf(stderr, "cannot connect to the X display\n");
		return -1;
	}
	root = xcb_setup_roots_iterator(xcb_get_setup(conn)).data->root;

	const xcb_query_extension_reply_t *randr = xcb_get_extension_data(conn, &xcb_randr_id);
	const xcb_query_extension_reply_t *shm = xcb_get_extension_data(conn, &xcb_shm_id);
	if (!randr || !randr->present || !shm || !shm->present) {
		fprintf(stderr, "the X server has no RandR or MIT-SHM, skipping\n");
		return 0;
	}

	xcb_randr_get_screen_resources_reply_t *res =
		xcb_randr_get_screen_resources_reply(conn, xcb_randr_get_screen_resources(conn, root), NULL);
	if (!res || !res->num_crtcs || !res->num_outputs) {
		fprintf(stderr, "the X server has no RandR CRTC or output, skipping\n");
		free(res);
		return 0;
	}
	crtc = xcb_randr_get_screen_resources_crtcs(res)[0];
	output = xcb_randr_get_screen_resources_outputs(res)[0];
	free(res);
	root_size(&orig_w, &orig_h);

	if (!obs_startup("en-US", NULL, NULL))
		return 0;

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = 320,
		.base_height = 240,
		.output_width = 320,
		.output_height = 240,
		.output_format = VIDEO_FORMAT_BGRA,
		.colorspace = VIDEO_CS_SRGB,
		.range = VIDEO_RANGE_FULL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	if (obs_reset_video(&ovi) != OBS_VIDEO_SUCCESS) {
		obs_shutdown();
		return 0;
	}
	obs_register_source(&xshm_input_v2);
	obs_up = true;
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up) {
		obs_shutdown();
		obs_up = false;
	}
	if (conn) {
		uint16_t w, h;
		root_size(&w, &h);
		if (orig_w && (w != orig_w || h != orig_h))
			set_screen_size(orig_w, orig_h);
		xcb_disconnect(conn);
	}
	return 0;
}

/* Milliseconds until the source is w x h, or -1 after timeout_ms. */
static int wait_size(obs_source_t *source, uint32_t w, uint32_t h, int timeout_ms)
{
	const uint64_t start = os_gettime_ns();
	for (;;) {
		const int elapsed = (int)((os_gettime_ns() - start) / 1000000);
		if (obs_source_get_width(source) == w && obs_source_get_height(source) == h)
			return elapsed;
		if (elapsed > timeout_ms)
			return -1;
		os_sleep_ms(10);
	}
}

static void capture_follows_screen_size(void **state)
{
	(void)state;

	if (!obs_up)
		skip();

	obs_data_t *settings = obs_data_create();
	obs_data_set_int(settings, "screen", 0);
	obs_data_set_bool(settings, "show_cursor", false);
	obs_source_t *source = obs_source_create_private("xshm_input_v2", "xshm test", settings);
	obs_data_release(settings);
	assert_non_null(source);
	obs_source_inc_showing(source);

	/* Started: the whole screen. */
	int ms = wait_size(source, orig_w, orig_h, 2000);
	fprintf(stderr, "start: %ux%u after %d ms\n", obs_source_get_width(source), obs_source_get_height(source), ms);
	assert_true(ms >= 0);

	/* Shrunk to three quarters. */
	const uint16_t small_w = orig_w / 4 * 3, small_h = orig_h / 4 * 3;
	assert_true(set_screen_size(small_w, small_h));
	ms = wait_size(source, small_w, small_h, 2000);
	fprintf(stderr, "screen %ux%u: source %ux%u after %d ms\n", small_w, small_h, obs_source_get_width(source),
		obs_source_get_height(source), ms);
	const bool followed_shrink = ms >= 0;

	/* Back to the original size. */
	assert_true(set_screen_size(orig_w, orig_h));
	ms = wait_size(source, orig_w, orig_h, 2000);
	fprintf(stderr, "screen %ux%u: source %ux%u after %d ms\n", orig_w, orig_h, obs_source_get_width(source),
		obs_source_get_height(source), ms);
	const bool followed_grow = ms >= 0;

	obs_source_dec_showing(source);
	obs_source_release(source);

	assert_true(followed_shrink);
	assert_true(followed_grow);
}

/* The source's size stays put while the screen does not change: no
 * geometry change is made up between frames. */
static void capture_keeps_size_without_change(void **state)
{
	(void)state;

	if (!obs_up)
		skip();

	obs_data_t *settings = obs_data_create();
	obs_data_set_int(settings, "screen", 0);
	obs_source_t *source = obs_source_create_private("xshm_input_v2", "xshm steady", settings);
	obs_data_release(settings);
	assert_non_null(source);
	obs_source_inc_showing(source);

	assert_true(wait_size(source, orig_w, orig_h, 2000) >= 0);
	for (int i = 0; i < 50; i++) {
		assert_int_equal(obs_source_get_width(source), orig_w);
		assert_int_equal(obs_source_get_height(source), orig_h);
		os_sleep_ms(10);
	}

	obs_source_dec_showing(source);
	obs_source_release(source);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(capture_keeps_size_without_change),
		cmocka_unit_test(capture_follows_screen_size),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
