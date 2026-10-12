/* Stopping a Custom FFmpeg Output (ffmpeg_output, obs-ffmpeg-output.c)
 * freed its encoders while the video or audio thread could still be
 * encoding a frame with them. libobs disconnects an output's raw video
 * and audio callbacks on a separate thread (end_data_capture_thread), so
 * when ffmpeg_output_stop() returns from obs_output_end_data_capture(),
 * receive_video() or receive_audio() may still be running, or about to
 * run, when ffmpeg_deactivate() frees the codec contexts. With libvpx this
 * corrupted the heap ("corrupted double-linked list", a crash in
 * vp8_remove_compressor()) or deadlocked the encoder's threads on stop
 * (obsproject/obs-studio#7536, reported by glaubersm).
 *
 * The test builds the real ffmpeg_output into the test and wraps
 * avcodec_send_frame() and avcodec_free_context() (-Wl,--wrap). It parks
 * the video (or audio) thread inside avcodec_send_frame() before the real
 * call, stops the output, and records whether the output freed the codec
 * context the parked thread is about to encode with. When it did, the
 * parked call returns AVERROR(EAGAIN) without touching the freed context,
 * so the check fails cleanly instead of crashing. The park ends after
 * PARK_MS, so a fixed output that waits for the callback only waits that
 * long. obs_startup() and core video need a display, so the cases skip
 * without one (run under xvfb-run). */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>

#include <obs.h>
#include <obs-module.h>
#include <util/base.h>
#include <util/platform.h>
#include <util/threading.h>

#include <libavcodec/avcodec.h>

#define W 128
#define H 128
#define PARK_MS 500

extern struct obs_output_info ffmpeg_output;

static bool obs_up;

const char *obs_module_text(const char *val)
{
	return val;
}

static void quiet_log(int lvl, const char *msg, va_list args, void *p)
{
	(void)p;
	if (lvl <= LOG_WARNING) {
		vfprintf(stderr, msg, args);
		fputc('\n', stderr);
	}
}

/* ------------------------------------------------------------------------- */
/* Encoder hooks */

static volatile long park_type = AVMEDIA_TYPE_UNKNOWN;
static volatile bool parked;
static volatile bool freed_while_parked;
static AVCodecContext *parked_ctx;
static os_event_t *parked_event;
static os_event_t *release_event;

int __real_avcodec_send_frame(AVCodecContext *ctx, const AVFrame *frame);
void __real_avcodec_free_context(AVCodecContext **pctx);

int __wrap_avcodec_send_frame(AVCodecContext *ctx, const AVFrame *frame)
{
	if (ctx && frame && os_atomic_load_long(&park_type) == (long)ctx->codec_type &&
	    os_atomic_compare_swap_long(&park_type, (long)ctx->codec_type, AVMEDIA_TYPE_UNKNOWN)) {
		parked_ctx = ctx;
		os_atomic_set_bool(&parked, true);
		os_event_signal(parked_event);
		os_event_timedwait(release_event, PARK_MS);

		const bool freed = os_atomic_load_bool(&freed_while_parked);
		os_atomic_set_bool(&parked, false);
		if (freed)
			return AVERROR(EAGAIN);
	}

	return __real_avcodec_send_frame(ctx, frame);
}

void __wrap_avcodec_free_context(AVCodecContext **pctx)
{
	if (pctx && *pctx && os_atomic_load_bool(&parked) && *pctx == parked_ctx)
		os_atomic_set_bool(&freed_while_parked, true);

	__real_avcodec_free_context(pctx);
}

/* ------------------------------------------------------------------------- */

static void on_start(void *param, calldata_t *cd)
{
	(void)cd;
	os_event_signal(param);
}

static bool have_encoder(const char *name)
{
	if (!avcodec_find_encoder_by_name(name)) {
		fprintf(stderr, "skipped: FFmpeg has no %s encoder\n", name);
		return false;
	}
	return true;
}

/* Records to a temporary file, parks the next video or audio encode call
 * and stops the output while that thread is parked. Returns whether the
 * output freed the parked encoder before the call returned. */
static bool stop_while_encoding(enum AVMediaType type, const char *format_name, const char *ext,
				const char *video_encoder, const char *audio_encoder, size_t mixers)
{
	char path[512];
	const char *tmp = getenv("TMPDIR") ? getenv("TMPDIR") : "/tmp";
	snprintf(path, sizeof(path), "%s/obs-ffmpeg-output-stop-%llu.%s", tmp, (unsigned long long)os_gettime_ns(),
		 ext);

	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, "url", path);
	obs_data_set_string(settings, "format_name", format_name);
	if (video_encoder)
		obs_data_set_string(settings, "video_encoder", video_encoder);
	if (audio_encoder)
		obs_data_set_string(settings, "audio_encoder", audio_encoder);
	obs_data_set_int(settings, "video_bitrate", 1000);
	obs_data_set_int(settings, "audio_bitrate", 128);
	obs_output_t *output = obs_output_create("ffmpeg_output", "stop test", settings, NULL);
	obs_data_release(settings);
	assert_non_null(output);
	obs_output_set_mixers(output, mixers);

	os_event_t *started;
	os_event_init(&started, OS_EVENT_TYPE_MANUAL);
	os_event_init(&parked_event, OS_EVENT_TYPE_MANUAL);
	os_event_init(&release_event, OS_EVENT_TYPE_MANUAL);
	os_atomic_set_bool(&freed_while_parked, false);
	signal_handler_t *sh = obs_output_get_signal_handler(output);
	signal_handler_connect(sh, "start", on_start, started);

	if (!obs_output_start(output) || os_event_timedwait(started, 10000) != 0) {
		const char *err = obs_output_get_last_error(output);
		fprintf(stderr, "the output did not start: %s\n", err ? err : "no error");
		fail();
	}

	/* Let a few frames through, then park the next one. */
	os_sleep_ms(300);
	os_atomic_set_long(&park_type, type);
	if (os_event_timedwait(parked_event, 5000) != 0) {
		fprintf(stderr, "no %s frame reached the encoder\n", av_get_media_type_string(type));
		fail();
	}

	obs_output_stop(output);
	const bool freed = os_atomic_load_bool(&freed_while_parked);
	os_event_signal(release_event);

	signal_handler_disconnect(sh, "start", on_start, started);
	obs_output_release(output);
	os_atomic_set_long(&park_type, AVMEDIA_TYPE_UNKNOWN);
	os_event_destroy(started);
	os_event_destroy(parked_event);
	os_event_destroy(release_event);
	parked_event = NULL;
	release_event = NULL;
	os_unlink(path);
	return freed;
}

static void stop_waits_for_video_encode(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	if (!have_encoder("ffv1"))
		skip();

	/* No mixers: video only, so no audio thread runs into the same race. */
	if (stop_while_encoding(AVMEDIA_TYPE_VIDEO, "matroska", "mkv", "ffv1", NULL, 0)) {
		fprintf(stderr, "stopping freed the video encoder while the video thread was encoding with it\n");
		fail();
	}
}

static void stop_waits_for_audio_encode(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	if (!have_encoder("flac"))
		skip();

	/* The FLAC container has no video stream, so only audio is encoded. */
	if (stop_while_encoding(AVMEDIA_TYPE_AUDIO, "flac", "flac", NULL, "flac", 1)) {
		fprintf(stderr, "stopping freed the audio encoder while the audio thread was encoding with it\n");
		fail();
	}
}

static int setup(void **state)
{
	(void)state;
	base_set_log_handler(quiet_log, NULL);

	const char *display = getenv("DISPLAY");
	if (!display || !*display) {
		fprintf(stderr, "skipped: no display\n");
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
		.base_width = W,
		.base_height = H,
		.output_width = W,
		.output_height = H,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	struct obs_audio_info oai = {.samples_per_sec = 48000, .speakers = SPEAKERS_STEREO};
	if (obs_reset_video(&ovi) != OBS_VIDEO_SUCCESS || !obs_reset_audio(&oai)) {
		fprintf(stderr, "skipped: core video/audio failed\n");
		obs_shutdown();
		return 0;
	}

	obs_register_output(&ffmpeg_output);
	obs_up = true;
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(stop_waits_for_video_encode),
		cmocka_unit_test(stop_waits_for_audio_encode),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
