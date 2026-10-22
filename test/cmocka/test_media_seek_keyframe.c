/*
 * Seeking a Media Source must land on the requested time, not on the
 * keyframe before it (obsproject/obs-websocket#1190, reported by
 * casperboone, with lrq51678, jwetzell and chnoma): SetMediaInputCursor to
 * 20210 ms showed and reported 20040 ms, and on a file with a keyframe every
 * 3 seconds, 1500 ms went to 0 and 5500 ms to 3000. The VLC source seeks
 * exactly, so the media-playback seek is at fault: it seeks the demuxer
 * backward to the keyframe at or before the target (AVSEEK_FLAG_BACKWARD),
 * flushes the decoders and plays from that keyframe, so both the frame
 * shown and the media time are the keyframe's.
 *
 * The cases write a 30 fps clip with a keyframe every 90 frames (3 s) whose
 * frames carry their frame number in two flat luma blocks, play it through
 * media-playback (the code behind ffmpeg_source) and read the frame number
 * of the first frame shown after a seek and the media time. One clip also
 * has an audio track whose level steps up every 500 ms, so the first audio
 * after the seek shows where audio resumed. Guards: a seek onto a keyframe,
 * and the fully decoded (cached) path, which already seeks exactly. No
 * display or graphics are needed.
 */

#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <cmocka.h>

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
#include <libavutil/channel_layout.h>

#include <obs.h>
#include <util/platform.h>
#include <util/threading.h>

#include <media-playback/media-playback.h>

#define W 64
#define H 64
#define FPS 30
#define GOP 90 /* a keyframe every 3 seconds */
#define CLIP_FRAMES (FPS * 8)
#define RATE 48000

/* luma code of the frame number: the left block holds n % 30, the right one
 * n / 30, in steps the encoder keeps apart */
#define Y_BASE 16
#define LEFT_STEP 7
#define RIGHT_STEP 20

/* audio level steps up every 500 ms */
#define LEVEL_STEP_MS 500
#define LEVEL_BASE 0.05f
#define LEVEL_STEP 0.05f

/* one frame at 30 fps, and some slack for the media time */
#define FRAME_MS (1000 / FPS)
#define TIME_SLACK_MS (2 * FRAME_MS)
/* how far a playing source's reported time may run ahead of the frame it
 * shows (it decodes ahead); the bound before #1766 */
#define PLAYING_AHEAD_MS 500

static bool files_ok;
static char video_path[512];
static char av_path[512];

/* ------------------------------------------------------------------------ */
/* Test files */

static void draw_frame(AVFrame *f, int n)
{
	int left = Y_BASE + (n % FPS) * LEFT_STEP;
	int right = Y_BASE + (n / FPS) * RIGHT_STEP;
	for (int y = 0; y < H; y++) {
		memset(f->data[0] + y * f->linesize[0], left, W / 2);
		memset(f->data[0] + y * f->linesize[0] + W / 2, right, W / 2);
	}
	for (int y = 0; y < H / 2; y++) {
		memset(f->data[1] + y * f->linesize[1], 128, W / 2);
		memset(f->data[2] + y * f->linesize[2], 128, W / 2);
	}
}

static int frame_number(const uint8_t *y_plane, uint32_t linesize)
{
	const uint8_t *row = y_plane + (H / 2) * linesize;
	int left = row[W / 4];
	int right = row[3 * W / 4];
	int lo = (int)lround((double)(left - Y_BASE) / LEFT_STEP);
	int hi = (int)lround((double)(right - Y_BASE) / RIGHT_STEP);
	return hi * FPS + lo;
}

static float level_at(int64_t sample)
{
	int64_t step = sample * 1000 / RATE / LEVEL_STEP_MS;
	return LEVEL_BASE + LEVEL_STEP * (float)step;
}

static bool write_packets(AVFormatContext *oc, AVCodecContext *enc, AVStream *st, AVPacket *pkt)
{
	int ret;
	while ((ret = avcodec_receive_packet(enc, pkt)) >= 0) {
		pkt->stream_index = st->index;
		av_packet_rescale_ts(pkt, enc->time_base, st->time_base);
		if (av_interleaved_write_frame(oc, pkt) < 0)
			return false;
	}
	return ret == AVERROR(EAGAIN) || ret == AVERROR_EOF;
}

/* MPEG-4 part 2 video (built into FFmpeg, unlike H.264), no B-frames, a
 * keyframe every GOP frames; with audio, an AAC track (also built in). */
static bool write_clip(const char *path, bool with_audio)
{
	AVFormatContext *oc = NULL;
	AVCodecContext *venc = NULL, *aenc = NULL;
	AVStream *vs = NULL, *as = NULL;
	AVPacket *pkt = av_packet_alloc();
	AVFrame *vf = av_frame_alloc();
	AVFrame *af = av_frame_alloc();
	bool ok = false;

	if (!pkt || !vf || !af || avformat_alloc_output_context2(&oc, NULL, "mp4", path) < 0)
		goto done;

	const AVCodec *vcodec = avcodec_find_encoder(AV_CODEC_ID_MPEG4);
	if (!vcodec || !(venc = avcodec_alloc_context3(vcodec)))
		goto done;
	venc->width = W;
	venc->height = H;
	venc->pix_fmt = AV_PIX_FMT_YUV420P;
	venc->time_base = (AVRational){1, FPS};
	venc->framerate = (AVRational){FPS, 1};
	venc->gop_size = GOP;
	venc->keyint_min = GOP;
	venc->max_b_frames = 0;
	venc->flags |= AV_CODEC_FLAG_QSCALE;
	venc->global_quality = FF_QP2LAMBDA * 2;
	if (oc->oformat->flags & AVFMT_GLOBALHEADER)
		venc->flags |= AV_CODEC_FLAG_GLOBAL_HEADER;
	if (avcodec_open2(venc, vcodec, NULL) < 0)
		goto done;
	if (!(vs = avformat_new_stream(oc, NULL)) || avcodec_parameters_from_context(vs->codecpar, venc) < 0)
		goto done;
	vs->time_base = venc->time_base;

	if (with_audio) {
		const AVCodec *acodec = avcodec_find_encoder(AV_CODEC_ID_AAC);
		if (!acodec || !(aenc = avcodec_alloc_context3(acodec)))
			goto done;
		aenc->sample_rate = RATE;
		aenc->sample_fmt = AV_SAMPLE_FMT_FLTP;
		aenc->bit_rate = 128000;
		aenc->time_base = (AVRational){1, RATE};
		av_channel_layout_default(&aenc->ch_layout, 1);
		if (oc->oformat->flags & AVFMT_GLOBALHEADER)
			aenc->flags |= AV_CODEC_FLAG_GLOBAL_HEADER;
		if (avcodec_open2(aenc, acodec, NULL) < 0)
			goto done;
		if (!(as = avformat_new_stream(oc, NULL)) || avcodec_parameters_from_context(as->codecpar, aenc) < 0)
			goto done;
		as->time_base = aenc->time_base;
	}

	if (avio_open(&oc->pb, path, AVIO_FLAG_WRITE) < 0)
		goto done;
	if (avformat_write_header(oc, NULL) < 0)
		goto done_io;

	vf->width = W;
	vf->height = H;
	vf->format = AV_PIX_FMT_YUV420P;
	if (av_frame_get_buffer(vf, 0) < 0)
		goto done_io;

	const int64_t total_samples = (int64_t)RATE * CLIP_FRAMES / FPS;
	int64_t samples = 0;
	const int chunk = aenc && aenc->frame_size > 0 ? aenc->frame_size : 1024;

	for (int n = 0; n < CLIP_FRAMES; n++) {
		if (av_frame_make_writable(vf) < 0)
			goto done_io;
		draw_frame(vf, n);
		vf->pts = n;
		if (avcodec_send_frame(venc, vf) < 0 || !write_packets(oc, venc, vs, pkt))
			goto done_io;

		/* audio up to the end of this video frame */
		int64_t until = (int64_t)RATE * (n + 1) / FPS;
		while (aenc && samples < until && samples < total_samples) {
			av_frame_unref(af);
			af->nb_samples = chunk;
			af->format = aenc->sample_fmt;
			af->sample_rate = RATE;
			if (av_channel_layout_copy(&af->ch_layout, &aenc->ch_layout) < 0 ||
			    av_frame_get_buffer(af, 0) < 0)
				goto done_io;
			float *out = (float *)af->data[0];
			for (int i = 0; i < chunk; i++) {
				int64_t s = samples + i;
				out[i] = level_at(s) * (float)sin(2.0 * M_PI * 440.0 * (double)s / RATE);
			}
			af->pts = samples;
			samples += chunk;
			if (avcodec_send_frame(aenc, af) < 0 || !write_packets(oc, aenc, as, pkt))
				goto done_io;
		}
	}
	if (avcodec_send_frame(venc, NULL) < 0 || !write_packets(oc, venc, vs, pkt))
		goto done_io;
	if (aenc && (avcodec_send_frame(aenc, NULL) < 0 || !write_packets(oc, aenc, as, pkt)))
		goto done_io;

	ok = av_write_trailer(oc) >= 0;

done_io:
	avio_closep(&oc->pb);
done:
	av_frame_free(&vf);
	av_frame_free(&af);
	av_packet_free(&pkt);
	avcodec_free_context(&venc);
	avcodec_free_context(&aenc);
	avformat_free_context(oc);
	return ok;
}

/* The keyframes the demuxer sees, so a muxer that moves them shows up as a
 * setup failure, not as a wrong seek. */
static bool check_keyframes(const char *path)
{
	AVFormatContext *ic = NULL;
	AVPacket *pkt = av_packet_alloc();
	bool ok = true;
	int frames = 0;

	if (!pkt || avformat_open_input(&ic, path, NULL, NULL) < 0 || avformat_find_stream_info(ic, NULL) < 0) {
		av_packet_free(&pkt);
		avformat_close_input(&ic);
		return false;
	}
	int vi = av_find_best_stream(ic, AVMEDIA_TYPE_VIDEO, -1, -1, NULL, 0);
	while (vi >= 0 && av_read_frame(ic, pkt) >= 0) {
		if (pkt->stream_index == vi) {
			bool key = (pkt->flags & AV_PKT_FLAG_KEY) != 0;
			if (key != (frames % GOP == 0)) {
				fprintf(stderr, "%s: frame %d keyframe=%d, expected one every %d frames\n", path,
					frames, key, GOP);
				ok = false;
			}
			frames++;
		}
		av_packet_unref(pkt);
	}
	av_packet_free(&pkt);
	avformat_close_input(&ic);
	return ok && frames == CLIP_FRAMES;
}

static void temp_path(char *buf, size_t size, const char *name)
{
	const char *tmp = getenv("TMPDIR");
	if (!tmp || !*tmp)
		tmp = getenv("TEMP");
	if (!tmp || !*tmp)
		tmp = "/tmp";
	snprintf(buf, size, "%s/obs-seek-keyframe-%llu-%s", tmp, (unsigned long long)os_gettime_ns(), name);
}

static int group_setup(void **state)
{
	UNUSED_PARAMETER(state);
	temp_path(video_path, sizeof(video_path), "video.mp4");
	temp_path(av_path, sizeof(av_path), "av.mp4");
	if (!write_clip(video_path, false) || !write_clip(av_path, true)) {
		fprintf(stderr, "could not write the test clips\n");
		return -1;
	}
	if (!check_keyframes(video_path) || !check_keyframes(av_path)) {
		fprintf(stderr, "the test clips do not have the expected keyframes\n");
		return -1;
	}
	files_ok = true;
	return 0;
}

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	os_unlink(video_path);
	os_unlink(av_path);
	return 0;
}

/* ------------------------------------------------------------------------ */
/* Player */

struct player {
	media_playback_t *mp;
	pthread_mutex_t mutex;

	/* frames shown (played or as the paused seek preview) */
	int last_frame;
	uint64_t frames_seen;

	/* after arm(): the first frame that is not the continuation of
	 * playback from before the seek, and the first audio level */
	bool armed;
	int armed_from;
	int first_frame;
	float first_level;
	bool got_level;
	bool got_seek_preview;
	int seek_preview_frame;
};

static void on_frame(struct player *p, struct obs_source_frame *f, bool seek_preview)
{
	if (f->format != VIDEO_FORMAT_I420 || f->width != W || f->height != H)
		return;
	int n = frame_number(f->data[0], f->linesize[0]);

	pthread_mutex_lock(&p->mutex);
	p->last_frame = n;
	p->frames_seen++;
	if (seek_preview) {
		p->got_seek_preview = true;
		p->seek_preview_frame = n;
	}
	/* a playing seek is taken a few frames after arm(); frames up to then
	 * continue the playback from before it */
	if (p->armed && p->first_frame < 0 && (n < p->armed_from || n > p->armed_from + 10))
		p->first_frame = n;
	pthread_mutex_unlock(&p->mutex);
}

static void video_cb(void *opaque, struct obs_source_frame *f)
{
	on_frame(opaque, f, false);
}

static void preload_cb(void *opaque, struct obs_source_frame *f)
{
	UNUSED_PARAMETER(opaque);
	UNUSED_PARAMETER(f);
}

static void seek_cb(void *opaque, struct obs_source_frame *f)
{
	on_frame(opaque, f, true);
}

static void audio_cb(void *opaque, struct obs_source_audio *a)
{
	struct player *p = opaque;
	if (a->format != AUDIO_FORMAT_FLOAT_PLANAR && a->format != AUDIO_FORMAT_FLOAT)
		return;
	const float *s = (const float *)a->data[0];
	float peak = 0.0f;
	for (uint32_t i = 0; i < a->frames; i++)
		peak = fmaxf(peak, fabsf(s[i]));

	pthread_mutex_lock(&p->mutex);
	if (p->armed && !p->got_level) {
		p->first_level = peak;
		p->got_level = true;
	}
	pthread_mutex_unlock(&p->mutex);
}

static void stop_cb(void *opaque)
{
	UNUSED_PARAMETER(opaque);
}

static void player_start(struct player *p, const char *path, bool full_decode)
{
	memset(p, 0, sizeof(*p));
	pthread_mutex_init(&p->mutex, NULL);
	p->last_frame = -1;
	p->first_frame = -1;

	struct mp_media_info info = {
		.opaque = p,
		.v_cb = video_cb,
		.v_preload_cb = preload_cb,
		.v_seek_cb = seek_cb,
		.a_cb = audio_cb,
		.stop_cb = stop_cb,
		.path = path,
		.speed = 100,
		.force_range = VIDEO_RANGE_DEFAULT,
		.is_local_file = true,
		.full_decode = full_decode,
	};
	p->mp = media_playback_create(&info);
	assert_non_null(p->mp);
	media_playback_play(p->mp, false, false);
}

static void player_stop(struct player *p)
{
	media_playback_destroy(p->mp);
	pthread_mutex_destroy(&p->mutex);
}

static int last_frame(struct player *p)
{
	pthread_mutex_lock(&p->mutex);
	int n = p->last_frame;
	pthread_mutex_unlock(&p->mutex);
	return n;
}

/* Warm-up: playback runs and has shown frame `n` or later. */
static void wait_for_frame(struct player *p, int n)
{
	for (int i = 0; i < 1000; i++) {
		if (last_frame(p) >= n)
			return;
		os_sleep_ms(10);
	}
	fprintf(stderr, "playback did not reach frame %d (last frame %d)\n", n, last_frame(p));
	fail();
}

static void arm(struct player *p)
{
	pthread_mutex_lock(&p->mutex);
	p->armed = true;
	p->armed_from = p->last_frame;
	p->first_frame = -1;
	p->got_level = false;
	p->got_seek_preview = false;
	pthread_mutex_unlock(&p->mutex);
}

static void pause_playback(struct player *p)
{
	media_playback_play_pause(p->mp, true);
	/* let the media thread take the pause */
	os_sleep_ms(200);
}

/* Waits for the paused seek's preview frame; returns its frame number. */
static int wait_seek_preview(struct player *p)
{
	for (int i = 0; i < 500; i++) {
		pthread_mutex_lock(&p->mutex);
		bool got = p->got_seek_preview;
		int n = p->seek_preview_frame;
		pthread_mutex_unlock(&p->mutex);
		if (got)
			return n;
		os_sleep_ms(10);
	}
	fprintf(stderr, "no frame was shown after the paused seek\n");
	fail();
	return -1;
}

/* Waits for the first frame after the seek; returns its frame number. */
static int wait_first_frame(struct player *p)
{
	for (int i = 0; i < 500; i++) {
		pthread_mutex_lock(&p->mutex);
		int n = p->first_frame;
		pthread_mutex_unlock(&p->mutex);
		if (n >= 0)
			return n;
		os_sleep_ms(10);
	}
	fprintf(stderr, "no frame was shown after the seek\n");
	fail();
	return -1;
}

static float wait_first_level(struct player *p)
{
	for (int i = 0; i < 500; i++) {
		pthread_mutex_lock(&p->mutex);
		bool got = p->got_level;
		float level = p->first_level;
		pthread_mutex_unlock(&p->mutex);
		if (got)
			return level;
		os_sleep_ms(10);
	}
	fprintf(stderr, "no audio was played after the seek\n");
	fail();
	return 0.0f;
}

static int expected_frame(int64_t ms)
{
	return (int)(ms * FPS / 1000);
}

static void expect_frame(const char *what, int64_t target_ms, int got)
{
	int want = expected_frame(target_ms);
	if (got < want - 1 || got > want + 1) {
		fprintf(stderr, "%s: seek to %lld ms showed frame %d (%lld ms), expected frame %d\n", what,
			(long long)target_ms, got, (long long)got * 1000 / FPS, want);
		fail();
	}
}

static void expect_time(const char *what, int64_t target_ms, int64_t got)
{
	if (got < target_ms - TIME_SLACK_MS || got > target_ms + TIME_SLACK_MS) {
		fprintf(stderr, "%s: seek to %lld ms reports a media time of %lld ms\n", what, (long long)target_ms,
			(long long)got);
		fail();
	}
}

/* Paused, as in the reports (SetMediaInputCursor on a paused source, or the
 * seek bar): the frame shown and the media time after a seek. */
static void paused_seek(const char *path, bool full_decode, int64_t target_ms)
{
	struct player p;
	player_start(&p, path, full_decode);
	wait_for_frame(&p, 5);
	pause_playback(&p);

	arm(&p);
	media_playback_seek(p.mp, target_ms);
	int shown = wait_seek_preview(&p);
	os_sleep_ms(100);
	int64_t t = media_playback_get_current_time(p.mp);

	player_stop(&p);

	fprintf(stderr, "%s paused seek to %lld ms: frame %d, media time %lld ms\n", full_decode ? "cached" : "direct",
		(long long)target_ms, shown, (long long)t);
	expect_frame("paused seek", target_ms, shown);
	expect_time("paused seek", target_ms, t);
}

static void paused_seek_between_keyframes(void **state)
{
	UNUSED_PARAMETER(state);
	assert_true(files_ok);
	/* chnoma's case: 1500 ms went to 0 */
	paused_seek(video_path, false, 1500);
}

static void paused_seek_into_second_gop(void **state)
{
	UNUSED_PARAMETER(state);
	assert_true(files_ok);
	/* chnoma's case: 5500 ms went to 3000 */
	paused_seek(video_path, false, 5500);
}

/* The last frame, after the last keyframe (at 6000 ms): the frames up to
 * the end of the file are dropped but the last one, not all of them. */
static void paused_seek_to_last_frame(void **state)
{
	UNUSED_PARAMETER(state);
	assert_true(files_ok);
	paused_seek(video_path, false, (CLIP_FRAMES - 1) * 1000 / FPS + FRAME_MS / 2);
}

/* Guard: a seek onto a keyframe lands there before and after the fix. */
static void paused_seek_onto_keyframe(void **state)
{
	UNUSED_PARAMETER(state);
	assert_true(files_ok);
	paused_seek(video_path, false, 3000);
}

/* Guard: the fully decoded (cached) path already seeks exactly. It shows no
 * preview while paused, so it is checked when playback resumes. */
static void cached_seek_between_keyframes(void **state)
{
	UNUSED_PARAMETER(state);
	assert_true(files_ok);

	const int64_t target_ms = 1500;
	struct player p;
	player_start(&p, video_path, true);
	wait_for_frame(&p, 5);
	pause_playback(&p);

	arm(&p);
	media_playback_seek(p.mp, target_ms);
	os_sleep_ms(200);
	media_playback_play_pause(p.mp, false);
	int shown = wait_first_frame(&p);

	player_stop(&p);

	fprintf(stderr, "cached seek to %lld ms: first frame %d\n", (long long)target_ms, shown);
	expect_frame("cached seek", target_ms, shown);
}

/* While playing: the first frame after the seek and the media time right
 * after it. */
static void playing_seek_between_keyframes(void **state)
{
	UNUSED_PARAMETER(state);
	assert_true(files_ok);

	const int64_t target_ms = 4500;
	struct player p;
	player_start(&p, video_path, false);
	wait_for_frame(&p, 9);

	arm(&p);
	const uint64_t seek_ns = os_gettime_ns();
	media_playback_seek(p.mp, target_ms);
	int shown = wait_first_frame(&p);
	int64_t t = media_playback_get_current_time(p.mp);
	const int64_t elapsed_ms = (int64_t)((os_gettime_ns() - seek_ns) / 1000000);

	player_stop(&p);

	fprintf(stderr, "playing seek to %lld ms: first frame %d, media time %lld ms, %lld ms after the seek\n",
		(long long)target_ms, shown, (long long)t, (long long)elapsed_ms);
	expect_frame("playing seek", target_ms, shown);
	/* it plays on from the target, so allow the real time that passed
	 * between the seek and reading the media time (a slow runner can take
	 * longer than any fixed bound to show the first frame), plus how far
	 * the reported time can run ahead of the shown frame while it decodes
	 * ahead */
	if (t < target_ms - TIME_SLACK_MS || t > target_ms + elapsed_ms + PLAYING_AHEAD_MS) {
		fprintf(stderr, "playing seek: seek to %lld ms reports a media time of %lld ms\n", (long long)target_ms,
			(long long)t);
		fail();
	}
}

/* With audio: audio resumes at the target too (not from the keyframe, and
 * not out of step with the video). Seeks while paused, then resumes. */
static void seek_with_audio_resumes_audio_at_target(void **state)
{
	UNUSED_PARAMETER(state);
	assert_true(files_ok);

	const int64_t target_ms = 1750; /* the middle of a level step */
	struct player p;
	player_start(&p, av_path, false);
	wait_for_frame(&p, 5);
	pause_playback(&p);

	arm(&p);
	media_playback_seek(p.mp, target_ms);
	int shown = wait_seek_preview(&p);
	os_sleep_ms(100);
	int64_t t = media_playback_get_current_time(p.mp);
	media_playback_play_pause(p.mp, false);
	float level = wait_first_level(&p);

	player_stop(&p);

	float want = level_at((int64_t)RATE * target_ms / 1000);
	fprintf(stderr, "seek with audio to %lld ms: frame %d, media time %lld ms, first audio level %.3f (%.3f)\n",
		(long long)target_ms, shown, (long long)t, level, want);
	expect_frame("seek with audio", target_ms, shown);
	expect_time("seek with audio", target_ms, t);
	if (fabsf(level - want) > LEVEL_STEP / 2) {
		fprintf(stderr, "seek with audio: audio resumed at level %.3f, the level at %lld ms is %.3f\n", level,
			(long long)target_ms, want);
		fail();
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(paused_seek_between_keyframes),
		cmocka_unit_test(paused_seek_into_second_gop),
		cmocka_unit_test(playing_seek_between_keyframes),
		cmocka_unit_test(seek_with_audio_resumes_audio_at_target),
		cmocka_unit_test(paused_seek_to_last_frame),
		cmocka_unit_test(paused_seek_onto_keyframe),
		cmocka_unit_test(cached_seek_between_keyframes),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
