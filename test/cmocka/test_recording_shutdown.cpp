/* Quitting OBS while recording drops the end of the recording (forum thread
 * 182904, OBS 30.2.3 on Ubuntu: Ctrl+Q or SIGTERM while recording, "a few
 * seconds are missing at the end").
 *
 * OBSBasic::closeWindow() leaves an active recording running until
 * applicationShutdown() calls outputHandler.reset(). Destroying an active
 * output force-stops it, so every frame the video encoder still holds
 * (x264's lookahead and B-frames) never reaches the file. A Stop Recording
 * click stops the output gracefully instead: the output keeps receiving
 * packets until it has everything up to the moment of the stop.
 *
 * The test runs StopRecordingForShutdown() (frontend/widgets/
 * RecordingShutdown.hpp, the step closeWindow() runs before the outputs are
 * destroyed) on a real libobs output, then releases the output the way
 * outputHandler.reset() does. The video encoder holds back ENCODER_DELAY
 * frames like a lookahead encoder, and the output finishes a graceful stop
 * the way obs-ffmpeg-mux does: at the first packet at or past the stop
 * timestamp.
 *
 * Core video needs a graphics module and libobs needs an X display on Linux,
 * so the libobs cases skip without one (run under xvfb-run).
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
// cmocka.h 1.1 (Debian 13) and the Windows cmocka.h have no extern "C" guard.
extern "C" {
#include <cmocka.h>
}

#include <cstdio>
#include <deque>

#include <obs.h>
#include <util/platform.h>
#include <util/threading.h>

#include "RecordingShutdown.hpp"

#define VENC_ID "test_shutdown_video_encoder"
#define RECORDING_ID "test_shutdown_recording"
#define STUCK_RECORDING_ID "test_shutdown_stuck_recording"

/* frames the encoder holds before it emits a packet (x264 veryfast's
 * lookahead is 10 frames, plus B-frames) */
#define ENCODER_DELAY 12
#define SHUTDOWN_TIMEOUT_MS 5000

static bool obs_up = false;

/* ------------------------------------------------------------------------- */
/* a video encoder with a lookahead */

static pthread_mutex_t enc_mutex = PTHREAD_MUTEX_INITIALIZER;
static std::deque<int64_t> held_frames;
static volatile long frames_in = 0;
static uint8_t packet_byte = 0;

static const char *enc_name(void *)
{
	return "test lookahead encoder";
}

static void *enc_create(obs_data_t *, obs_encoder_t *encoder)
{
	pthread_mutex_lock(&enc_mutex);
	held_frames.clear();
	pthread_mutex_unlock(&enc_mutex);
	return encoder;
}

static void enc_destroy(void *) {}

static bool enc_encode(void *, struct encoder_frame *frame, struct encoder_packet *packet, bool *received)
{
	os_atomic_inc_long(&frames_in);

	pthread_mutex_lock(&enc_mutex);
	held_frames.push_back(frame->pts);
	*received = held_frames.size() > ENCODER_DELAY;
	if (*received) {
		int64_t pts = held_frames.front();
		held_frames.pop_front();
		packet->data = &packet_byte;
		packet->size = 1;
		packet->pts = pts;
		packet->dts = pts;
		packet->type = OBS_ENCODER_VIDEO;
		packet->keyframe = true;
	}
	pthread_mutex_unlock(&enc_mutex);
	return true;
}

/* ------------------------------------------------------------------------- */
/* a recording output that stops like obs-ffmpeg-mux */

struct recording {
	obs_output_t *output;
	bool stuck;
	volatile long stop_ts_usec; /* 0 = not stopping */
	volatile long packets;
	volatile long reached_stop;
};

static recording rec_state;

static const char *rec_name(void *)
{
	return "test recording";
}

static void *rec_create(obs_data_t *, obs_output_t *output)
{
	rec_state.output = output;
	os_atomic_set_long(&rec_state.stop_ts_usec, 0);
	os_atomic_set_long(&rec_state.packets, 0);
	os_atomic_set_long(&rec_state.reached_stop, 0);
	return &rec_state;
}

static void rec_destroy(void *) {}

static bool rec_start(void *data)
{
	recording *rec = static_cast<recording *>(data);
	if (!obs_output_can_begin_data_capture(rec->output, 0)) {
		return false;
	}
	if (!obs_output_initialize_encoders(rec->output, 0)) {
		return false;
	}
	return obs_output_begin_data_capture(rec->output, 0);
}

static void rec_stop(void *data, uint64_t ts)
{
	recording *rec = static_cast<recording *>(data);

	/* ts == 0: forced stop, end now */
	if (ts == 0) {
		obs_output_end_data_capture(rec->output);
		return;
	}

	/* graceful stop: end once the packets reach ts (never, if stuck) */
	os_atomic_set_long(&rec->stop_ts_usec, (long)(ts / 1000));
}

static void rec_packet(void *data, struct encoder_packet *packet)
{
	recording *rec = static_cast<recording *>(data);
	if (!packet) {
		return;
	}

	long stop_ts = os_atomic_load_long(&rec->stop_ts_usec);
	if (stop_ts && !rec->stuck && packet->sys_dts_usec >= stop_ts) {
		if (!os_atomic_set_long(&rec->reached_stop, 1)) {
			obs_output_end_data_capture(rec->output);
		}
		return;
	}
	if (!os_atomic_load_long(&rec->reached_stop)) {
		os_atomic_inc_long(&rec->packets);
	}
}

static void register_types(void)
{
	obs_encoder_info venc = {};
	venc.id = VENC_ID;
	venc.type = OBS_ENCODER_VIDEO;
	venc.codec = "h264";
	venc.get_name = enc_name;
	venc.create = enc_create;
	venc.destroy = enc_destroy;
	venc.encode = enc_encode;
	obs_register_encoder(&venc);

	obs_output_info rec = {};
	rec.id = RECORDING_ID;
	rec.flags = OBS_OUTPUT_VIDEO | OBS_OUTPUT_ENCODED;
	rec.get_name = rec_name;
	rec.create = rec_create;
	rec.destroy = rec_destroy;
	rec.start = rec_start;
	rec.stop = rec_stop;
	rec.encoded_packet = rec_packet;
	obs_register_output(&rec);
}

/* ------------------------------------------------------------------------- */
/* the frontend's view of the recording (SimpleOutput / AdvancedOutput) */

struct OutputHandler {
	obs_output_t *fileOutput;
	int gracefulStops = 0;
	int forcedStops = 0;

	bool RecordingActive() const { return obs_output_active(fileOutput); }
	void StopRecording(bool force)
	{
		if (force) {
			forcedStops++;
			obs_output_force_stop(fileOutput);
		} else {
			gracefulStops++;
			obs_output_stop(fileOutput);
		}
	}
};

struct FakeHandler {
	bool active = false;
	int stops = 0;

	bool RecordingActive() const { return active; }
	void StopRecording(bool) { stops++; }
};

static obs_encoder_t *venc = nullptr;

static obs_output_t *start_recording(bool stuck)
{
	rec_state.stuck = stuck;
	venc = obs_video_encoder_create(VENC_ID, "venc", nullptr, nullptr);
	obs_output_t *output = obs_output_create(RECORDING_ID, "recording", nullptr, nullptr);
	assert_non_null(venc);
	assert_non_null(output);
	obs_encoder_set_video(venc, obs_get_video());
	obs_output_set_video_encoder(output, venc);
	assert_true(obs_output_start(output));

	/* run until the encoder has passed its lookahead well */
	for (int i = 0; i < 500 && os_atomic_load_long(&rec_state.packets) < 30; i++) {
		os_sleep_ms(10);
	}
	assert_true(os_atomic_load_long(&rec_state.packets) >= 30);
	return output;
}

/* outputHandler.reset(): the outputs and encoders are released */
static void destroy_recording(obs_output_t *output)
{
	obs_output_release(output);
	obs_encoder_release(venc);
	venc = nullptr;
}

/* ------------------------------------------------------------------------- */

/* Every frame the encoder received before the quit reaches the recording. */
static void test_quit_keeps_frames_held_by_encoder(void **state)
{
	(void)state;
	if (!obs_up) {
		skip();
	}

	obs_output_t *output = start_recording(false);
	OutputHandler handler{output};

	long frames_before_quit = os_atomic_load_long(&frames_in);
	bool graceful = StopRecordingForShutdown(&handler, SHUTDOWN_TIMEOUT_MS);
	destroy_recording(output);

	long written = os_atomic_load_long(&rec_state.packets);
	printf("frames encoded before quit: %ld, packets written: %ld, graceful stops: %d\n", frames_before_quit,
	       written, handler.gracefulStops);

	assert_true(written >= frames_before_quit);
	assert_true(os_atomic_load_long(&rec_state.reached_stop));
	assert_true(graceful);
	assert_int_equal(handler.gracefulStops, 1);
	assert_int_equal(handler.forcedStops, 0);
}

/* A recording that never finishes stopping does not hang the quit: it is
 * force-stopped once the timeout passes. */
static void test_quit_does_not_hang_on_stuck_recording(void **state)
{
	(void)state;
	if (!obs_up) {
		skip();
	}

	obs_output_t *output = start_recording(true);
	OutputHandler handler{output};

	uint64_t start = os_gettime_ns();
	bool graceful = StopRecordingForShutdown(&handler, 300);
	uint64_t elapsed_ms = (os_gettime_ns() - start) / 1000000;
	/* a forced stop ends the capture on another thread */
	for (int i = 0; i < 100 && obs_output_active(output); i++) {
		os_sleep_ms(10);
	}
	bool still_active = obs_output_active(output);
	destroy_recording(output);

	printf("stuck recording: returned after %llu ms, graceful %d, forced stops %d\n",
	       (unsigned long long)elapsed_ms, graceful, handler.forcedStops);

	assert_false(still_active);
	assert_false(graceful);
	assert_int_equal(handler.gracefulStops, 1);
	assert_int_equal(handler.forcedStops, 1);
	assert_true(elapsed_ms >= 300);
	assert_true(elapsed_ms < 3000);
}

/* Nothing to do without an output handler or an active recording. */
static void test_quit_without_recording(void **state)
{
	(void)state;
	assert_true(StopRecordingForShutdown(static_cast<FakeHandler *>(nullptr), SHUTDOWN_TIMEOUT_MS));

	FakeHandler inactive;
	assert_true(StopRecordingForShutdown(&inactive, SHUTDOWN_TIMEOUT_MS));
	assert_int_equal(inactive.stops, 0);
}

static int setup(void **state)
{
	(void)state;
	if (!obs_startup("en-US", nullptr, nullptr)) {
		return 0;
	}

	obs_video_info ovi = {};
	ovi.graphics_module = GRAPHICS_MODULE_PATH;
	ovi.fps_num = 30;
	ovi.fps_den = 1;
	ovi.base_width = 64;
	ovi.base_height = 64;
	ovi.output_width = 64;
	ovi.output_height = 64;
	ovi.output_format = VIDEO_FORMAT_NV12;
	ovi.colorspace = VIDEO_CS_709;
	ovi.range = VIDEO_RANGE_PARTIAL;
	ovi.scale_type = OBS_SCALE_BICUBIC;
	int ret = obs_reset_video(&ovi);
	if (ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "obs_reset_video failed: %d\n", ret);
		obs_shutdown();
		return 0;
	}

	register_types();
	obs_up = true;
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (obs_up) {
		obs_shutdown();
	}
	obs_up = false;
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_quit_keeps_frames_held_by_encoder),
		cmocka_unit_test(test_quit_does_not_hang_on_stuck_recording),
		cmocka_unit_test(test_quit_without_recording),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
