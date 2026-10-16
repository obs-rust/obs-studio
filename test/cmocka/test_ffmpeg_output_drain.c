/* Stopping a Custom FFmpeg Output (ffmpeg_output, obs-ffmpeg-output.c)
 * loses the end of the recording. Encoders hold frames back (B-frames,
 * lookahead: libx264, libvpx, SVT-AV1, and the AAC encoder's one frame of
 * delay), and FFmpeg hands them out only after the end of the input is
 * signalled with avcodec_send_frame(ctx, NULL). ffmpeg_deactivate() never
 * sends it, so those frames are freed with the encoder; it also frees the
 * packets the write thread had not written yet. With libx264 or libvpx-vp9
 * the last ~0.8 s of video is missing, with SVT-AV1 more than a second.
 *
 * The test builds the real ffmpeg_output into the test and wraps
 * avcodec_send_frame() (-Wl,--wrap) to count the frames the output gives
 * its encoder. It records for a moment and stops the output. Every frame
 * the encoder got before obs_output_stop() was called was rendered before
 * the stop time, so it must be in the file: the test counts the packets of
 * the finished file and compares. MPEG-4 part 2 with 3 B-frames and the
 * AAC encoder are built into FFmpeg; libx264 is used where FFmpeg has it.
 * obs_startup() and core video need a display, so the cases skip without
 * one (run under xvfb-run). */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <stdlib.h>

#include <obs.h>
#include <obs-module.h>
#include <util/base.h>
#include <util/platform.h>
#include <util/threading.h>

#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>

#define W 128
#define H 128
#define RECORD_MS 1500

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
/* Encoder hook: frames given to the encoders */

static volatile long video_frames_in;
static volatile long audio_frames_in;

int __real_avcodec_send_frame(AVCodecContext *ctx, const AVFrame *frame);

int __wrap_avcodec_send_frame(AVCodecContext *ctx, const AVFrame *frame)
{
	int ret = __real_avcodec_send_frame(ctx, frame);
	if (ctx && frame && ret == 0) {
		if (ctx->codec_type == AVMEDIA_TYPE_VIDEO)
			os_atomic_inc_long(&video_frames_in);
		else if (ctx->codec_type == AVMEDIA_TYPE_AUDIO)
			os_atomic_inc_long(&audio_frames_in);
	}
	return ret;
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

/* Packets of the given type in the finished file, or -1 if it can't be
 * read. */
static long count_packets(const char *path, enum AVMediaType type)
{
	AVFormatContext *ic = NULL;
	if (avformat_open_input(&ic, path, NULL, NULL) < 0)
		return -1;
	if (avformat_find_stream_info(ic, NULL) < 0) {
		avformat_close_input(&ic);
		return -1;
	}

	int si = av_find_best_stream(ic, type, -1, -1, NULL, 0);
	AVPacket *pkt = av_packet_alloc();
	long n = 0;
	while (si >= 0 && pkt && av_read_frame(ic, pkt) >= 0) {
		if (pkt->stream_index == si)
			n++;
		av_packet_unref(pkt);
	}
	av_packet_free(&pkt);
	avformat_close_input(&ic);
	return si >= 0 ? n : -1;
}

struct take {
	long sent_before_stop; /* frames the encoder got before the stop call */
	long sent_total;
	long in_file;
};

/* Records to a temporary file for RECORD_MS and stops the output. */
static struct take record(enum AVMediaType type, const char *format_name, const char *ext, const char *video_encoder,
			  const char *video_settings, const char *audio_encoder, size_t mixers)
{
	struct take t = {0};
	char path[512];
	const char *tmp = getenv("TMPDIR") ? getenv("TMPDIR") : "/tmp";
	snprintf(path, sizeof(path), "%s/obs-ffmpeg-output-drain-%llu.%s", tmp, (unsigned long long)os_gettime_ns(),
		 ext);

	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, "url", path);
	obs_data_set_string(settings, "format_name", format_name);
	if (video_encoder)
		obs_data_set_string(settings, "video_encoder", video_encoder);
	if (video_settings)
		obs_data_set_string(settings, "video_settings", video_settings);
	if (audio_encoder)
		obs_data_set_string(settings, "audio_encoder", audio_encoder);
	obs_data_set_int(settings, "video_bitrate", 1000);
	obs_data_set_int(settings, "audio_bitrate", 128);
	obs_output_t *output = obs_output_create("ffmpeg_output", "drain test", settings, NULL);
	obs_data_release(settings);
	assert_non_null(output);
	obs_output_set_mixers(output, mixers);

	os_event_t *started;
	os_event_init(&started, OS_EVENT_TYPE_MANUAL);
	signal_handler_t *sh = obs_output_get_signal_handler(output);
	signal_handler_connect(sh, "start", on_start, started);

	os_atomic_set_long(&video_frames_in, 0);
	os_atomic_set_long(&audio_frames_in, 0);
	volatile long *counter = type == AVMEDIA_TYPE_VIDEO ? &video_frames_in : &audio_frames_in;

	if (!obs_output_start(output) || os_event_timedwait(started, 10000) != 0) {
		const char *err = obs_output_get_last_error(output);
		fprintf(stderr, "the output did not start: %s\n", err ? err : "no error");
		fail();
	}

	os_sleep_ms(RECORD_MS);
	t.sent_before_stop = os_atomic_load_long(counter);
	obs_output_stop(output);

	signal_handler_disconnect(sh, "start", on_start, started);
	/* the file is finished when the output is destroyed at the latest */
	obs_output_release(output);
	os_event_destroy(started);
	t.sent_total = os_atomic_load_long(counter);

	t.in_file = count_packets(path, type);
	os_unlink(path);

	fprintf(stderr, "%s %s: %ld frames to the encoder before the stop (%ld in all), %ld packets in the file\n",
		format_name, av_get_media_type_string(type), t.sent_before_stop, t.sent_total, t.in_file);
	return t;
}

static void expect_all_frames(const char *what, struct take t)
{
	if (t.sent_before_stop <= 0) {
		fprintf(stderr, "%s: no frame reached the encoder\n", what);
		fail();
	}
	if (t.in_file < t.sent_before_stop) {
		fprintf(stderr, "%s: the file has %ld packets, %ld frames were encoded before the stop (%ld lost)\n",
			what, t.in_file, t.sent_before_stop, t.sent_before_stop - t.in_file);
		fail();
	}
}

/* MPEG-4 with B-frames holds 3 frames back. */
static void stop_writes_frames_held_by_video_encoder(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	if (!have_encoder("mpeg4"))
		skip();

	/* No mixers: video only. */
	struct take t = record(AVMEDIA_TYPE_VIDEO, "matroska", "mkv", "mpeg4", "bf=3", NULL, 0);
	expect_all_frames("mpeg4 with B-frames", t);
}

/* libx264's lookahead holds most of a second back (as in the reports). */
static void stop_writes_frames_held_by_x264_lookahead(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	if (!have_encoder("libx264"))
		skip();

	struct take t = record(AVMEDIA_TYPE_VIDEO, "matroska", "mkv", "libx264", "preset=veryfast", NULL, 0);
	expect_all_frames("libx264", t);
}

/* The AAC encoder holds one frame back. ADTS has no video stream. */
static void stop_writes_frames_held_by_audio_encoder(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	if (!have_encoder("aac"))
		skip();

	struct take t = record(AVMEDIA_TYPE_AUDIO, "adts", "aac", NULL, NULL, "aac", 1);
	expect_all_frames("aac", t);
}

/* Guard: an encoder without delay (FFV1, intra only) lost nothing before
 * either; frames after the stop time are still left out. */
static void stop_without_encoder_delay_keeps_every_frame(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	if (!have_encoder("ffv1"))
		skip();

	struct take t = record(AVMEDIA_TYPE_VIDEO, "matroska", "mkv", "ffv1", NULL, NULL, 0);
	expect_all_frames("ffv1", t);
	if (t.in_file > t.sent_total) {
		fprintf(stderr, "ffv1: the file has more packets (%ld) than frames encoded (%ld)\n", t.in_file,
			t.sent_total);
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
		cmocka_unit_test(stop_writes_frames_held_by_video_encoder),
		cmocka_unit_test(stop_writes_frames_held_by_x264_lookahead),
		cmocka_unit_test(stop_writes_frames_held_by_audio_encoder),
		cmocka_unit_test(stop_without_encoder_delay_keeps_every_frame),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
