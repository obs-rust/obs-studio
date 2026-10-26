/*
 * Audio monitoring went silent on Windows after a Media Source playing a
 * song with embedded cover art restarted (obsproject/obs-studio#9428 and
 * #11739; upstream fix obsproject/obs-studio#14091): after the scene was
 * shown again, or on the next loop, the song still played to the stream
 * but no longer to the monitoring device, until monitoring was switched off
 * and on again.
 *
 * The WASAPI monitor holds a source's audio back while it runs ahead of the
 * source's video, and stops doing so once the video timestamp
 * (source->last_frame_ts) has stood still for 100 ms: cover art is one
 * picture. On a restart the picture is decoded again with a new timestamp.
 * The monitor reset its count on the new timestamp but never stored it, so
 * the count stayed at 0 and the audio was held back for good.
 *
 * The WASAPI monitor itself only runs on Windows. This test plays generated
 * files with a cover (FLAC always, MP3 when an MP3 encoder is available)
 * through the real ffmpeg_source and real libobs, and runs the monitor's
 * decision (monitor-video-sync.h) in an audio capture callback, where the
 * monitor runs it, with the source's real video timestamp. It needs core
 * video, so it runs under xvfb-run and skips without a display.
 */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
#include <libavutil/channel_layout.h>
#include <libavutil/mem.h>

#include <obs-internal.h>
#include <obs-module.h>
#include <util/platform.h>
#include <util/threading.h>
#include <util/util_uint64.h>

#include "monitor-video-sync.h"

/* obs-ffmpeg-source.c is compiled into this test; it only needs strings. */
const char *obs_module_text(const char *val)
{
	return val;
}

extern struct obs_source_info ffmpeg_source;

#define W 64
#define H 64
#define RATE 48000

/* a 5 second 440 Hz tone */
#define CLIP_MS 5000
#define TONE_HZ 440.0
#define TONE_LEVEL 0.5f

/* how long playback is watched before and after the restart */
#define WATCH_MS 1000

static bool obs_up;
static bool have_mp3;

enum clip { MP3_COVER, FLAC_COVER, CLIP_COUNT };
static const char *const clip_names[CLIP_COUNT] = {"MP3 with cover art", "FLAC with cover art"};
static char clip_paths[CLIP_COUNT][512];

/* ------------------------------------------------------------------------ */
/* Test files */

static bool mux_packet(AVFormatContext *oc, AVPacket *pkt, AVStream *st, AVRational enc_tb)
{
	pkt->stream_index = st->index;
	av_packet_rescale_ts(pkt, enc_tb, st->time_base);
	return av_interleaved_write_frame(oc, pkt) >= 0;
}

/* One JPEG frame of a solid colour, as an album cover. */
static bool encode_cover(AVPacket *out, AVCodecParameters *par)
{
	const AVCodec *codec = avcodec_find_encoder(AV_CODEC_ID_MJPEG);
	if (!codec)
		return false;

	AVCodecContext *enc = avcodec_alloc_context3(codec);
	AVFrame *frame = av_frame_alloc();
	bool ok = false;
	if (!enc || !frame)
		goto done;

	enc->width = W;
	enc->height = H;
	enc->pix_fmt = AV_PIX_FMT_YUVJ420P;
	enc->color_range = AVCOL_RANGE_JPEG;
	enc->time_base = (AVRational){1, 25};
	if (avcodec_open2(enc, codec, NULL) < 0)
		goto done;

	frame->width = W;
	frame->height = H;
	frame->format = AV_PIX_FMT_YUVJ420P;
	if (av_frame_get_buffer(frame, 0) < 0)
		goto done;
	for (int y = 0; y < H; y++)
		memset(frame->data[0] + y * frame->linesize[0], 180, W);
	for (int y = 0; y < H / 2; y++) {
		memset(frame->data[1] + y * frame->linesize[1], 60, W / 2);
		memset(frame->data[2] + y * frame->linesize[2], 200, W / 2);
	}
	frame->pts = 0;

	if (avcodec_send_frame(enc, frame) < 0 || avcodec_send_frame(enc, NULL) < 0)
		goto done;
	if (avcodec_receive_packet(enc, out) < 0)
		goto done;
	ok = avcodec_parameters_from_context(par, enc) >= 0;

done:
	av_frame_free(&frame);
	avcodec_free_context(&enc);
	return ok;
}

static void fill_tone(AVFrame *frame, int64_t first)
{
	for (int i = 0; i < frame->nb_samples; i++) {
		float v = TONE_LEVEL * (float)sin(2.0 * M_PI * TONE_HZ * (double)(first + i) / RATE);
		switch (frame->format) {
		case AV_SAMPLE_FMT_FLT:
		case AV_SAMPLE_FMT_FLTP:
			((float *)frame->data[0])[i] = v;
			break;
		case AV_SAMPLE_FMT_S16:
		case AV_SAMPLE_FMT_S16P:
			((int16_t *)frame->data[0])[i] = (int16_t)(v * 32767.0f);
			break;
		case AV_SAMPLE_FMT_S32:
		case AV_SAMPLE_FMT_S32P:
			((int32_t *)frame->data[0])[i] = (int32_t)(v * 2147483647.0f);
			break;
		default:
			break;
		}
	}
}

static enum AVSampleFormat pick_sample_format(const AVCodecContext *enc, const AVCodec *codec)
{
	static const enum AVSampleFormat wanted[] = {AV_SAMPLE_FMT_FLTP, AV_SAMPLE_FMT_FLT,  AV_SAMPLE_FMT_S16P,
						     AV_SAMPLE_FMT_S16,  AV_SAMPLE_FMT_S32P, AV_SAMPLE_FMT_S32};
	const enum AVSampleFormat *formats = NULL;
	int count = 0;
	if (avcodec_get_supported_config(enc, codec, AV_CODEC_CONFIG_SAMPLE_FORMAT, 0, (const void **)&formats,
					 &count) < 0 ||
	    !formats)
		return AV_SAMPLE_FMT_NONE;

	for (size_t w = 0; w < sizeof(wanted) / sizeof(wanted[0]); w++)
		for (int i = 0; i < count; i++)
			if (formats[i] == wanted[w])
				return wanted[w];
	return AV_SAMPLE_FMT_NONE;
}

static bool drain_audio(AVFormatContext *oc, AVCodecContext *enc, AVStream *st, AVPacket *pkt)
{
	int ret;
	while ((ret = avcodec_receive_packet(enc, pkt)) >= 0) {
		if (!mux_packet(oc, pkt, st, enc->time_base))
			return false;
	}
	return ret == AVERROR(EAGAIN) || ret == AVERROR_EOF;
}

/* A mono tone, encoded with the given codec, with a JPEG cover as an
 * attached picture if asked. The muxers (ID3v2 APIC in MP3, a PICTURE block
 * in FLAC) need the cover before the first audio packet. */
static bool write_clip(const char *path, const char *format, enum AVCodecID codec_id, bool cover)
{
	const AVCodec *codec = avcodec_find_encoder(codec_id);
	if (!codec)
		return false;

	AVFormatContext *oc = NULL;
	AVCodecContext *enc = NULL;
	AVPacket *pkt = av_packet_alloc();
	AVFrame *frame = av_frame_alloc();
	bool ok = false;

	if (!pkt || !frame || avformat_alloc_output_context2(&oc, NULL, format, path) < 0)
		goto done;

	enc = avcodec_alloc_context3(codec);
	if (!enc)
		goto done;
	enc->sample_rate = RATE;
	enc->time_base = (AVRational){1, RATE};
	av_channel_layout_default(&enc->ch_layout, 1);
	enc->sample_fmt = pick_sample_format(enc, codec);
	if (enc->sample_fmt == AV_SAMPLE_FMT_NONE)
		goto done;
	if (codec_id == AV_CODEC_ID_MP3)
		enc->bit_rate = 128000;
	if (avcodec_open2(enc, codec, NULL) < 0)
		goto done;

	AVStream *as = avformat_new_stream(oc, NULL);
	if (!as || avcodec_parameters_from_context(as->codecpar, enc) < 0)
		goto done;
	as->time_base = enc->time_base;

	AVStream *vs = NULL;
	AVPacket *pic = NULL;
	if (cover) {
		vs = avformat_new_stream(oc, NULL);
		pic = av_packet_alloc();
		if (!vs || !pic || !encode_cover(pic, vs->codecpar)) {
			av_packet_free(&pic);
			goto done;
		}
		vs->disposition |= AV_DISPOSITION_ATTACHED_PIC;
		vs->time_base = (AVRational){1, 90000};
	}

	if (avio_open(&oc->pb, path, AVIO_FLAG_WRITE) < 0) {
		av_packet_free(&pic);
		goto done;
	}
	if (avformat_write_header(oc, NULL) < 0) {
		av_packet_free(&pic);
		goto done_io;
	}

	if (pic) {
		pic->stream_index = vs->index;
		pic->pts = pic->dts = 0;
		pic->flags |= AV_PKT_FLAG_KEY;
		bool written = av_write_frame(oc, pic) >= 0;
		av_packet_free(&pic);
		if (!written)
			goto done_io;
	}

	const int64_t total = (int64_t)RATE * CLIP_MS / 1000;
	const int chunk = enc->frame_size > 0 ? enc->frame_size : 4096;
	for (int64_t done_samples = 0; done_samples < total;) {
		av_frame_unref(frame);
		frame->nb_samples = (int)(total - done_samples < chunk ? total - done_samples : chunk);
		frame->format = enc->sample_fmt;
		frame->sample_rate = RATE;
		if (av_channel_layout_copy(&frame->ch_layout, &enc->ch_layout) < 0 || av_frame_get_buffer(frame, 0) < 0)
			goto done_io;
		fill_tone(frame, done_samples);
		frame->pts = done_samples;
		done_samples += frame->nb_samples;

		if (avcodec_send_frame(enc, frame) < 0 || !drain_audio(oc, enc, as, pkt))
			goto done_io;
	}
	if (avcodec_send_frame(enc, NULL) < 0 || !drain_audio(oc, enc, as, pkt))
		goto done_io;

	ok = av_write_trailer(oc) >= 0;

done_io:
	avio_closep(&oc->pb);
done:
	av_frame_free(&frame);
	av_packet_free(&pkt);
	avcodec_free_context(&enc);
	avformat_free_context(oc);
	return ok;
}

/* ------------------------------------------------------------------------ */

static int group_setup(void **state)
{
	UNUSED_PARAMETER(state);

	const char *display = getenv("DISPLAY");
	if (!display || !*display)
		return 0;

	if (!obs_startup("en-US", NULL, NULL))
		return 0;

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
	if (obs_reset_video(&ovi) != OBS_VIDEO_SUCCESS) {
		obs_shutdown();
		return 0;
	}

	struct obs_audio_info oai = {.samples_per_sec = RATE, .speakers = SPEAKERS_STEREO};
	if (!obs_reset_audio(&oai)) {
		obs_shutdown();
		return 0;
	}

	obs_register_source(&ffmpeg_source);

	const char *tmp = getenv("TMPDIR") ? getenv("TMPDIR") : "/tmp";
	unsigned long long id = (unsigned long long)os_gettime_ns();
	static const char *const exts[CLIP_COUNT] = {"mp3", "flac"};
	for (int c = 0; c < CLIP_COUNT; c++)
		snprintf(clip_paths[c], sizeof(clip_paths[c]), "%s/obs-cover-monitor-%llu-%d.%s", tmp, id, c, exts[c]);

	/* MP3 needs an external encoder (LAME); FLAC is built in. */
	have_mp3 = write_clip(clip_paths[MP3_COVER], "mp3", AV_CODEC_ID_MP3, true);
	if (!have_mp3)
		fprintf(stderr, "no MP3 encoder in this FFmpeg; the MP3 case is skipped\n");

	if (!write_clip(clip_paths[FLAC_COVER], "flac", AV_CODEC_ID_FLAC, true)) {
		fprintf(stderr, "could not write the FLAC test file\n");
		obs_shutdown();
		return -1;
	}

	obs_up = true;
	return 0;
}

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (obs_up) {
		obs_shutdown();
		for (int c = 0; c < CLIP_COUNT; c++)
			os_unlink(clip_paths[c]);
		obs_up = false;
	}
	return 0;
}

/* ------------------------------------------------------------------------ */
/* The monitor's video sync state, fed the way the WASAPI monitor feeds it:
 * from the source's audio capture callback, with source->last_frame_ts. */

struct monitor {
	pthread_mutex_t mutex;
	uint64_t prev_video_ts;
	uint64_t time_since_prev;
	uint64_t last_frame_ts;
	uint64_t packets;
};

static void on_audio(void *param, obs_source_t *source, const struct audio_data *audio, bool muted)
{
	UNUSED_PARAMETER(muted);
	struct monitor *m = param;
	uint64_t last_frame_ts = source->last_frame_ts;

	pthread_mutex_lock(&m->mutex);
	audio_monitor_track_video_ts(&m->prev_video_ts, &m->time_since_prev, last_frame_ts,
				     util_mul_div64(audio->frames, 1000000000ULL, RATE));
	m->last_frame_ts = last_frame_ts;
	m->packets++;
	pthread_mutex_unlock(&m->mutex);
}

static struct monitor snapshot(struct monitor *m)
{
	pthread_mutex_lock(&m->mutex);
	struct monitor s = *m;
	m->packets = 0;
	pthread_mutex_unlock(&m->mutex);
	return s;
}

static void cover_restart(enum clip c)
{
	if (c == MP3_COVER && !have_mp3)
		skip();

	obs_data_t *settings = obs_data_create();
	obs_data_set_bool(settings, "is_local_file", true);
	obs_data_set_string(settings, "local_file", clip_paths[c]);
	obs_data_set_bool(settings, "looping", false);
	obs_data_set_bool(settings, "restart_on_activate", true);
	obs_data_set_bool(settings, "close_when_inactive", false);
	obs_data_set_bool(settings, "hw_decode", false);
	obs_source_t *media = obs_source_create("ffmpeg_source", "cover art media", settings, NULL);
	obs_data_release(settings);
	if (!media) {
		fprintf(stderr, "could not create the media source\n");
		fail();
	}

	obs_scene_t *scene = obs_scene_create("music");
	obs_scene_t *other = obs_scene_create("elsewhere");
	obs_scene_add(scene, media);

	struct monitor m = {0};
	pthread_mutex_init(&m.mutex, NULL);
	obs_source_add_audio_capture_callback(media, on_audio, &m);
	obs_set_output_source(0, obs_scene_get_source(scene));

	os_sleep_ms(WATCH_MS);
	struct monitor first = snapshot(&m);

	/* the program goes to another scene and back: restart on activate */
	obs_set_output_source(0, obs_scene_get_source(other));
	os_sleep_ms(500);
	snapshot(&m);
	obs_set_output_source(0, obs_scene_get_source(scene));
	os_sleep_ms(WATCH_MS);
	struct monitor again = snapshot(&m);

	obs_source_remove_audio_capture_callback(media, on_audio, &m);
	obs_set_output_source(0, NULL);
	obs_source_remove(obs_scene_get_source(scene));
	obs_source_remove(obs_scene_get_source(other));
	obs_scene_release(scene);
	obs_scene_release(other);
	obs_source_release(media);
	pthread_mutex_destroy(&m.mutex);

	fprintf(stderr,
		"%s: first play: %llu packets, cover at %llu, still for %llu ms; shown again: %llu packets, cover "
		"at %llu, monitor has %llu, still for %llu ms\n",
		clip_names[c], (unsigned long long)first.packets, (unsigned long long)first.last_frame_ts,
		(unsigned long long)(first.time_since_prev / 1000000), (unsigned long long)again.packets,
		(unsigned long long)again.last_frame_ts, (unsigned long long)again.prev_video_ts,
		(unsigned long long)(again.time_since_prev / 1000000));

	if (!first.packets || !again.packets || !first.last_frame_ts || !again.last_frame_ts) {
		fprintf(stderr, "%s: no audio or no cover frame reached the source\n", clip_names[c]);
		fail();
	}
	if (first.time_since_prev <= AUDIO_MONITOR_VIDEO_STALL_NS) {
		fprintf(stderr, "%s: first play: the still cover was not seen as still\n", clip_names[c]);
		fail();
	}
	if (again.last_frame_ts == first.last_frame_ts) {
		fprintf(stderr, "%s: the restart did not show the cover again with a new timestamp\n", clip_names[c]);
		fail();
	}
	if (again.time_since_prev <= AUDIO_MONITOR_VIDEO_STALL_NS) {
		fprintf(stderr,
			"%s: after the restart the still cover was not seen as still: the monitor would hold the "
			"audio back for good\n",
			clip_names[c]);
		fail();
	}
}

/* The report: an MP3 with an album cover. */
static void mp3_cover_restart_monitored(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	cover_restart(MP3_COVER);
}

/* The same without the MP3 encoder: any attached picture does it. */
static void flac_cover_restart_monitored(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	cover_restart(FLAC_COVER);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(mp3_cover_restart_monitored),
		cmocka_unit_test(flac_cover_restart_monitored),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
