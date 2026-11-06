#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif

/*
 * An output that was started but never received a packet must finish
 * stopping, and destroying it must return (#3739).
 *
 * The FFmpeg muxer output (ffmpeg_muxer, which the source-record plugin uses)
 * finishes a stop only when a packet at or after the stop time arrives:
 * ffmpeg_mux_stop() sets its stopping flag, and ffmpeg_mux_data() ends the
 * data capture. The interleaver hands an output no packet until both its
 * video and its audio encoder have produced one. When the audio never comes
 * (source-record's audio callback has no audio while its source is pending),
 * the output gets no packet at all, its stop never ends, and
 * obs_output_destroy() waits on the stopping event forever: OBS does not exit.
 *
 * The test registers a raw video encoder that produces a packet for every
 * frame, an audio encoder that produces packets or none, and an output that
 * stops like ffmpeg_muxer. Each case runs in a child process with a
 * watchdog, so a hang fails the case instead of the test run. Core video
 * needs a display, so the cases skip without one.
 */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

#include <obs.h>
#include <util/platform.h>
#include <util/threading.h>

#define VENC_ID "test_unstarted_venc"
#define AENC_ID "test_unstarted_aenc"
#define OUTPUT_ID "test_unstarted_output"
#define WATCHDOG_MS 30000

enum result {
	RESULT_OK = 0,
	RESULT_NO_STARTUP = 10,
	RESULT_NO_START = 11,
	RESULT_STOP_NOT_FINISHED = 12,
	RESULT_NO_PACKETS_IN_CONTROL = 13,
	RESULT_PACKETS_WITHOUT_AUDIO = 14,
	RESULT_HUNG = 99,
};

static const char *result_name(int r)
{
	switch (r) {
	case RESULT_OK:
		return "ok";
	case RESULT_NO_STARTUP:
		return "obs startup failed";
	case RESULT_NO_START:
		return "obs_output_start() failed";
	case RESULT_STOP_NOT_FINISHED:
		return "the stop did not finish within 3 s";
	case RESULT_NO_PACKETS_IN_CONTROL:
		return "control: the output got no packet with audio";
	case RESULT_PACKETS_WITHOUT_AUDIO:
		return "the output got packets although the audio encoder produced none";
	case RESULT_HUNG:
		return "hung (watchdog): obs_output_stop()/obs_output_release() did not return";
	default:
		return "unknown";
	}
}

static volatile bool audio_produces;
static uint8_t packet_byte;

/* ------------------------------------------------------------------------- */
/* encoders */

static const char *enc_name(void *type_data)
{
	UNUSED_PARAMETER(type_data);
	return "test encoder";
}

static void *enc_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	UNUSED_PARAMETER(settings);
	return encoder;
}

static void enc_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static bool venc_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received)
{
	UNUSED_PARAMETER(data);
	packet->data = &packet_byte;
	packet->size = 1;
	packet->pts = frame->pts;
	packet->dts = frame->pts;
	packet->type = OBS_ENCODER_VIDEO;
	packet->keyframe = true;
	*received = true;
	return true;
}

/* produces a packet per frame, or none, like an encoder whose audio never
 * arrives */
static bool aenc_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received)
{
	UNUSED_PARAMETER(data);
	if (!os_atomic_load_bool(&audio_produces)) {
		*received = false;
		return true;
	}
	packet->data = &packet_byte;
	packet->size = 1;
	packet->pts = frame->pts;
	packet->dts = frame->pts;
	packet->type = OBS_ENCODER_AUDIO;
	*received = true;
	return true;
}

static size_t aenc_frame_size(void *data)
{
	UNUSED_PARAMETER(data);
	return 1024;
}

static void register_encoders(void)
{
	struct obs_encoder_info venc = {
		.id = VENC_ID,
		.type = OBS_ENCODER_VIDEO,
		.codec = "h264",
		.get_name = enc_name,
		.create = enc_create,
		.destroy = enc_destroy,
		.encode = venc_encode,
	};
	obs_register_encoder(&venc);

	struct obs_encoder_info aenc = {
		.id = AENC_ID,
		.type = OBS_ENCODER_AUDIO,
		.codec = "aac",
		.get_name = enc_name,
		.create = enc_create,
		.destroy = enc_destroy,
		.encode = aenc_encode,
		.get_frame_size = aenc_frame_size,
	};
	obs_register_encoder(&aenc);
}

/* ------------------------------------------------------------------------- */
/* an output that stops like ffmpeg_muxer: the stop only sets a stop time, and
 * the first packet at or after it ends the data capture */

struct muxer {
	obs_output_t *output;
	volatile bool active;
	volatile bool stopping;
	volatile long long stop_usec;
	volatile long packets;
};

static struct muxer *last_muxer;

static const char *mux_name(void *type_data)
{
	UNUSED_PARAMETER(type_data);
	return "test muxer";
}

static void *mux_create(obs_data_t *settings, obs_output_t *output)
{
	UNUSED_PARAMETER(settings);
	struct muxer *mux = bzalloc(sizeof(*mux));
	mux->output = output;
	last_muxer = mux;
	return mux;
}

static void mux_destroy(void *data)
{
	if (last_muxer == data)
		last_muxer = NULL;
	bfree(data);
}

static bool mux_start(void *data)
{
	struct muxer *mux = data;
	if (!obs_output_can_begin_data_capture(mux->output, 0))
		return false;
	if (!obs_output_initialize_encoders(mux->output, 0))
		return false;
	os_atomic_set_bool(&mux->active, true);
	return obs_output_begin_data_capture(mux->output, 0);
}

static void mux_stop(void *data, uint64_t ts)
{
	struct muxer *mux = data;
	mux->stop_usec = (long long)(ts / 1000);
	os_atomic_set_bool(&mux->stopping, true);
}

static void mux_packet(void *data, struct encoder_packet *packet)
{
	struct muxer *mux = data;
	if (!packet || !os_atomic_load_bool(&mux->active))
		return;
	os_atomic_inc_long(&mux->packets);
	if (os_atomic_load_bool(&mux->stopping) && packet->sys_dts_usec >= mux->stop_usec) {
		os_atomic_set_bool(&mux->active, false);
		obs_output_end_data_capture(mux->output);
	}
}

static void register_output(void)
{
	struct obs_output_info info = {
		.id = OUTPUT_ID,
		.flags = OBS_OUTPUT_AV | OBS_OUTPUT_ENCODED,
		.get_name = mux_name,
		.create = mux_create,
		.destroy = mux_destroy,
		.start = mux_start,
		.stop = mux_stop,
		.encoded_packet = mux_packet,
	};
	obs_register_output(&info);
}

/* ------------------------------------------------------------------------- */
/* one case, in a child process */

static bool init_core(void)
{
	if (!obs_startup("en-US", NULL, NULL))
		return false;

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
		.scale_type = OBS_SCALE_BICUBIC,
	};
	if (obs_reset_video(&ovi) != OBS_VIDEO_SUCCESS)
		return false;

	struct obs_audio_info oai = {.samples_per_sec = 48000, .speakers = SPEAKERS_STEREO};
	return obs_reset_audio(&oai);
}

static bool wait_inactive(obs_output_t *output, int timeout_ms)
{
	for (int i = 0; i < timeout_ms / 10; i++) {
		if (!obs_output_active(output))
			return true;
		os_sleep_ms(10);
	}
	return !obs_output_active(output);
}

static int run_case(bool audio)
{
	if (!init_core())
		return RESULT_NO_STARTUP;
	register_encoders();
	register_output();
	os_atomic_set_bool(&audio_produces, audio);

	obs_encoder_t *venc = obs_video_encoder_create(VENC_ID, "video", NULL, NULL);
	obs_encoder_t *aenc = obs_audio_encoder_create(AENC_ID, "audio", NULL, 0, NULL);
	obs_output_t *output = obs_output_create(OUTPUT_ID, "output", NULL, NULL);
	obs_encoder_set_video(venc, obs_get_video());
	obs_encoder_set_audio(aenc, obs_get_audio());
	obs_output_set_video_encoder(output, venc);
	obs_output_set_audio_encoder(output, aenc, 0);

	if (!obs_output_start(output))
		return RESULT_NO_START;

	os_sleep_ms(1500);
	long packets = last_muxer ? os_atomic_load_long(&last_muxer->packets) : 0;
	obs_output_stop(output);

	/* a stop that never finishes would make the release below wait on the
	 * stopping event forever: report it instead */
	if (!wait_inactive(output, 3000)) {
		fprintf(stderr, "audio %s: %ld packets before the stop, still active 3 s after it\n",
			audio ? "on" : "off", packets);
		return RESULT_STOP_NOT_FINISHED;
	}

	obs_output_release(output);
	obs_encoder_release(venc);
	obs_encoder_release(aenc);
	fprintf(stderr, "audio %s: %ld packets before the stop, stopped and destroyed\n", audio ? "on" : "off",
		packets);

	if (audio && packets == 0)
		return RESULT_NO_PACKETS_IN_CONTROL;
	if (!audio && packets != 0)
		return RESULT_PACKETS_WITHOUT_AUDIO;
	obs_shutdown();
	return RESULT_OK;
}

static int run_in_child(bool audio)
{
	fflush(stderr);
	pid_t pid = fork();
	if (pid < 0)
		return RESULT_HUNG;
	if (pid == 0)
		_exit(run_case(audio));

	int status = 0;
	for (int waited = 0; waited < WATCHDOG_MS; waited += 50) {
		pid_t done = waitpid(pid, &status, WNOHANG);
		if (done == pid)
			return WIFEXITED(status) ? WEXITSTATUS(status) : RESULT_HUNG;
		os_sleep_ms(50);
	}
	kill(pid, SIGKILL);
	waitpid(pid, &status, 0);
	return RESULT_HUNG;
}

static bool have_display(void)
{
	const char *display = getenv("DISPLAY");
	return display && *display;
}

static void expect(bool audio)
{
	if (!have_display()) {
		fprintf(stderr, "skipped: no display\n");
		skip();
	}
	int r = run_in_child(audio);
	if (r == RESULT_NO_STARTUP) {
		fprintf(stderr, "skipped: %s\n", result_name(r));
		skip();
	}
	if (r != RESULT_OK) {
		fprintf(stderr, "audio %s: %s\n", audio ? "on" : "off", result_name(r));
		fail();
	}
}

/* #3739: no packet ever reached the output */
static void output_without_packets_stops_and_is_destroyed(void **state)
{
	UNUSED_PARAMETER(state);
	expect(false);
}

/* control: with audio the output gets packets and stops as before */
static void output_with_packets_stops_and_is_destroyed(void **state)
{
	UNUSED_PARAMETER(state);
	expect(true);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(output_without_packets_stops_and_is_destroyed),
		cmocka_unit_test(output_with_packets_stops_and_is_destroyed),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
