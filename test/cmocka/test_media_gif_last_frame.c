/*
 * A Media Source playing a one-shot (non-looping) GIF must show every frame
 * (upstream obsproject/obs-studio#10425): with "Loop" off and "Show nothing
 * when playback ends" on, the GIF disappeared before its last frame was
 * shown.
 *
 * The cases generate a short clip with libavformat/libavcodec, ten frames of
 * distinct solid colours at 100 ms each, play it through the real
 * ffmpeg_source and record which colour every program output frame shows.
 * Every frame must appear, in order, and the last one must not only flash.
 * A looping GIF and the same frames as a
 * video file are checked the same way, and the video once more right after a
 * run that ended on its last frame. They need core video, so they run
 * under xvfb-run and skip without a display.
 */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
#include <libavutil/opt.h>

#include <obs.h>
#include <obs-module.h>
#include <util/platform.h>
#include <util/threading.h>

/* obs-ffmpeg-source.c is compiled into this test; it only needs strings. */
const char *obs_module_text(const char *val)
{
	return val;
}

extern struct obs_source_info ffmpeg_source;

#define W 64
#define H 64
#define FRAMES 10
/* frame delay of the clip, in centiseconds (the GIF time base) */
#define DELAY_CS 10

/* All of them are exact in the GIF encoder's fixed RGB8 palette (red and
 * green in steps of 36, blue in steps of 85). */
static const uint8_t colours[FRAMES][3] = {
	{252, 0, 0},   {0, 252, 0},     {0, 0, 255},   {252, 252, 0}, {0, 252, 255},
	{252, 0, 255}, {252, 252, 255}, {252, 108, 0}, {108, 0, 170}, {108, 108, 85},
};

#define NOTHING (-1)
#define UNKNOWN (-2)

static bool obs_up;
static char gif_once_path[512];
static char gif_loop_path[512];
static char video_path[512];

static void fill_rgb(uint8_t *data, int linesize, const uint8_t rgb[3], bool bgra)
{
	for (int y = 0; y < H; y++) {
		uint8_t *p = data + (size_t)y * linesize;
		for (int x = 0; x < W; x++) {
			if (bgra) {
				p[x * 4 + 0] = rgb[2];
				p[x * 4 + 1] = rgb[1];
				p[x * 4 + 2] = rgb[0];
				p[x * 4 + 3] = 255;
			} else {
				/* RGB8: rrrgggbb */
				p[x] = (uint8_t)(((rgb[0] / 36) << 5) | ((rgb[1] / 36) << 2) | (rgb[2] / 85));
			}
		}
	}
}

/* Encodes the frames as a GIF that plays once (loop = -1) or loops forever
 * (loop = 0). */
static bool write_gif(const char *path, int loop)
{
	const AVCodec *codec = avcodec_find_encoder(AV_CODEC_ID_GIF);
	if (!codec)
		return false;

	AVFormatContext *oc = NULL;
	if (avformat_alloc_output_context2(&oc, NULL, "gif", path) < 0)
		return false;

	bool ok = false;
	AVCodecContext *enc = avcodec_alloc_context3(codec);
	AVFrame *frame = av_frame_alloc();
	AVPacket *pkt = av_packet_alloc();
	AVStream *st = avformat_new_stream(oc, NULL);
	if (!enc || !frame || !pkt || !st)
		goto out;

	enc->width = W;
	enc->height = H;
	enc->pix_fmt = AV_PIX_FMT_RGB8;
	enc->time_base = (AVRational){1, 100};
	if (avcodec_open2(enc, codec, NULL) < 0)
		goto out;
	if (avcodec_parameters_from_context(st->codecpar, enc) < 0)
		goto out;
	st->time_base = enc->time_base;

	frame->format = enc->pix_fmt;
	frame->width = W;
	frame->height = H;
	if (av_frame_get_buffer(frame, 0) < 0)
		goto out;

	if (avio_open(&oc->pb, path, AVIO_FLAG_WRITE) < 0)
		goto out;
	AVDictionary *opts = NULL;
	av_dict_set_int(&opts, "loop", loop, 0);
	int ret = avformat_write_header(oc, &opts);
	av_dict_free(&opts);
	if (ret < 0)
		goto out_io;

	for (int i = 0; i <= FRAMES; i++) {
		if (i < FRAMES) {
			if (av_frame_make_writable(frame) < 0)
				goto out_io;
			fill_rgb(frame->data[0], frame->linesize[0], colours[i], false);
			frame->pts = (int64_t)i * DELAY_CS;
			ret = avcodec_send_frame(enc, frame);
		} else {
			ret = avcodec_send_frame(enc, NULL);
		}
		if (ret < 0)
			goto out_io;

		while ((ret = avcodec_receive_packet(enc, pkt)) == 0) {
			pkt->duration = DELAY_CS;
			av_packet_rescale_ts(pkt, enc->time_base, st->time_base);
			pkt->stream_index = st->index;
			if (av_interleaved_write_frame(oc, pkt) < 0)
				goto out_io;
		}
		if (ret != AVERROR(EAGAIN) && ret != AVERROR_EOF)
			goto out_io;
	}

	ok = av_write_trailer(oc) >= 0;
out_io:
	avio_closep(&oc->pb);
out:
	av_packet_free(&pkt);
	av_frame_free(&frame);
	avcodec_free_context(&enc);
	avformat_free_context(oc);
	return ok;
}

/* The same frames as raw BGRA video in NUT, written with libavformat so no
 * encoder is needed. */
static bool write_video(const char *path)
{
	AVFormatContext *oc = NULL;
	if (avformat_alloc_output_context2(&oc, NULL, "nut", path) < 0)
		return false;

	AVStream *vs = avformat_new_stream(oc, NULL);
	if (!vs)
		goto fail;

	vs->time_base = (AVRational){DELAY_CS, 100};
	vs->codecpar->codec_type = AVMEDIA_TYPE_VIDEO;
	vs->codecpar->codec_id = AV_CODEC_ID_RAWVIDEO;
	vs->codecpar->format = AV_PIX_FMT_BGRA;
	/* NUT stores the raw pixel format as a tag */
	vs->codecpar->codec_tag = avcodec_pix_fmt_to_codec_tag(AV_PIX_FMT_BGRA);
	vs->codecpar->width = W;
	vs->codecpar->height = H;

	if (avio_open(&oc->pb, path, AVIO_FLAG_WRITE) < 0)
		goto fail;
	if (avformat_write_header(oc, NULL) < 0)
		goto fail_io;

	static uint8_t pixels[W * H * 4];
	for (int i = 0; i < FRAMES; i++) {
		fill_rgb(pixels, W * 4, colours[i], true);

		AVPacket *pkt = av_packet_alloc();
		if (!pkt || av_new_packet(pkt, (int)sizeof(pixels)) < 0) {
			av_packet_free(&pkt);
			goto fail_io;
		}
		memcpy(pkt->data, pixels, sizeof(pixels));
		pkt->stream_index = vs->index;
		pkt->pts = pkt->dts = av_rescale_q(i, (AVRational){DELAY_CS, 100}, vs->time_base);
		pkt->duration = av_rescale_q(1, (AVRational){DELAY_CS, 100}, vs->time_base);
		pkt->flags |= AV_PKT_FLAG_KEY;
		bool written = av_interleaved_write_frame(oc, pkt) >= 0;
		av_packet_free(&pkt);
		if (!written)
			goto fail_io;
	}

	av_write_trailer(oc);
	avio_closep(&oc->pb);
	avformat_free_context(oc);
	return true;

fail_io:
	avio_closep(&oc->pb);
fail:
	avformat_free_context(oc);
	return false;
}

static void unlink_clips(void)
{
	if (*gif_once_path)
		os_unlink(gif_once_path);
	if (*gif_loop_path)
		os_unlink(gif_loop_path);
	if (*video_path)
		os_unlink(video_path);
}

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
		.output_format = VIDEO_FORMAT_BGRA,
		.colorspace = VIDEO_CS_SRGB,
		.range = VIDEO_RANGE_FULL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	if (obs_reset_video(&ovi) != OBS_VIDEO_SUCCESS) {
		obs_shutdown();
		return 0;
	}

	struct obs_audio_info oai = {.samples_per_sec = 48000, .speakers = SPEAKERS_STEREO};
	if (!obs_reset_audio(&oai)) {
		obs_shutdown();
		return 0;
	}

	obs_register_source(&ffmpeg_source);

	const char *tmp = getenv("TMPDIR") ? getenv("TMPDIR") : "/tmp";
	unsigned long long id = (unsigned long long)os_gettime_ns();
	snprintf(gif_once_path, sizeof(gif_once_path), "%s/obs-media-gif-once-%llu.gif", tmp, id);
	snprintf(gif_loop_path, sizeof(gif_loop_path), "%s/obs-media-gif-loop-%llu.gif", tmp, id);
	snprintf(video_path, sizeof(video_path), "%s/obs-media-gif-video-%llu.nut", tmp, id);
	if (!write_gif(gif_once_path, -1) || !write_gif(gif_loop_path, 0) || !write_video(video_path)) {
		fprintf(stderr, "could not write the test clips\n");
		unlink_clips();
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
		unlink_clips();
		obs_up = false;
	}
	return 0;
}

#define MAX_SEEN 1024

struct watch {
	pthread_mutex_t mutex;
	size_t count;
	int seen[MAX_SEEN];
};

static int classify(const uint8_t *bgra)
{
	if (bgra[0] < 30 && bgra[1] < 30 && bgra[2] < 30)
		return NOTHING;

	for (int i = 0; i < FRAMES; i++) {
		if (abs(bgra[2] - colours[i][0]) < 30 && abs(bgra[1] - colours[i][1]) < 30 &&
		    abs(bgra[0] - colours[i][2]) < 30)
			return i;
	}
	return UNKNOWN;
}

static void raw_video(void *param, struct video_data *frame)
{
	struct watch *w = param;
	int s = classify(frame->data[0] + (H / 2) * frame->linesize[0] + (W / 2) * 4);

	pthread_mutex_lock(&w->mutex);
	if (w->count < MAX_SEEN)
		w->seen[w->count++] = s;
	pthread_mutex_unlock(&w->mutex);
}

/* Plays the clip in a scene for play_ms (or until it ends, and then a bit
 * more), recording every output frame. clear_on_end is "Show nothing when
 * playback ends". */
static void play(const char *path, bool looping, bool clear_on_end, uint32_t play_ms, struct watch *w)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_bool(settings, "is_local_file", true);
	obs_data_set_string(settings, "local_file", path);
	obs_data_set_bool(settings, "looping", looping);
	obs_data_set_bool(settings, "restart_on_activate", true);
	obs_data_set_bool(settings, "clear_on_media_end", clear_on_end);
	obs_data_set_bool(settings, "close_when_inactive", false);
	obs_data_set_bool(settings, "hw_decode", false);
	obs_source_t *media = obs_source_create("ffmpeg_source", "media", settings, NULL);
	obs_data_release(settings);
	if (!media) {
		fprintf(stderr, "could not create the media source\n");
		fail();
	}

	obs_scene_t *scene = obs_scene_create("scene");
	obs_scene_add(scene, media);

	memset(w, 0, sizeof(*w));
	pthread_mutex_init(&w->mutex, NULL);
	obs_add_raw_video_callback(NULL, raw_video, w);
	obs_set_output_source(0, obs_scene_get_source(scene));

	bool ended = false;
	for (uint32_t t = 0; t < play_ms && !ended; t += 10) {
		os_sleep_ms(10);
		ended = !looping && obs_source_media_get_state(media) == OBS_MEDIA_STATE_ENDED;
	}
	os_sleep_ms(300);

	obs_remove_raw_video_callback(raw_video, w);
	obs_set_output_source(0, NULL);
	/* the main canvas holds a reference to its scenes until they are removed */
	obs_source_remove(obs_scene_get_source(scene));
	obs_scene_release(scene);
	obs_source_release(media);
	pthread_mutex_destroy(&w->mutex);

	if (!looping && !ended) {
		fprintf(stderr, "playback did not end\n");
		fail();
	}
}

/* Collapses the recorded output frames into runs of the same picture. */
struct run {
	int what;
	size_t frames;
};

static size_t collapse(const struct watch *w, struct run *runs, size_t max)
{
	size_t n = 0;
	for (size_t i = 0; i < w->count; i++) {
		if (n && runs[n - 1].what == w->seen[i]) {
			runs[n - 1].frames++;
		} else if (n < max) {
			runs[n].what = w->seen[i];
			runs[n].frames = 1;
			n++;
		}
	}
	return n;
}

static void log_runs(const char *what, const struct run *runs, size_t n)
{
	fprintf(stderr, "%s:", what);
	for (size_t i = 0; i < n; i++) {
		if (runs[i].what == NOTHING)
			fprintf(stderr, " nothing");
		else if (runs[i].what == UNKNOWN)
			fprintf(stderr, " ?");
		else
			fprintf(stderr, " %d", runs[i].what);
		fprintf(stderr, "x%zu", runs[i].frames);
	}
	fprintf(stderr, "\n");
}

/* The pictures from the first frame of the clip on, without blanks or
 * blends (an output frame drawn while the picture changes), must be the
 * frames of the clip in order, repeated `loops` times. Pictures before the
 * first frame are left out: the first output frame recorded can still be
 * the last picture of the case before. Returns the run that showed the
 * clip's last frame the first time through. */
static size_t check_order(const struct run *runs, size_t n, int loops)
{
	int expected = 0;
	int done = 0;
	bool started = false;
	size_t last = n;

	for (size_t i = 0; i < n && done < loops; i++) {
		if (runs[i].what < 0)
			continue;
		if (!started && runs[i].what != 0)
			continue;
		started = true;

		if (runs[i].what != expected) {
			fprintf(stderr, "frame %d was shown where frame %d was expected (%d of %d frames shown)\n",
				runs[i].what, expected, expected, FRAMES);
			fail();
		}
		if (++expected == FRAMES) {
			if (!done)
				last = i;
			expected = 0;
			done++;
		}
	}

	if (!started) {
		fprintf(stderr, "the first frame was never shown\n");
		fail();
	}
	if (done < loops) {
		fprintf(stderr, "only %d of %d frames were shown before playback ended\n", expected, FRAMES);
		fail();
	}
	return last;
}

/* The clip's last frame (the run check_order found) must stay up about as
 * long as the others (three output frames at 30 fps), not only flash. */
static void check_last_frame_held(const struct run *runs, size_t last)
{
	if (runs[last].frames < 2) {
		fprintf(stderr, "the last frame was shown for only %zu output frame(s)\n", runs[last].frames);
		fail();
	}
}

/* The report: a one-shot GIF must show all of its frames. */
static void gif_once_shows_every_frame(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	static struct watch w;
	static struct run runs[MAX_SEEN];
	play(gif_once_path, false, true, 5000, &w);
	size_t n = collapse(&w, runs, MAX_SEEN);
	log_runs("one-shot GIF", runs, n);
	check_last_frame_held(runs, check_order(runs, n, 1));
}

/* Guard: a looping GIF shows all of its frames on every loop. */
static void gif_loop_shows_every_frame(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	static struct watch w;
	static struct run runs[MAX_SEEN];
	play(gif_loop_path, true, true, 2600, &w);
	size_t n = collapse(&w, runs, MAX_SEEN);
	log_runs("looping GIF", runs, n);
	check_order(runs, n, 2);
}

/* The same frames as a video file, played once. */
static void video_once_shows_every_frame(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	static struct watch w;
	static struct run runs[MAX_SEEN];
	play(video_path, false, true, 5000, &w);
	size_t n = collapse(&w, runs, MAX_SEEN);
	log_runs("one-shot video", runs, n);
	check_last_frame_held(runs, check_order(runs, n, 1));
}

/* The video once more, right after a run that ended on its last frame
 * ("Show nothing when playback ends" off). The first output frame recorded
 * can still be the last picture of the run before (libobs hands a new raw
 * video callback a frame it rendered before the callback was added), so
 * the recording starts with the clip's last frame for one output frame.
 * That is not the clip playing: the checks are about the frames from its
 * first frame on. Which picture the run before ends on depends on timing
 * (the looping case before video_once_shows_every_frame ends anywhere in
 * the clip), so this makes the case that timing hits now and then happen
 * every time. */
static void video_once_after_last_frame_held(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	static struct watch w;
	static struct run runs[MAX_SEEN];
	play(video_path, false, false, 5000, &w);
	size_t n = collapse(&w, runs, MAX_SEEN);
	log_runs("one-shot video, last frame held", runs, n);
	if (!n || runs[n - 1].what != FRAMES - 1) {
		fprintf(stderr, "the run before did not end on the last frame\n");
		fail();
	}

	play(video_path, false, true, 5000, &w);
	n = collapse(&w, runs, MAX_SEEN);
	log_runs("one-shot video again", runs, n);
	check_last_frame_held(runs, check_order(runs, n, 1));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(gif_once_shows_every_frame),
		cmocka_unit_test(gif_loop_shows_every_frame),
		cmocka_unit_test(video_once_shows_every_frame),
		cmocka_unit_test(video_once_after_last_frame_held),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
