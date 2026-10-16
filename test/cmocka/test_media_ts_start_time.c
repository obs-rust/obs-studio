/*
 * A Media Source playing a file whose timestamps do not start at 0 (an
 * MPEG-TS file: broadcast and camera recordings start anywhere, and the
 * ffmpeg tool's own .ts files start at about 1.4 s) reports the media time
 * with the start time added and seeks to the wrong place. media-playback
 * hands the decoded timestamps out as the media time, so a .ts that starts
 * at 1000 s shows 16:41 at its first frame, and it seeks by passing the
 * requested time to av_seek_frame() as a timestamp of the file, so every
 * seek lands start-time early: on that file any seek goes back to the
 * first frame, on a 1.4 s one a seek lands 1.4 s early and the last 1.4 s
 * can't be reached. FFmpeg's timestamps count from AVFormatContext's
 * start_time; restarting (mp_media_reset) already seeks to it.
 *
 * The cases write a 30 fps MPEG-TS clip whose timestamps start at 100 s and
 * whose frames carry their frame number in two flat luma blocks (every frame
 * a keyframe, so where a seek lands depends on the time alone), play it
 * through media-playback (the code behind ffmpeg_source) and read the frame
 * shown and the media time. Both the direct and the fully decoded (cached)
 * path are checked. Guard: the same clip in Matroska, starting at 0. No
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

#include <obs.h>
#include <util/platform.h>
#include <util/threading.h>

#include <media-playback/media-playback.h>

#define W 64
#define H 64
#define FPS 30
#define CLIP_FRAMES (FPS * 8)
#define START_S 100 /* the MPEG-TS clip's timestamps start here */

/* luma code of the frame number: the left block holds n % 30, the right one
 * n / 30, in steps the encoder keeps apart */
#define Y_BASE 16
#define LEFT_STEP 7
#define RIGHT_STEP 20

#define FRAME_MS (1000 / FPS)
#define TIME_SLACK_MS (2 * FRAME_MS)

static bool files_ok;
static char ts_path[512];
static char mkv_path[512];

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

/* MPEG-4 part 2 video (built into FFmpeg), every frame a keyframe, with
 * timestamps from start_frame on. */
static bool write_clip(const char *path, const char *format, int start_frame)
{
	AVFormatContext *oc = NULL;
	AVCodecContext *venc = NULL;
	AVStream *vs = NULL;
	AVPacket *pkt = av_packet_alloc();
	AVFrame *vf = av_frame_alloc();
	bool ok = false;

	if (!pkt || !vf || avformat_alloc_output_context2(&oc, NULL, format, path) < 0)
		goto done;

	const AVCodec *vcodec = avcodec_find_encoder(AV_CODEC_ID_MPEG4);
	if (!vcodec || !(venc = avcodec_alloc_context3(vcodec)))
		goto done;
	venc->width = W;
	venc->height = H;
	venc->pix_fmt = AV_PIX_FMT_YUV420P;
	venc->time_base = (AVRational){1, FPS};
	venc->framerate = (AVRational){FPS, 1};
	venc->gop_size = 1;
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

	if (avio_open(&oc->pb, path, AVIO_FLAG_WRITE) < 0)
		goto done;
	if (avformat_write_header(oc, NULL) < 0)
		goto done_io;

	vf->width = W;
	vf->height = H;
	vf->format = AV_PIX_FMT_YUV420P;
	if (av_frame_get_buffer(vf, 0) < 0)
		goto done_io;

	for (int n = 0; n < CLIP_FRAMES; n++) {
		if (av_frame_make_writable(vf) < 0)
			goto done_io;
		draw_frame(vf, n);
		vf->pts = start_frame + n;
		if (avcodec_send_frame(venc, vf) < 0 || !write_packets(oc, venc, vs, pkt))
			goto done_io;
	}
	if (avcodec_send_frame(venc, NULL) < 0 || !write_packets(oc, venc, vs, pkt))
		goto done_io;

	ok = av_write_trailer(oc) >= 0;

done_io:
	avio_closep(&oc->pb);
done:
	av_frame_free(&vf);
	av_packet_free(&pkt);
	avcodec_free_context(&venc);
	avformat_free_context(oc);
	return ok;
}

/* The start time the demuxer reports, in ms, or -1. */
static int64_t start_time_ms(const char *path)
{
	AVFormatContext *ic = NULL;
	if (avformat_open_input(&ic, path, NULL, NULL) < 0)
		return -1;
	int64_t ms = -1;
	if (avformat_find_stream_info(ic, NULL) >= 0 && ic->start_time != AV_NOPTS_VALUE)
		ms = ic->start_time / 1000;
	avformat_close_input(&ic);
	return ms;
}

static void temp_path(char *buf, size_t size, const char *name)
{
	const char *tmp = getenv("TMPDIR");
	if (!tmp || !*tmp)
		tmp = getenv("TEMP");
	if (!tmp || !*tmp)
		tmp = "/tmp";
	snprintf(buf, size, "%s/obs-ts-start-time-%llu-%s", tmp, (unsigned long long)os_gettime_ns(), name);
}

static int group_setup(void **state)
{
	UNUSED_PARAMETER(state);
	temp_path(ts_path, sizeof(ts_path), "clip.ts");
	temp_path(mkv_path, sizeof(mkv_path), "clip.mkv");
	if (!write_clip(ts_path, "mpegts", START_S * FPS) || !write_clip(mkv_path, "matroska", 0)) {
		fprintf(stderr, "could not write the test clips\n");
		return -1;
	}
	int64_t ts_start = start_time_ms(ts_path);
	int64_t mkv_start = start_time_ms(mkv_path);
	fprintf(stderr, "start times: ts %lld ms, mkv %lld ms\n", (long long)ts_start, (long long)mkv_start);
	if (ts_start < START_S * 1000 || ts_start > START_S * 1000 + 1000 || mkv_start != 0) {
		fprintf(stderr, "the test clips do not have the expected start times\n");
		return -1;
	}
	files_ok = true;
	return 0;
}

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	os_unlink(ts_path);
	os_unlink(mkv_path);
	return 0;
}

/* ------------------------------------------------------------------------ */
/* Player */

struct player {
	media_playback_t *mp;
	pthread_mutex_t mutex;

	int last_frame;
	uint64_t frames_seen;

	/* after arm(): the first frame that is not the continuation of
	 * playback from before the seek */
	bool armed;
	int armed_from;
	int first_frame;
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
	p->got_seek_preview = false;
	pthread_mutex_unlock(&p->mutex);
}

static void pause_playback(struct player *p)
{
	media_playback_play_pause(p->mp, true);
	os_sleep_ms(200);
}

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

static void expect_frame(const char *what, int64_t target_ms, int got)
{
	int want = (int)(target_ms * FPS / 1000);
	if (got < want - 1 || got > want + 1) {
		fprintf(stderr, "%s: seek to %lld ms showed frame %d (%lld ms of the clip), expected frame %d\n", what,
			(long long)target_ms, got, (long long)got * 1000 / FPS, want);
		fail();
	}
}

static void expect_time(const char *what, int64_t target_ms, int64_t got)
{
	if (got < target_ms - TIME_SLACK_MS || got > target_ms + TIME_SLACK_MS) {
		fprintf(stderr, "%s: expected a media time of %lld ms, it reports %lld ms\n", what,
			(long long)target_ms, (long long)got);
		fail();
	}
}

/* The media time while playing counts from the first frame, not from the
 * file's first timestamp. */
static void media_time_counts_from_first_frame(const char *path, bool full_decode)
{
	struct player p;
	player_start(&p, path, full_decode);
	wait_for_frame(&p, 15);
	pause_playback(&p);
	int shown = last_frame(&p);
	int64_t t = media_playback_get_current_time(p.mp);
	int64_t duration = media_playback_get_duration(p.mp) / 1000;
	player_stop(&p);

	fprintf(stderr, "%s %s: frame %d shown, media time %lld ms of %lld ms\n", full_decode ? "cached" : "direct",
		path == ts_path ? "ts" : "mkv", shown, (long long)t, (long long)duration);
	/* the media time runs up to a frame ahead of the frame shown */
	if (t < (int64_t)shown * 1000 / FPS - TIME_SLACK_MS || t > (int64_t)(shown + 1) * 1000 / FPS + TIME_SLACK_MS) {
		fprintf(stderr, "media time %lld ms while frame %d (%lld ms) is shown\n", (long long)t, shown,
			(long long)shown * 1000 / FPS);
		fail();
	}
	if (duration < CLIP_FRAMES * 1000 / FPS - TIME_SLACK_MS ||
	    duration > CLIP_FRAMES * 1000 / FPS + TIME_SLACK_MS) {
		fprintf(stderr, "duration %lld ms, the clip is %d ms\n", (long long)duration, CLIP_FRAMES * 1000 / FPS);
		fail();
	}
}

/* Paused (the seek bar, or SetMediaInputCursor on a paused source): the
 * frame shown and the media time after a seek. */
static void paused_seek(const char *path, int64_t target_ms)
{
	struct player p;
	player_start(&p, path, false);
	wait_for_frame(&p, 5);
	pause_playback(&p);

	arm(&p);
	media_playback_seek(p.mp, target_ms);
	int shown = wait_seek_preview(&p);
	os_sleep_ms(100);
	int64_t t = media_playback_get_current_time(p.mp);
	player_stop(&p);

	fprintf(stderr, "%s paused seek to %lld ms: frame %d, media time %lld ms\n", path == ts_path ? "ts" : "mkv",
		(long long)target_ms, shown, (long long)t);
	expect_frame("paused seek", target_ms, shown);
	expect_time("paused seek", target_ms, t);
}

static void ts_media_time_counts_from_first_frame(void **state)
{
	UNUSED_PARAMETER(state);
	assert_true(files_ok);
	media_time_counts_from_first_frame(ts_path, false);
}

static void ts_paused_seek_lands_on_target(void **state)
{
	UNUSED_PARAMETER(state);
	assert_true(files_ok);
	paused_seek(ts_path, 2500);
}

/* The last second of the file: a seek landing start-time early never
 * reaches it. */
static void ts_paused_seek_reaches_the_end(void **state)
{
	UNUSED_PARAMETER(state);
	assert_true(files_ok);
	paused_seek(ts_path, (CLIP_FRAMES - 10) * 1000 / FPS);
}

static void ts_playing_seek_lands_on_target(void **state)
{
	UNUSED_PARAMETER(state);
	assert_true(files_ok);

	const int64_t target_ms = 4500;
	struct player p;
	player_start(&p, ts_path, false);
	wait_for_frame(&p, 9);

	arm(&p);
	media_playback_seek(p.mp, target_ms);
	int shown = wait_first_frame(&p);
	int64_t t = media_playback_get_current_time(p.mp);
	player_stop(&p);

	fprintf(stderr, "ts playing seek to %lld ms: first frame %d, media time %lld ms\n", (long long)target_ms, shown,
		(long long)t);
	expect_frame("playing seek", target_ms, shown);
	if (t < target_ms - TIME_SLACK_MS || t > target_ms + 500) {
		fprintf(stderr, "playing seek: seek to %lld ms reports a media time of %lld ms\n", (long long)target_ms,
			(long long)t);
		fail();
	}
}

/* The fully decoded (cached) path: the media time, and a seek. The cache
 * shows no preview while paused, so the seek is checked on resume. */
static void ts_cached_media_time_counts_from_first_frame(void **state)
{
	UNUSED_PARAMETER(state);
	assert_true(files_ok);
	media_time_counts_from_first_frame(ts_path, true);
}

static void ts_cached_seek_lands_on_target(void **state)
{
	UNUSED_PARAMETER(state);
	assert_true(files_ok);

	const int64_t target_ms = 2500;
	struct player p;
	player_start(&p, ts_path, true);
	wait_for_frame(&p, 5);
	pause_playback(&p);

	arm(&p);
	media_playback_seek(p.mp, target_ms);
	os_sleep_ms(200);
	media_playback_play_pause(p.mp, false);
	int shown = wait_first_frame(&p);
	player_stop(&p);

	fprintf(stderr, "ts cached seek to %lld ms: first frame %d\n", (long long)target_ms, shown);
	expect_frame("cached seek", target_ms, shown);
}

/* Guards: a file starting at 0 behaves the same before and after. */
static void mkv_media_time_counts_from_first_frame(void **state)
{
	UNUSED_PARAMETER(state);
	assert_true(files_ok);
	media_time_counts_from_first_frame(mkv_path, false);
}

static void mkv_paused_seek_lands_on_target(void **state)
{
	UNUSED_PARAMETER(state);
	assert_true(files_ok);
	paused_seek(mkv_path, 2500);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(ts_media_time_counts_from_first_frame),
		cmocka_unit_test(ts_paused_seek_lands_on_target),
		cmocka_unit_test(ts_paused_seek_reaches_the_end),
		cmocka_unit_test(ts_playing_seek_lands_on_target),
		cmocka_unit_test(ts_cached_media_time_counts_from_first_frame),
		cmocka_unit_test(ts_cached_seek_lands_on_target),
		cmocka_unit_test(mkv_media_time_counts_from_first_frame),
		cmocka_unit_test(mkv_paused_seek_lands_on_target),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
