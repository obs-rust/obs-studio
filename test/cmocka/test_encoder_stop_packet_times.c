/*
 * do_encode() appends to encoder_packet_times on the thread that delivers
 * frames, and takes no lock to do so. A stop on another thread must therefore
 * leave the array alone until the encoder is disconnected
 * (obs-rust/obs-studio#3788). obs_encoder_stop() used to set num to 0 before it
 * disconnected. Landing between the increment and the read in
 * da_push_back_new(), that made do_encode() write to element -1, which is the
 * ASan heap-buffer-overflow the issue reports. That window is a few
 * instructions, so these cases pin the state that opens it instead: the stopping
 * thread's write, seen from the encode callback on the delivering thread.
 *
 * The encoder emits no packets, so the array only grows while it runs, and num
 * at each encode is the number of frames since the start.
 *
 * Core video needs a graphics module and libobs needs an X display on Linux, so
 * the cases run under xvfb-run and skip without a display.
 */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <stdlib.h>

#include <obs.h>
#include <util/platform.h>
#include <util/threading.h>
#include "obs-internal.h"

#define WAIT_MS 20000
#define POLL_MS 2
/* How long a stop gets to write to the array after it erased its callback. Only
 * the failing case depends on it: the fixed code passes at any value. */
#define STOP_WRITE_MS 100

static volatile long frames;
static volatile long shrank;
static volatile long first_num;

/* The in-flight case: the encode callback holds a frame while a stop runs. */
static volatile bool hold_armed;
static volatile bool hold_entered;
static volatile bool stop_begun;
static volatile bool hold_done;
static volatile long num_before_stop;
static volatile long num_during_stop;

static const char *venc_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "packet times test encoder";
}

static void *venc_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	UNUSED_PARAMETER(settings);
	return encoder;
}

static void venc_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static bool callback_erased(obs_encoder_t *encoder)
{
	pthread_mutex_lock(&encoder->callbacks_mutex);
	bool erased = encoder->callbacks.num == 0;
	pthread_mutex_unlock(&encoder->callbacks_mutex);
	return erased;
}

/* Waits for the stop on the main thread, which itself waits for this frame to
 * finish before it can disconnect. Bounded, so that a stop that never comes
 * fails the case and does not hang it. */
static void hold_frame_during_stop(obs_encoder_t *encoder)
{
	os_atomic_set_long(&num_before_stop, (long)encoder->encoder_packet_times.num);
	os_atomic_set_bool(&hold_entered, true);

	for (int ms = 0; ms < WAIT_MS && !(os_atomic_load_bool(&stop_begun) && callback_erased(encoder)); ms += POLL_MS)
		os_sleep_ms(POLL_MS);
	os_sleep_ms(STOP_WRITE_MS);

	os_atomic_set_long(&num_during_stop, (long)encoder->encoder_packet_times.num);
	os_atomic_set_bool(&hold_done, true);
}

static bool venc_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received_packet)
{
	obs_encoder_t *encoder = data;
	UNUSED_PARAMETER(frame);
	UNUSED_PARAMETER(packet);

	long num = (long)encoder->encoder_packet_times.num;
	if (os_atomic_load_long(&first_num) < 0)
		os_atomic_set_long(&first_num, num);
	if (num != os_atomic_load_long(&frames))
		os_atomic_set_long(&shrank, 1);
	os_atomic_inc_long(&frames);

	if (os_atomic_load_bool(&hold_armed) && !os_atomic_load_bool(&hold_done))
		hold_frame_during_stop(encoder);

	*received_packet = false;
	return true;
}

static struct obs_encoder_info venc_info = {
	.id = "test_ept_venc",
	.type = OBS_ENCODER_VIDEO,
	.codec = "h264",
	.get_name = venc_name,
	.create = venc_create,
	.destroy = venc_destroy,
	.encode = venc_encode,
};

static const char *out_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "packet times test output";
}

static void *out_create(obs_data_t *settings, obs_output_t *output)
{
	UNUSED_PARAMETER(settings);
	return output;
}

static void out_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static bool out_start(void *data)
{
	obs_output_t *output = data;
	if (!obs_output_can_begin_data_capture(output, 0))
		return false;
	if (!obs_output_initialize_encoders(output, 0))
		return false;
	return obs_output_begin_data_capture(output, 0);
}

static void out_stop(void *data, uint64_t ts)
{
	UNUSED_PARAMETER(ts);
	obs_output_end_data_capture(data);
}

static void out_packet(void *data, struct encoder_packet *packet)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(packet);
}

static struct obs_output_info out_info = {
	.id = "test_ept_output",
	.flags = OBS_OUTPUT_VIDEO | OBS_OUTPUT_ENCODED,
	.encoded_video_codecs = "h264",
	.get_name = out_name,
	.create = out_create,
	.destroy = out_destroy,
	.start = out_start,
	.stop = out_stop,
	.encoded_packet = out_packet,
};

static void reset_counters(void)
{
	os_atomic_set_long(&frames, 0);
	os_atomic_set_long(&shrank, 0);
	os_atomic_set_long(&first_num, -1);
	os_atomic_set_bool(&hold_armed, false);
	os_atomic_set_bool(&hold_entered, false);
	os_atomic_set_bool(&stop_begun, false);
	os_atomic_set_bool(&hold_done, false);
}

static obs_encoder_t *create_encoder(void)
{
	reset_counters();
	obs_encoder_t *encoder = obs_video_encoder_create("test_ept_venc", "venc", NULL, NULL);
	obs_encoder_set_video(encoder, obs_get_video());
	return encoder;
}

static obs_output_t *start_output(obs_encoder_t *encoder, const char *name)
{
	obs_output_t *output = obs_output_create("test_ept_output", name, NULL, NULL);
	obs_output_set_video_encoder(output, encoder);
	if (!obs_output_start(output)) {
		fprintf(stderr, "the test output did not start: %s\n", obs_output_get_last_error(output));
		fail();
	}
	return output;
}

static void wait_frames(long count)
{
	for (int ms = 0; ms < WAIT_MS && os_atomic_load_long(&frames) < count; ms += POLL_MS)
		os_sleep_ms(POLL_MS);
	if (os_atomic_load_long(&frames) < count) {
		fprintf(stderr, "the encoder did not reach %ld frames\n", count);
		fail();
	}
}

static void stop_output(obs_output_t *output)
{
	obs_output_force_stop(output);
	for (int ms = 0; ms < WAIT_MS && obs_output_active(output); ms += POLL_MS)
		os_sleep_ms(POLL_MS);
	assert_false(obs_output_active(output));
}

/* The encoder keeps running for the other output, so the stop can never be
 * ordered against do_encode(), and the entries are not its to drop. */
static void stopping_one_of_two_outputs_keeps_the_array(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_initialized())
		skip();

	obs_encoder_t *encoder = create_encoder();
	obs_output_t *a = start_output(encoder, "a");
	obs_output_t *b = start_output(encoder, "b");
	wait_frames(3);

	stop_output(a);
	long after_stop = os_atomic_load_long(&frames);
	wait_frames(after_stop + 3);
	long changed = os_atomic_load_long(&shrank);

	stop_output(b);
	obs_output_release(a);
	obs_output_release(b);
	obs_encoder_release(encoder);

	if (changed) {
		fprintf(stderr, "stopping one output changed the running encoder's packet times\n");
		fail();
	}
}

/* The shape of the #3788 report: the last output stops while a frame is inside
 * do_encode(). The disconnect waits for that frame, so nothing may touch the
 * array before it. */
static void stopping_the_last_output_waits_for_the_frame(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_initialized())
		skip();

	obs_encoder_t *encoder = create_encoder();
	obs_output_t *output = start_output(encoder, "out");
	wait_frames(3);

	os_atomic_set_bool(&hold_armed, true);
	for (int ms = 0; ms < WAIT_MS && !os_atomic_load_bool(&hold_entered); ms += POLL_MS)
		os_sleep_ms(POLL_MS);
	assert_true(os_atomic_load_bool(&hold_entered));

	os_atomic_set_bool(&stop_begun, true);
	stop_output(output);
	assert_true(os_atomic_load_bool(&hold_done));

	long before = os_atomic_load_long(&num_before_stop);
	long during = os_atomic_load_long(&num_during_stop);
	obs_output_release(output);
	obs_encoder_release(encoder);

	if (before != during) {
		fprintf(stderr, "a stop changed the packet times from %ld to %ld during a frame\n", before, during);
		fail();
	}
}

/* Entries left by the previous run must not match the new run's PTS, which
 * starts at 0 again. */
static void restarting_starts_with_an_empty_array(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_initialized())
		skip();

	obs_encoder_t *encoder = create_encoder();
	obs_output_t *output = start_output(encoder, "out");
	wait_frames(3);
	stop_output(output);

	reset_counters();
	if (!obs_output_start(output)) {
		fprintf(stderr, "the test output did not restart: %s\n", obs_output_get_last_error(output));
		fail();
	}
	wait_frames(1);
	long first = os_atomic_load_long(&first_num);

	stop_output(output);
	obs_output_release(output);
	obs_encoder_release(encoder);

	if (first != 0) {
		fprintf(stderr, "the restarted encoder began with %ld stale packet times\n", first);
		fail();
	}
}

static int setup(void **state)
{
	UNUSED_PARAMETER(state);
	const char *display = getenv("DISPLAY");
	if (!display || !*display || !obs_startup("en-US", NULL, NULL)) {
		fprintf(stderr, "no display or no core video: the cases skip\n");
		return 0;
	}

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 60,
		.fps_den = 1,
		.base_width = 64,
		.base_height = 64,
		.output_width = 64,
		.output_height = 64,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	if (obs_reset_video(&ovi) != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "no core video: the cases skip\n");
		obs_shutdown();
		return 0;
	}

	obs_register_encoder(&venc_info);
	obs_register_output(&out_info);
	return 0;
}

static int teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (obs_initialized())
		obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(stopping_one_of_two_outputs_keeps_the_array),
		cmocka_unit_test(stopping_the_last_output_waits_for_the_frame),
		cmocka_unit_test(restarting_starts_with_an_empty_array),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
