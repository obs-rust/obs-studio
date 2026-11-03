/* #3204: drive the real capture tick with a delayed X transport. No graphics
 * context is needed: uploads record the completed shared-memory bytes. */
#include "plugins/linux-capture/build/linux-capture-nix.pch.h"
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

static unsigned requests, waits, uploads, flushes, discards;
static unsigned cursor_requests, cursor_uploads;
static bool ready, writing, cursor_ready, failed;
static uint8_t shared_pixel[4];

const char *obs_module_text(const char *key)
{
	return key;
}

static bool pipeline_showing(const obs_source_t *source)
{
	(void)source;
	return true;
}

static xcb_generic_event_t *pipeline_event(xcb_connection_t *xcb)
{
	(void)xcb;
	return NULL;
}

static xcb_shm_get_image_cookie_t pipeline_request(xcb_connection_t *xcb, xcb_drawable_t drawable, int16_t x, int16_t y,
						   uint16_t width, uint16_t height, uint32_t mask, uint8_t format,
						   xcb_shm_seg_t segment, uint32_t offset)
{
	(void)xcb;
	(void)drawable;
	(void)x;
	(void)y;
	(void)width;
	(void)height;
	(void)mask;
	(void)format;
	(void)segment;
	(void)offset;
	assert_false(writing);
	writing = true;
	return (xcb_shm_get_image_cookie_t){++requests};
}

static void *pipeline_completed(void)
{
	writing = false;
	shared_pixel[0] = (uint8_t)requests;
	return failed ? NULL : calloc(1, sizeof(xcb_shm_get_image_reply_t));
}

static xcb_shm_get_image_reply_t *pipeline_wait(xcb_connection_t *xcb, xcb_shm_get_image_cookie_t cookie,
						xcb_generic_error_t **error)
{
	(void)xcb;
	(void)cookie;
	(void)error;
	waits++;
	return pipeline_completed();
}

int pipeline_poll(xcb_connection_t *xcb, unsigned sequence, void **reply, xcb_generic_error_t **error)
{
	(void)xcb;
	(void)error;
	if (sequence >= 1000) {
		if (!cursor_ready)
			return 0;
		*reply = calloc(1, sizeof(xcb_xfixes_get_cursor_image_reply_t));
		return 1;
	}
	if (!ready)
		return 0;
	*reply = pipeline_completed();
	return 1;
}

static int pipeline_flush(xcb_connection_t *xcb)
{
	(void)xcb;
	flushes++;
	return 1;
}

void pipeline_discard(xcb_connection_t *xcb, unsigned sequence)
{
	(void)xcb;
	(void)sequence;
	discards++;
}

static void pipeline_upload(gs_texture_t *texture, const uint8_t *data, uint32_t stride, bool flip)
{
	(void)texture;
	(void)stride;
	(void)flip;
	assert_false(writing);
	assert_int_equal(data[0], requests);
	uploads++;
}

static void pipeline_graphics(void) {}
static void pipeline_cursor(xcb_connection_t *xcb, xcb_xcursor_t *cursor)
{
	(void)xcb;
	(void)cursor;
}

static xcb_xfixes_get_cursor_image_cookie_t pipeline_cursor_request(xcb_connection_t *xcb)
{
	(void)xcb;
	return (xcb_xfixes_get_cursor_image_cookie_t){1000 + ++cursor_requests};
}

static void pipeline_cursor_reply(xcb_xcursor_t *cursor, xcb_xfixes_get_cursor_image_reply_t *reply)
{
	(void)cursor;
	assert_non_null(reply);
	cursor_uploads++;
}

/* Headers are already parsed; substitute only the transport and GPU edges of
 * the implementation. All capture state transitions remain production code. */
#define obs_source_showing pipeline_showing
#define xcb_poll_for_event pipeline_event
#define xcb_shm_get_image_unchecked pipeline_request
#define xcb_shm_get_image_reply pipeline_wait
#define xcb_poll_for_reply pipeline_poll
#define xcb_flush pipeline_flush
#define xcb_discard_reply pipeline_discard
#define gs_texture_set_image pipeline_upload
#define obs_enter_graphics pipeline_graphics
#define obs_leave_graphics pipeline_graphics
#define xcb_xcursor_update pipeline_cursor
#define xcb_xfixes_get_cursor_image_unchecked pipeline_cursor_request
#define xcb_xcursor_update_reply pipeline_cursor_reply
#include "plugins/linux-capture/xshm-input.c.h"

static int pipeline_setup(void **state)
{
	(void)state;
	requests = waits = uploads = flushes = discards = cursor_requests = cursor_uploads = 0;
	ready = writing = cursor_ready = failed = false;
	return 0;
}

static void delayed_reply_never_blocks_or_reuses_buffer(void **state)
{
	(void)state;
	xcb_screen_t screen = {0};
	xcb_shm_t shm = {.data = shared_pixel};
	struct xshm_data data = {
		.xcb = (xcb_connection_t *)&screen,
		.xcb_screen = &screen,
		.xshm = &shm,
		.texture = (gs_texture_t *)&screen,
		.adj_width = 1,
		.adj_height = 1,
	};
	/* The first tick submits; an unavailable reply leaves the buffer alone. */
	xshm_video_tick(&data, 0);
	assert_int_equal(waits, 0);
	assert_int_equal(requests, 1);
	assert_int_equal(uploads, 0);
	assert_int_equal(flushes, 1);
	xshm_video_tick(&data, 0);
	assert_int_equal(requests, 1);
	assert_int_equal(uploads, 0);
	/* A completed frame is uploaded before another request can write it. */
	ready = true;
	xshm_video_tick(&data, 0);
	assert_int_equal(uploads, 1);
	assert_int_equal(requests, 2);
	assert_int_equal(waits, 0);
	assert_int_equal(flushes, 2);
	xshm_discard_capture(&data);
	assert_int_equal(discards, 1);
	assert_false(data.image_pending);
}

static void cursor_reply_does_not_block_or_accumulate(void **state)
{
	(void)state;
	xcb_screen_t screen = {0};
	xcb_shm_t shm = {.data = shared_pixel};
	struct xshm_data data = {
		.xcb = (xcb_connection_t *)&screen,
		.xcb_screen = &screen,
		.xshm = &shm,
		.texture = (gs_texture_t *)&screen,
		.show_cursor = true,
		.adj_width = 1,
		.adj_height = 1,
	};
	xshm_video_tick(&data, 0);
	assert_int_equal(cursor_requests, 1);
	ready = true;
	xshm_video_tick(&data, 0);
	assert_int_equal(uploads, 1);
	assert_int_equal(requests, 2);
	assert_int_equal(cursor_requests, 1);
	assert_int_equal(cursor_uploads, 0);
	cursor_ready = true;
	xshm_video_tick(&data, 0);
	assert_int_equal(cursor_uploads, 1);
	assert_int_equal(requests, 3);
	assert_int_equal(cursor_requests, 2);
	xshm_discard_capture(&data);
	assert_int_equal(discards, 2);
	assert_false(data.image_pending);
	assert_false(data.cursor_pending);
}

static void failed_reply_preserves_texture_and_retries(void **state)
{
	(void)state;
	xcb_screen_t screen = {0};
	xcb_shm_t shm = {.data = shared_pixel};
	struct xshm_data data = {
		.xcb = (xcb_connection_t *)&screen,
		.xcb_screen = &screen,
		.xshm = &shm,
		.texture = (gs_texture_t *)&screen,
		.adj_width = 1,
		.adj_height = 1,
	};
	xshm_video_tick(&data, 0);
	ready = failed = true;
	xshm_video_tick(&data, 0);
	assert_int_equal(uploads, 0);
	assert_int_equal(requests, 2);
	assert_false(data.frame_ready);
	failed = false;
	xshm_video_tick(&data, 0);
	assert_int_equal(uploads, 1);
	assert_true(data.frame_ready);
	xshm_discard_capture(&data);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup(delayed_reply_never_blocks_or_reuses_buffer, pipeline_setup),
		cmocka_unit_test_setup(cursor_reply_does_not_block_or_accumulate, pipeline_setup),
		cmocka_unit_test_setup(failed_reply_preserves_texture_and_retries, pipeline_setup),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
