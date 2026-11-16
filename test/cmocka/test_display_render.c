/*
 * Characterize the display render (render_display() in libobs/obs-display.c,
 * which render_displays() calls for every display on each frame of the
 * graphics thread) and the display create with a swap chain, where
 * test_display_lifecycle does not reach: it makes its displays with no swap
 * chain on a core with no video.
 *
 * Here the core runs video on libobs-opengl, and each display gets a real
 * swap chain: a child of the X root window, so it needs an X display (Linux,
 * under xvfb-run). Without one, or when the video does not start, each case
 * skips. It pins:
 * - create with graphics data: the swap chain made, cx, cy, next_cx and
 *   next_cy all the requested size, and the display on the list;
 * - create when the swap chain cannot be made (no parent window): NULL, the
 *   error line logged once, and the list as it was;
 * - the render: each draw callback called in the order added, a pair added
 *   twice called twice, each with next_cx and next_cy; remove taking the
 *   first match only;
 * - resize and update_color_space: the next frame draws at the new size,
 *   cx and cy take it, and the colour-space flag is cleared;
 * - a disabled display: no callback runs, and its size, next size and
 *   colour-space flag stay as they were, while the other displays draw.
 *
 * The callbacks run on the graphics thread. Every wait is for the record it
 * checks: each callback signals an event, and the waits wake on it, with a
 * 10 s bound that only turns a hang into a failure. Records
 * are cleared while holding draw_callbacks_mutex, which the render holds
 * across its callback loop, so a cleared record starts at a whole pass.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "cmocka-compat.h"

#include <X11/Xlib.h>

#include "obs-internal.h"
#include "obs-nix-platform.h"
#include <util/platform.h>
#include <util/threading.h>

#define MAX_RECORDS 64
#define WAIT_NS (10ULL * 1000000000ULL)

static bool obs_up;
static Display *xdisplay;

struct record {
	int tag;
	uint32_t cx;
	uint32_t cy;
};

/* What the callbacks saw, on the graphics thread. */
static pthread_mutex_t records_mutex = PTHREAD_MUTEX_INITIALIZER;
static struct record records[MAX_RECORDS];
static size_t num_records;

static os_event_t *drawn;

static int tag_a = 1;
static int tag_b = 2;
static int tag_c = 3;

static void draw(void *param, uint32_t cx, uint32_t cy)
{
	pthread_mutex_lock(&records_mutex);
	if (num_records < MAX_RECORDS) {
		records[num_records].tag = *(int *)param;
		records[num_records].cx = cx;
		records[num_records].cy = cy;
		num_records++;
	}
	pthread_mutex_unlock(&records_mutex);
	os_event_signal(drawn);
}

/* Clear the records between two passes of this display's callbacks. */
static void clear_records(obs_display_t *display)
{
	pthread_mutex_lock(&display->draw_callbacks_mutex);
	pthread_mutex_lock(&records_mutex);
	num_records = 0;
	pthread_mutex_unlock(&records_mutex);
	pthread_mutex_unlock(&display->draw_callbacks_mutex);
}

/* Wait until at least `count` records exist; copy them out. */
static size_t wait_records(size_t count, struct record *out)
{
	const uint64_t start = os_gettime_ns();
	for (;;) {
		pthread_mutex_lock(&records_mutex);
		const size_t num = num_records;
		memcpy(out, records, sizeof(records));
		pthread_mutex_unlock(&records_mutex);
		if (num >= count)
			return num;
		if (os_gettime_ns() - start > WAIT_NS) {
			fprintf(stderr, "waited 10 s for %zu records, have %zu\n", count, num);
			fail();
		}
		os_event_timedwait(drawn, 100);
	}
}

/* Wait until a record of `tag` at cx x cy exists. */
static void wait_size(int tag, uint32_t cx, uint32_t cy)
{
	const uint64_t start = os_gettime_ns();
	for (;;) {
		bool found = false;
		pthread_mutex_lock(&records_mutex);
		for (size_t i = 0; i < num_records; i++)
			found = found || (records[i].tag == tag && records[i].cx == cx && records[i].cy == cy);
		if (num_records == MAX_RECORDS)
			num_records = 0;
		pthread_mutex_unlock(&records_mutex);
		if (found)
			return;
		if (os_gettime_ns() - start > WAIT_NS) {
			fprintf(stderr, "waited 10 s for a %ux%u draw of tag %d\n", cx, cy, tag);
			fail();
		}
		os_event_timedwait(drawn, 100);
	}
}

static obs_display_t *create(uint32_t cx, uint32_t cy, uint32_t color)
{
	struct gs_init_data data = {0};
	data.window.id = (uint32_t)DefaultRootWindow(xdisplay);
	data.window.display = obs_get_nix_platform_display();
	data.cx = cx;
	data.cy = cy;
	data.format = GS_BGRA;
	data.zsformat = GS_ZS_NONE;
	return obs_display_create(&data, color);
}

#define REQUIRE_VIDEO()                                                                  \
	do {                                                                             \
		if (!obs_up) {                                                           \
			fprintf(stderr, "skipped: no video (no X display or no GL)\n"); \
			skip();                                                          \
		}                                                                        \
	} while (0)

static void test_create_with_swap(void **state)
{
	(void)state;
	REQUIRE_VIDEO();

	obs_display_t *display = create(320, 240, 0xff102030);
	assert_non_null(display);

	assert_non_null(display->swap);
	assert_int_equal(display->cx, 320);
	assert_int_equal(display->cy, 240);
	assert_int_equal(display->next_cx, 320);
	assert_int_equal(display->next_cy, 240);
	assert_int_equal(display->background_color, 0xff102030);
	assert_true(display->enabled);
	assert_false(display->update_color_space);
	assert_ptr_equal(*display->prev_next, display);

	obs_display_destroy(display);
}

static volatile long error_lines;

static void count_log(int level, const char *format, va_list args, void *param)
{
	(void)args;
	(void)param;
	if (level == LOG_ERROR && strcmp(format, "obs_display_init: Failed to create swap chain") == 0)
		os_atomic_inc_long(&error_lines);
}

static void test_create_fails(void **state)
{
	(void)state;
	REQUIRE_VIDEO();

	obs_display_t *other = create(64, 64, 0);
	assert_non_null(other);
	obs_display_t **head = other->prev_next;

	/* No parent window: the swap chain cannot be made. */
	struct gs_init_data data = {0};
	data.window.display = obs_get_nix_platform_display();
	data.cx = 64;
	data.cy = 64;
	data.format = GS_BGRA;
	data.zsformat = GS_ZS_NONE;

	error_lines = 0;
	base_set_log_handler(count_log, NULL);
	obs_display_t *display = obs_display_create(&data, 0);
	base_set_log_handler(NULL, NULL);

	assert_null(display);
	assert_int_equal(os_atomic_load_long(&error_lines), 1);
	/* The list is as it was: the other display alone at the head. */
	assert_ptr_equal(*head, other);
	assert_ptr_equal(other->prev_next, head);
	assert_null(other->next);

	obs_display_destroy(other);
}

static void test_render_callbacks(void **state)
{
	(void)state;
	REQUIRE_VIDEO();

	struct record seen[MAX_RECORDS];
	obs_display_t *display = create(320, 240, 0);
	assert_non_null(display);

	obs_display_add_draw_callback(display, draw, &tag_a);
	obs_display_add_draw_callback(display, draw, &tag_b);
	obs_display_add_draw_callback(display, draw, &tag_a);

	/* One whole pass: in the order added, the pair twice, at next_cx x
	 * next_cy. */
	clear_records(display);
	wait_records(3, seen);
	assert_int_equal(seen[0].tag, tag_a);
	assert_int_equal(seen[1].tag, tag_b);
	assert_int_equal(seen[2].tag, tag_a);
	for (size_t i = 0; i < 3; i++) {
		assert_int_equal(seen[i].cx, 320);
		assert_int_equal(seen[i].cy, 240);
	}

	/* Remove takes the first match: b, then a. */
	obs_display_remove_draw_callback(display, draw, &tag_a);
	clear_records(display);
	wait_records(2, seen);
	assert_int_equal(seen[0].tag, tag_b);
	assert_int_equal(seen[1].tag, tag_a);

	/* Remove of a pair that is not there changes nothing. */
	obs_display_remove_draw_callback(display, draw, &tag_c);
	clear_records(display);
	wait_records(2, seen);
	assert_int_equal(seen[0].tag, tag_b);
	assert_int_equal(seen[1].tag, tag_a);

	obs_display_destroy(display);
}

static void test_render_resize(void **state)
{
	(void)state;
	REQUIRE_VIDEO();

	obs_display_t *display = create(320, 240, 0);
	assert_non_null(display);
	obs_display_add_draw_callback(display, draw, &tag_a);
	clear_records(display);

	/* resize leaves cx and cy to the render. Holding graphics keeps the
	 * render off meanwhile: render_displays() enters the graphics context
	 * before it renders any display. */
	obs_enter_graphics();
	obs_display_resize(display, 200, 100);
	obs_display_update_color_space(display);
	const uint32_t pending_cx = display->cx;
	const uint32_t pending_cy = display->cy;
	obs_leave_graphics();
	assert_int_equal(pending_cx, 320);
	assert_int_equal(pending_cy, 240);

	wait_size(tag_a, 200, 100);

	/* The render stored the size it drew at and cleared the flag. */
	pthread_mutex_lock(&display->draw_info_mutex);
	assert_int_equal(display->cx, 200);
	assert_int_equal(display->cy, 100);
	assert_int_equal(display->next_cx, 200);
	assert_int_equal(display->next_cy, 100);
	assert_false(display->update_color_space);
	pthread_mutex_unlock(&display->draw_info_mutex);

	/* The colour-space flag alone, at the same size: cleared too. */
	obs_display_update_color_space(display);
	clear_records(display);
	struct record seen[MAX_RECORDS];
	wait_records(1, seen);
	clear_records(display);
	wait_records(1, seen);
	pthread_mutex_lock(&display->draw_info_mutex);
	assert_false(display->update_color_space);
	assert_int_equal(display->cx, 200);
	assert_int_equal(display->cy, 100);
	pthread_mutex_unlock(&display->draw_info_mutex);

	obs_display_destroy(display);
}

static volatile long disabled_calls;

static void draw_disabled(void *param, uint32_t cx, uint32_t cy)
{
	(void)param;
	(void)cx;
	(void)cy;
	os_atomic_inc_long(&disabled_calls);
}

static void test_render_disabled(void **state)
{
	(void)state;
	REQUIRE_VIDEO();

	obs_display_t *off = create(320, 240, 0);
	obs_display_t *on = create(160, 120, 0);
	assert_non_null(off);
	assert_non_null(on);

	disabled_calls = 0;
	obs_display_set_enabled(off, false);
	obs_display_add_draw_callback(off, draw_disabled, NULL);
	obs_display_resize(off, 64, 32);
	obs_display_update_color_space(off);

	obs_display_add_draw_callback(on, draw, &tag_b);
	clear_records(on);

	/* Three passes of the enabled display: each frame visits both. */
	struct record seen[MAX_RECORDS];
	wait_records(3, seen);
	assert_int_equal(seen[0].tag, tag_b);
	assert_int_equal(seen[0].cx, 160);
	assert_int_equal(seen[0].cy, 120);

	assert_int_equal(os_atomic_load_long(&disabled_calls), 0);
	pthread_mutex_lock(&off->draw_info_mutex);
	assert_int_equal(off->cx, 320);
	assert_int_equal(off->cy, 240);
	assert_int_equal(off->next_cx, 64);
	assert_int_equal(off->next_cy, 32);
	assert_true(off->update_color_space);
	pthread_mutex_unlock(&off->draw_info_mutex);

	/* Enabled again: it draws at the size it was given. */
	obs_display_set_enabled(off, true);
	obs_display_remove_draw_callback(off, draw_disabled, NULL);
	obs_display_add_draw_callback(off, draw, &tag_c);
	wait_size(tag_c, 64, 32);

	obs_display_destroy(on);
	obs_display_destroy(off);
}

static int group_setup(void **state)
{
	(void)state;
	if (os_event_init(&drawn, OS_EVENT_TYPE_AUTO) != 0)
		return -1;
	xdisplay = XOpenDisplay(NULL);
	if (!xdisplay) {
		fprintf(stderr, "skipped: no X display\n");
		return 0;
	}
	if (!obs_startup("en-US", NULL, NULL)) {
		fprintf(stderr, "skipped: obs_startup failed\n");
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
		fprintf(stderr, "skipped: obs_reset_video failed: %d\n", ret);
		obs_shutdown();
		return 0;
	}
	obs_up = true;
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_initialized())
		obs_shutdown();
	if (xdisplay)
		XCloseDisplay(xdisplay);
	os_event_destroy(drawn);
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_create_with_swap), cmocka_unit_test(test_create_fails),
		cmocka_unit_test(test_render_callbacks), cmocka_unit_test(test_render_resize),
		cmocka_unit_test(test_render_disabled),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
