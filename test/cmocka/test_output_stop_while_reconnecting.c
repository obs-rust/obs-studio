#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif

/*
 * An output that is stopped while it waits to reconnect must finish
 * stopping, and destroying it must return (#3739).
 *
 * A stream through the FFmpeg muxer (ffmpeg_mpegts_muxer, for example an SRT
 * server that does not answer, from OBS or from the source-record plugin)
 * fails to connect: the muxer ends its data capture and libobs waits to
 * reconnect. A stop in that wait goes to obs_output_actual_stop(), which
 * resets the stopping event and calls the output's stop. ffmpeg_mux_stop()
 * only sets its stopping flag: the stop ends with the next packet, and an
 * output whose data capture ended gets none. Nothing signals the stopping
 * event, and obs_output_destroy() waits on it forever: OBS does not exit.
 *
 * The cases stop an output while it waits to reconnect, with a stop that
 * only sets a flag (ffmpeg_muxer) and with a stop that ends the stop itself
 * when the output is not active (rtmp_output). Both must stop, signal "stop"
 * once and be destroyed. A control stops a connected output. Each case runs
 * in a child process with a watchdog, so a hang fails the case instead of
 * the test run. Core video needs a display, so the cases skip without one.
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

#define VENC_ID "test_reconnect_venc"
#define AENC_ID "test_reconnect_aenc"
#define OUTPUT_ID "test_reconnect_output"
#define WATCHDOG_MS 30000
/* long enough that the stop always comes before the reconnect */
#define RETRY_SEC 20

enum stop_kind {
	STOP_SETS_FLAG,     /* ffmpeg_mux_stop() */
	STOP_SIGNALS_ITSELF /* rtmp_stream_stop() on an output that is not active */
};

enum result {
	RESULT_OK = 0,
	RESULT_NO_STARTUP = 10,
	RESULT_NO_START = 11,
	RESULT_NO_PACKETS = 12,
	RESULT_NOT_RECONNECTING = 13,
	RESULT_STOP_NOT_FINISHED = 14,
	RESULT_STOP_SIGNALS = 15,
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
	case RESULT_NO_PACKETS:
		return "the output got no packet before the failure";
	case RESULT_NOT_RECONNECTING:
		return "the output did not wait to reconnect after the failure";
	case RESULT_STOP_NOT_FINISHED:
		return "the stop did not finish within 3 s";
	case RESULT_STOP_SIGNALS:
		return "the output did not signal \"stop\" exactly once";
	case RESULT_HUNG:
		return "hung (watchdog): obs_output_stop()/obs_output_release() did not return";
	default:
		return "unknown";
	}
}

static enum stop_kind stop_kind;
static uint8_t packet_byte;
static volatile long stop_signals;

/* ------------------------------------------------------------------------- */
/* encoders: a packet for every frame */

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

static bool enc_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received,
		       enum obs_encoder_type type)
{
	UNUSED_PARAMETER(data);
	packet->data = &packet_byte;
	packet->size = 1;
	packet->pts = frame->pts;
	packet->dts = frame->pts;
	packet->type = type;
	packet->keyframe = true;
	*received = true;
	return true;
}

static bool venc_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received)
{
	return enc_encode(data, frame, packet, received, OBS_ENCODER_VIDEO);
}

static bool aenc_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received)
{
	return enc_encode(data, frame, packet, received, OBS_ENCODER_AUDIO);
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
/* a stream output: the first packet at or after the stop time ends the data
 * capture (ffmpeg_muxer); a stop while not active either only sets the flag
 * (ffmpeg_muxer) or signals the stop (rtmp_output) */

struct stream {
	obs_output_t *output;
	volatile bool active;
	volatile bool stopping;
	volatile long long stop_usec;
	volatile long packets;
};

static struct stream *last_stream;

static const char *stream_name(void *type_data)
{
	UNUSED_PARAMETER(type_data);
	return "test stream";
}

static void *stream_create(obs_data_t *settings, obs_output_t *output)
{
	UNUSED_PARAMETER(settings);
	struct stream *s = bzalloc(sizeof(*s));
	s->output = output;
	last_stream = s;
	return s;
}

static void stream_destroy(void *data)
{
	if (last_stream == data)
		last_stream = NULL;
	bfree(data);
}

static bool stream_start(void *data)
{
	struct stream *s = data;
	if (!obs_output_can_begin_data_capture(s->output, 0))
		return false;
	if (!obs_output_initialize_encoders(s->output, 0))
		return false;
	os_atomic_set_bool(&s->stopping, false);
	os_atomic_set_bool(&s->active, true);
	return obs_output_begin_data_capture(s->output, 0);
}

static void stream_stop(void *data, uint64_t ts)
{
	struct stream *s = data;
	if (stop_kind == STOP_SIGNALS_ITSELF && !os_atomic_load_bool(&s->active)) {
		obs_output_signal_stop(s->output, OBS_OUTPUT_SUCCESS);
		return;
	}
	s->stop_usec = (long long)(ts / 1000);
	os_atomic_set_bool(&s->stopping, true);
}

static void stream_packet(void *data, struct encoder_packet *packet)
{
	struct stream *s = data;
	if (!packet || !os_atomic_load_bool(&s->active))
		return;
	os_atomic_inc_long(&s->packets);
	if (os_atomic_load_bool(&s->stopping) && packet->sys_dts_usec >= s->stop_usec) {
		os_atomic_set_bool(&s->active, false);
		obs_output_end_data_capture(s->output);
	}
}

static void register_output(void)
{
	struct obs_output_info info = {
		.id = OUTPUT_ID,
		.flags = OBS_OUTPUT_AV | OBS_OUTPUT_ENCODED,
		.get_name = stream_name,
		.create = stream_create,
		.destroy = stream_destroy,
		.start = stream_start,
		.stop = stream_stop,
		.encoded_packet = stream_packet,
	};
	obs_register_output(&info);
}

static void on_stop(void *data, calldata_t *cd)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(cd);
	os_atomic_inc_long(&stop_signals);
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

static bool wait_for(obs_output_t *output, bool (*cond)(const obs_output_t *), bool want, int timeout_ms)
{
	for (int i = 0; i < timeout_ms / 10; i++) {
		if (cond(output) == want)
			return true;
		os_sleep_ms(10);
	}
	return cond(output) == want;
}

static int run_case(bool reconnecting, enum stop_kind kind)
{
	if (!init_core())
		return RESULT_NO_STARTUP;
	register_encoders();
	register_output();
	stop_kind = kind;

	obs_encoder_t *venc = obs_video_encoder_create(VENC_ID, "video", NULL, NULL);
	obs_encoder_t *aenc = obs_audio_encoder_create(AENC_ID, "audio", NULL, 0, NULL);
	obs_output_t *output = obs_output_create(OUTPUT_ID, "output", NULL, NULL);
	obs_encoder_set_video(venc, obs_get_video());
	obs_encoder_set_audio(aenc, obs_get_audio());
	obs_output_set_video_encoder(output, venc);
	obs_output_set_audio_encoder(output, aenc, 0);
	obs_output_set_reconnect_settings(output, 5, RETRY_SEC);
	signal_handler_connect(obs_output_get_signal_handler(output), "stop", on_stop, NULL);

	if (!obs_output_start(output))
		return RESULT_NO_START;

	os_sleep_ms(1000);
	long packets = last_stream ? os_atomic_load_long(&last_stream->packets) : 0;
	if (packets == 0)
		return RESULT_NO_PACKETS;

	if (reconnecting) {
		/* the connection fails, as ffmpeg-mux reports for a server that
		 * does not answer: the output ends and libobs waits to reconnect */
		os_atomic_set_bool(&last_stream->active, false);
		obs_output_signal_stop(output, OBS_OUTPUT_DISCONNECTED);
		if (!wait_for(output, obs_output_reconnecting, true, 3000))
			return RESULT_NOT_RECONNECTING;
		/* the data capture has ended (its end runs on a thread) */
		os_sleep_ms(200);
	}

	obs_output_stop(output);

	/* a stop that never finishes would make the release below wait on the
	 * stopping event forever: report it instead */
	if (!wait_for(output, obs_output_active, false, 3000)) {
		fprintf(stderr, "%ld packets before the stop, still active 3 s after it\n", packets);
		return RESULT_STOP_NOT_FINISHED;
	}

	obs_output_release(output);
	obs_encoder_release(venc);
	obs_encoder_release(aenc);
	long signals = os_atomic_load_long(&stop_signals);
	fprintf(stderr, "%ld packets before the stop, stopped and destroyed, %ld \"stop\" signals\n", packets, signals);
	if (signals != 1)
		return RESULT_STOP_SIGNALS;
	obs_shutdown();
	return RESULT_OK;
}

static int run_in_child(bool reconnecting, enum stop_kind kind)
{
	fflush(stderr);
	pid_t pid = fork();
	if (pid < 0)
		return RESULT_HUNG;
	if (pid == 0)
		_exit(run_case(reconnecting, kind));

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

static void expect(bool reconnecting, enum stop_kind kind, const char *label)
{
	if (!have_display()) {
		fprintf(stderr, "skipped: no display\n");
		skip();
	}
	int r = run_in_child(reconnecting, kind);
	if (r == RESULT_NO_STARTUP) {
		fprintf(stderr, "skipped: %s\n", result_name(r));
		skip();
	}
	if (r != RESULT_OK) {
		fprintf(stderr, "%s: %s\n", label, result_name(r));
		fail();
	}
}

/* #3739: ffmpeg_muxer, stopped while it waits to reconnect */
static void flag_stop_while_reconnecting_finishes(void **state)
{
	UNUSED_PARAMETER(state);
	expect(true, STOP_SETS_FLAG, "stop sets a flag, while reconnecting");
}

/* rtmp_output ends the stop itself: it must still signal "stop" once */
static void self_signalling_stop_while_reconnecting_finishes(void **state)
{
	UNUSED_PARAMETER(state);
	expect(true, STOP_SIGNALS_ITSELF, "stop signals itself, while reconnecting");
}

/* control: a connected output stops with its next packet, as before */
static void stop_while_connected_finishes(void **state)
{
	UNUSED_PARAMETER(state);
	expect(false, STOP_SETS_FLAG, "stop while connected");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(flag_stop_while_reconnecting_finishes),
		cmocka_unit_test(self_signalling_stop_while_reconnecting_finishes),
		cmocka_unit_test(stop_while_connected_finishes),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
