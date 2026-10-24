/* linux-capture: the XComposite window capture follows its window from X
 * events, end to end (#1356).
 *
 * The capture used to ask the X server whether its window was viewable once
 * or twice per frame. It now keeps that state from the Map/Unmap/Destroy
 * events its watcher receives (read with xcb_poll_for_event), with a
 * safety-net round trip at most once a second. This drives the real source
 * (the linux-capture unity file is compiled in) against the Xvfb display
 * ctest runs under: a plain xcb window, listed in a minimal EWMH setup this
 * test publishes in place of a window manager, is mapped, unmapped, mapped
 * again and destroyed while libobs ticks the source. Skips without a
 * display. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "cmocka-compat.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <xcb/xcb.h>
#include <xcb/composite.h>

#include <obs.h>
#include <util/platform.h>
#include <util/threading.h>

#include "plugins/linux-capture/xcomposite-input.h"

#define WIN_W 160
#define WIN_H 120
#define WIN_NAME "obs-xcomposite-test"
#define WIN_CLASS "obs-test\0ObsTest"

static bool obs_up;
static xcb_connection_t *conn;
static xcb_window_t root, wm_check, target;

static pthread_mutex_t sig_mutex = PTHREAD_MUTEX_INITIALIZER;
static int hooked_count, unhooked_count;

static xcb_atom_t atom(const char *name)
{
	xcb_intern_atom_reply_t *r = xcb_intern_atom_reply(conn, xcb_intern_atom(conn, 0, strlen(name), name), NULL);
	xcb_atom_t a = r ? r->atom : XCB_ATOM_NONE;
	free(r);
	return a;
}

/* xcb_aux_sync without libxcb-util. */
static void xcb_aux_sync_like(xcb_connection_t *c)
{
	free(xcb_get_input_focus_reply(c, xcb_get_input_focus(c), NULL));
}

static void set_client_list(xcb_window_t *wins, uint32_t n)
{
	xcb_change_property(conn, XCB_PROP_MODE_REPLACE, root, atom("_NET_CLIENT_LIST"), XCB_ATOM_WINDOW, 32, n, wins);
	xcb_aux_sync_like(conn);
}

static void on_hooked(void *data, calldata_t *cd)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(cd);
	pthread_mutex_lock(&sig_mutex);
	hooked_count++;
	pthread_mutex_unlock(&sig_mutex);
}

static void on_unhooked(void *data, calldata_t *cd)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(cd);
	pthread_mutex_lock(&sig_mutex);
	unhooked_count++;
	pthread_mutex_unlock(&sig_mutex);
}

static int count(int *c)
{
	pthread_mutex_lock(&sig_mutex);
	int v = *c;
	pthread_mutex_unlock(&sig_mutex);
	return v;
}

/* Milliseconds until *c reaches want, or -1 after timeout_ms. */
static int wait_count(int *c, int want, int timeout_ms)
{
	uint64_t start = os_gettime_ns();
	for (;;) {
		int elapsed = (int)((os_gettime_ns() - start) / 1000000);
		if (count(c) >= want)
			return elapsed;
		if (elapsed > timeout_ms)
			return -1;
		os_sleep_ms(5);
	}
}

static int group_setup(void **state)
{
	UNUSED_PARAMETER(state);

	const char *display = getenv("DISPLAY");
	if (!display || !*display)
		return 0;

	conn = xcb_connect(NULL, NULL);
	if (xcb_connection_has_error(conn)) {
		fprintf(stderr, "cannot connect to the X display\n");
		return -1;
	}
	const xcb_query_extension_reply_t *ext = xcb_get_extension_data(conn, &xcb_composite_id);
	if (!ext || !ext->present) {
		fprintf(stderr, "the X server has no Composite extension\n");
		return -1;
	}

	xcb_screen_t *screen = xcb_setup_roots_iterator(xcb_get_setup(conn)).data;
	root = screen->root;

	/* A minimal EWMH window manager: the supporting-WM check window. */
	wm_check = xcb_generate_id(conn);
	xcb_create_window(conn, XCB_COPY_FROM_PARENT, wm_check, root, -10, -10, 1, 1, 0, XCB_WINDOW_CLASS_INPUT_OUTPUT,
			  screen->root_visual, 0, NULL);
	xcb_atom_t check = atom("_NET_SUPPORTING_WM_CHECK");
	xcb_change_property(conn, XCB_PROP_MODE_REPLACE, root, check, XCB_ATOM_WINDOW, 32, 1, &wm_check);
	xcb_change_property(conn, XCB_PROP_MODE_REPLACE, wm_check, check, XCB_ATOM_WINDOW, 32, 1, &wm_check);

	/* The window to capture, unmapped for now. */
	target = xcb_generate_id(conn);
	uint32_t bg = screen->white_pixel;
	xcb_create_window(conn, XCB_COPY_FROM_PARENT, target, root, 0, 0, WIN_W, WIN_H, 0,
			  XCB_WINDOW_CLASS_INPUT_OUTPUT, screen->root_visual, XCB_CW_BACK_PIXEL, &bg);
	xcb_change_property(conn, XCB_PROP_MODE_REPLACE, target, XCB_ATOM_WM_NAME, XCB_ATOM_STRING, 8, strlen(WIN_NAME),
			    WIN_NAME);
	xcb_change_property(conn, XCB_PROP_MODE_REPLACE, target, XCB_ATOM_WM_CLASS, XCB_ATOM_STRING, 8,
			    sizeof(WIN_CLASS), WIN_CLASS);
	set_client_list(&target, 1);

	if (!obs_startup("en-US", NULL, NULL))
		return 0;

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 60,
		.fps_den = 1,
		.base_width = WIN_W,
		.base_height = WIN_H,
		.output_width = WIN_W,
		.output_height = WIN_H,
		.output_format = VIDEO_FORMAT_BGRA,
		.colorspace = VIDEO_CS_SRGB,
		.range = VIDEO_RANGE_FULL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	if (obs_reset_video(&ovi) != OBS_VIDEO_SUCCESS) {
		obs_shutdown();
		return 0;
	}

	xcomposite_load();
	obs_up = true;
	return 0;
}

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (obs_up) {
		obs_shutdown();
		xcomposite_unload();
		obs_up = false;
	}
	if (conn)
		xcb_disconnect(conn);
	return 0;
}

static void map_target(bool map)
{
	if (map)
		xcb_map_window(conn, target);
	else
		xcb_unmap_window(conn, target);
	xcb_aux_sync_like(conn);
}

static void window_follows_map_unmap_destroy(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	map_target(true);

	char setting[256];
	snprintf(setting, sizeof(setting), "%u\r\n%s\r\n%s", (unsigned)target, WIN_NAME, "obs-test");
	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, "capture_window", setting);
	obs_source_t *source = obs_source_create_private("xcomposite_input", "xcomposite test", settings);
	obs_data_release(settings);
	assert_non_null(source);

	signal_handler_t *sh = obs_source_get_signal_handler(source);
	signal_handler_connect(sh, "hooked", on_hooked, NULL);
	signal_handler_connect(sh, "unhooked", on_unhooked, NULL);
	obs_source_inc_showing(source);

	/* (1) Mapped: xcompcap_update hooked the window. Xvfb's llvmpipe cannot
	 * import the window pixmap as an EGLImage ("Cannot create EGLImage"), so
	 * the source has no texture and reports 0x0 here; the hooked state is
	 * what this checks. The rounds below run with that texture missing, the
	 * path a capture takes while its pixmap cannot be made. */
	calldata_t cd = {0};
	proc_handler_call(obs_source_get_proc_handler(source), "get_hooked", &cd);
	bool hooked = calldata_bool(&cd, "hooked");
	fprintf(stderr, "mapped: hooked=%d name=%s\n", hooked, calldata_string(&cd, "name"));
	calldata_free(&cd);
	assert_true(hooked);

	/* Let a safety-net check pass, so the next one is about a second away. */
	os_sleep_ms(1100);

	/* (2) Unmapped: the UnmapNotify reports the window lost well before the
	 * once-a-second safety net could. Three rounds, so a safety-net check
	 * that happens to land in the window cannot pass all of them. */
	for (int round = 0; round < 3; round++) {
		int before_unhooked = count(&unhooked_count);
		int before_hooked = count(&hooked_count);
		map_target(false);
		int ms = wait_count(&unhooked_count, before_unhooked + 1, 2000);
		fprintf(stderr, "round %d: unhooked after %d ms\n", round, ms);
		assert_int_in_range(ms, 0, 400);

		/* (3) Mapped again: the MapNotify forces a search; it recovers. */
		map_target(true);
		ms = wait_count(&hooked_count, before_hooked + 1, 3000);
		fprintf(stderr, "round %d: hooked again after %d ms\n", round, ms);
		assert_int_in_range(ms, 0, 2000);
		os_sleep_ms(300);
	}

	/* (4) Destroyed: the source unhooks. */
	int before_unhooked = count(&unhooked_count);
	set_client_list(NULL, 0);
	xcb_destroy_window(conn, target);
	xcb_aux_sync_like(conn);
	int ms = wait_count(&unhooked_count, before_unhooked + 1, 3000);
	fprintf(stderr, "destroyed: unhooked after %d ms\n", ms);
	assert_int_in_range(ms, 0, 2000);

	signal_handler_disconnect(sh, "hooked", on_hooked, NULL);
	signal_handler_disconnect(sh, "unhooked", on_unhooked, NULL);
	obs_source_dec_showing(source);
	obs_source_release(source);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(window_follows_map_unmap_destroy),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
