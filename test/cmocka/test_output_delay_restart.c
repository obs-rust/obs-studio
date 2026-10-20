/* A delayed output that stops itself must start normally the next time.
 *
 * With a stream delay, the output's data capture begins twice: once when the
 * output is started (the encoders start and their packets go into the delay
 * queue) and once when the delayed start reaches the output and it connects
 * (begin_delayed_capture(), which marks the output as capturing, so that the
 * delayed packets are passed on from then on, and emits "start"). Ending the
 * data capture clears that mark, but only while the delay is still active.
 *
 * obs_output_signal_stop() without a reconnect (rtmp_output reports a lost
 * connection with OBS_OUTPUT_DISCONNECTED when auto-reconnect is off, and an
 * encoder error with OBS_OUTPUT_ENCODE_ERROR) cleared the delay before it
 * ended the data capture, so the mark stayed set. On the next start, the
 * delayed packets reached the output's encoded_packet() before the output had
 * connected, and when it connected obs_output_begin_data_capture() returned
 * false without emitting "start" (the UI never shows the stream as started)
 * or resetting the packet data.
 *
 * The test registers an encoded, video-only output that behaves like
 * rtmp_output: start() returns true and the test "connects" later by calling
 * obs_output_begin_data_capture(). A registered video encoder on a video
 * output the test drives gives the packets. The delay is 1 s of real time.
 * Core video needs a graphics module and libobs needs an X display on Linux,
 * so the test skips without one (run under xvfb-run). */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <obs.h>
#include <media-io/video-frame.h>
#include <util/platform.h>
#include <util/threading.h>

#include "obs-internal.h"

#define OUTPUT_ID "test_delay_restart_output"
#define ENCODER_ID "test_delay_restart_video"
#define FRAME_MS 20
#define WAIT_MS 5000

static struct {
	obs_output_t *output;
	obs_encoder_t *encoder;
	video_t *video;
	uint64_t next_ts;
	atomic_bool connected;
	atomic_long start_calls;
	atomic_long packets;
	atomic_long packets_before_connect;
	atomic_long start_signals;
} h;

static const char *test_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "delay restart test";
}

static void *encoder_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	UNUSED_PARAMETER(settings);
	return encoder;
}

static void test_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static bool encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received)
{
	UNUSED_PARAMETER(data);
	static uint8_t payload;
	packet->type = OBS_ENCODER_VIDEO;
	packet->data = &payload;
	packet->size = sizeof(payload);
	packet->pts = packet->dts = frame->pts;
	packet->keyframe = true;
	*received = true;
	return true;
}

static void *output_create(obs_data_t *settings, obs_output_t *output)
{
	UNUSED_PARAMETER(settings);
	return output;
}

/* Like rtmp_output: connect later, on another thread (here, the test's). */
static bool output_start(void *data)
{
	UNUSED_PARAMETER(data);
	atomic_store(&h.connected, false);
	atomic_fetch_add(&h.start_calls, 1);
	return true;
}

static void output_stop(void *data, uint64_t ts)
{
	UNUSED_PARAMETER(ts);
	obs_output_end_data_capture(data);
}

static void output_packet(void *data, struct encoder_packet *packet)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(packet);
	atomic_fetch_add(&h.packets, 1);
	if (!atomic_load(&h.connected))
		atomic_fetch_add(&h.packets_before_connect, 1);
}

static void on_start(void *data, calldata_t *cd)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(cd);
	atomic_fetch_add(&h.start_signals, 1);
}

/* One frame of the driven video output, then a frame's worth of real time:
 * each packet the encoder makes passes the delay queue, which hands on the
 * entries older than the delay. */
static void feed_frame(void)
{
	struct video_frame frame;
	if (video_output_lock_frame(h.video, &frame, 1, h.next_ts)) {
		memset(frame.data[0], 0, frame.linesize[0] * 64);
		video_output_unlock_frame(h.video);
	}
	h.next_ts += FRAME_MS * 1000000ULL;
	os_sleep_ms(FRAME_MS);
}

static bool feed_until_start_calls(long n)
{
	for (int t = 0; t < WAIT_MS; t += FRAME_MS) {
		if (atomic_load(&h.start_calls) >= n)
			return true;
		feed_frame();
	}
	return false;
}

static bool feed_until_inactive(void)
{
	for (int t = 0; t < WAIT_MS; t += FRAME_MS) {
		if (!obs_output_active(h.output))
			return true;
		feed_frame();
	}
	return false;
}

static void feed_frames(int n)
{
	for (int i = 0; i < n; i++)
		feed_frame();
}

/* The delayed start reached the output: it "connects" after a few more
 * frames (rtmp_output connects on its own thread while packets keep coming),
 * then begins its data capture. */
static bool connect_output(void)
{
	feed_frames(10);
	atomic_store(&h.connected, true);
	return obs_output_begin_data_capture(h.output, 0);
}

static int group_setup(void **state)
{
	UNUSED_PARAMETER(state);
	assert_true(obs_startup("en-US", NULL, NULL));
	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 50,
		.fps_den = 1,
		.base_width = 64,
		.base_height = 64,
		.output_width = 64,
		.output_height = 64,
		.output_format = VIDEO_FORMAT_RGBA,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_FULL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	assert_int_equal(obs_reset_video(&ovi), OBS_VIDEO_SUCCESS);

	struct obs_encoder_info encoder = {
		.id = ENCODER_ID,
		.type = OBS_ENCODER_VIDEO,
		.codec = "h264",
		.get_name = test_name,
		.create = encoder_create,
		.destroy = test_destroy,
		.encode = encode,
	};
	obs_register_encoder(&encoder);
	struct obs_output_info output = {
		.id = OUTPUT_ID,
		.flags = OBS_OUTPUT_VIDEO | OBS_OUTPUT_ENCODED,
		.get_name = test_name,
		.create = output_create,
		.destroy = test_destroy,
		.start = output_start,
		.stop = output_stop,
		.encoded_packet = output_packet,
	};
	obs_register_output(&output);

	struct video_output_info vi = {
		.name = "driven video",
		.format = VIDEO_FORMAT_RGBA,
		.fps_num = 50,
		.fps_den = 1,
		.width = 64,
		.height = 64,
		.cache_size = 8,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_FULL,
	};
	assert_int_equal(video_output_open(&h.video, &vi), VIDEO_OUTPUT_SUCCESS);
	h.next_ts = 1000000000ULL;

	h.encoder = obs_video_encoder_create(ENCODER_ID, "video", NULL, NULL);
	assert_non_null(h.encoder);
	obs_encoder_set_video(h.encoder, h.video);
	return 0;
}

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	obs_encoder_release(h.encoder);
	video_output_close(h.video);
	obs_shutdown();
	return 0;
}

static int setup(void **state)
{
	UNUSED_PARAMETER(state);
	atomic_store(&h.connected, false);
	atomic_store(&h.start_calls, 0);
	atomic_store(&h.packets, 0);
	atomic_store(&h.packets_before_connect, 0);
	atomic_store(&h.start_signals, 0);

	h.output = obs_output_create(OUTPUT_ID, "output", NULL, NULL);
	assert_non_null(h.output);
	obs_output_set_video_encoder(h.output, h.encoder);
	obs_output_set_delay(h.output, 1, 0);
	/* Auto-reconnect off, so a lost connection stops the output. */
	obs_output_set_reconnect_settings(h.output, 0, 0);
	signal_handler_connect(obs_output_get_signal_handler(h.output), "start", on_start, NULL);
	return 0;
}

static int teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (obs_output_active(h.output))
		obs_output_force_stop(h.output);
	signal_handler_disconnect(obs_output_get_signal_handler(h.output), "start", on_start, NULL);
	obs_output_release(h.output);
	h.output = NULL;
	return 0;
}

/* Start the delayed output and let it connect: the first start works on
 * master too, and checks the harness. */
static void start_and_connect(long n)
{
	long packets = atomic_load(&h.packets);
	atomic_store(&h.packets_before_connect, 0);

	assert_true(obs_output_start(h.output));
	assert_true(feed_until_start_calls(n));
	bool began = connect_output();
	feed_frames(10);

	long before = atomic_load(&h.packets_before_connect);
	long after = atomic_load(&h.packets) - packets - before;
	print_message("start %ld: begin_data_capture %s, start signals %ld, packets before connect %ld, after %ld\n", n,
		      began ? "true" : "false", atomic_load(&h.start_signals), before, after);

	assert_true(began);
	assert_int_equal(atomic_load(&h.start_signals), n);
	assert_int_equal(before, 0);
	assert_true(after > 0);
}

static void stop_from_output_then_restart(int code)
{
	start_and_connect(1);

	/* The output stops itself, as rtmp_output does on a lost connection
	 * (auto-reconnect off) or an encoder error. */
	obs_output_signal_stop(h.output, code);
	assert_true(feed_until_inactive());
	print_message("after stop: delay_active %d, delay_capturing %d\n", h.output->delay_active,
		      h.output->delay_capturing);
	bool capturing = os_atomic_load_bool(&h.output->delay_capturing);
	assert_false(os_atomic_load_bool(&h.output->delay_active));

	/* What the user sees first: the next start. */
	start_and_connect(2);
	assert_false(capturing);

	obs_output_stop(h.output);
	assert_true(feed_until_inactive());
}

static void restart_after_disconnect_without_reconnect(void **state)
{
	UNUSED_PARAMETER(state);
	stop_from_output_then_restart(OBS_OUTPUT_DISCONNECTED);
}

static void restart_after_encode_error(void **state)
{
	UNUSED_PARAMETER(state);
	stop_from_output_then_restart(OBS_OUTPUT_ENCODE_ERROR);
}

/* Guard: a delayed stop (obs_output_stop) already left the output ready to
 * start again. */
static void restart_after_delayed_stop(void **state)
{
	UNUSED_PARAMETER(state);
	start_and_connect(1);

	obs_output_stop(h.output);
	assert_true(feed_until_inactive());
	assert_false(os_atomic_load_bool(&h.output->delay_active));
	assert_false(os_atomic_load_bool(&h.output->delay_capturing));

	start_and_connect(2);

	obs_output_stop(h.output);
	assert_true(feed_until_inactive());
}

int main(void)
{
	if (!getenv("DISPLAY"))
		return 77; /* Run with xvfb-run on headless Linux hosts. */
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(restart_after_disconnect_without_reconnect, setup, teardown),
		cmocka_unit_test_setup_teardown(restart_after_encode_error, setup, teardown),
		cmocka_unit_test_setup_teardown(restart_after_delayed_stop, setup, teardown),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
