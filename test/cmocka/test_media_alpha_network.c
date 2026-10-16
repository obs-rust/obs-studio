/*
 * A Media Source playing a WebM with alpha (VP8 or VP9 with an alpha
 * channel, as overlays and alerts are made) from a URL plays the audio but
 * never shows a picture. The same file shows when it is played as a local
 * file, and a WebM without alpha shows from a URL.
 *
 * media-playback drops the video frames of a network input until one is a
 * keyframe (so a stream joined in the middle shows no garbage before its
 * first keyframe) and takes that from AV_FRAME_FLAG_KEY on the decoded
 * frame. For alpha WebM, decode.c picks FFmpeg's libvpx decoders (FFmpeg's
 * own vp8/vp9 decoders ignore the alpha channel), and FFmpeg's libvpx
 * wrapper never sets AV_FRAME_FLAG_KEY, so every frame is dropped.
 *
 * The cases write short WebM clips whose frames carry their frame number in
 * two flat luma blocks and whose right half is transparent, and play them
 * through media-playback (the code behind ffmpeg_source) as a network input
 * (is_local_file = false; the keyframe gate depends on that alone, not on
 * the protocol). They check that the video is shown from the first keyframe
 * with its alpha, and that a clip starting with frames that depend on an
 * earlier keyframe still shows nothing before its first keyframe: for the
 * libvpx decoders and, as a guard, for an FFmpeg decoder that flags its
 * keyframes (MPEG-4 part 2). Another guard plays the alpha clip as a local
 * file. The clips play looped and are read up to their last frame (the
 * hold of the last frame at the end of a clip needs libobs's video). No
 * display or graphics are needed. The alpha cases skip when FFmpeg has no
 * libvpx.
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
#include <libavutil/dict.h>
#include <libavutil/opt.h>

#include <obs.h>
#include <util/platform.h>
#include <util/threading.h>

#include <media-playback/media-playback.h>

#define W 64
#define H 64
#define FPS 30
#define GOP 30
#define CLIP_FRAMES (FPS * 3)
#define LATE_START 10 /* the late clips start here, between keyframes */

/* luma code of the frame number: the left block holds n % 30, the right one
 * n / 30, in steps the encoder keeps apart */
#define Y_BASE 16
#define LEFT_STEP 7
#define RIGHT_STEP 20

enum clip { VP9_ALPHA, VP8_ALPHA, VP9_ALPHA_LATE, VP8_ALPHA_LATE, MPEG4_LATE, CLIP_COUNT };

struct clip_desc {
	const char *encoder;
	const char *name;
	bool alpha;
	int first_packet;
};

static const struct clip_desc clips[CLIP_COUNT] = {
	[VP9_ALPHA] = {"libvpx-vp9", "vp9-alpha.webm", true, 0},
	[VP8_ALPHA] = {"libvpx", "vp8-alpha.webm", true, 0},
	[VP9_ALPHA_LATE] = {"libvpx-vp9", "vp9-alpha-late.webm", true, LATE_START},
	[VP8_ALPHA_LATE] = {"libvpx", "vp8-alpha-late.webm", true, LATE_START},
	[MPEG4_LATE] = {"mpeg4", "mpeg4-late.mkv", false, LATE_START},
};

static char paths[CLIP_COUNT][512];
static bool written[CLIP_COUNT];

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
	if (f->format == AV_PIX_FMT_YUVA420P) {
		/* left half opaque, right half transparent */
		for (int y = 0; y < H; y++) {
			memset(f->data[3] + y * f->linesize[3], 255, W / 2);
			memset(f->data[3] + y * f->linesize[3] + W / 2, 0, W / 2);
		}
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

/* Packets before first_packet are dropped, so a late clip starts with
 * frames that depend on a keyframe it does not contain. */
static bool write_packets(AVFormatContext *oc, AVCodecContext *enc, AVStream *st, AVPacket *pkt, int *count,
			  int first_packet)
{
	int ret;
	while ((ret = avcodec_receive_packet(enc, pkt)) >= 0) {
		if ((*count)++ < first_packet) {
			av_packet_unref(pkt);
			continue;
		}
		pkt->stream_index = st->index;
		av_packet_rescale_ts(pkt, enc->time_base, st->time_base);
		if (av_interleaved_write_frame(oc, pkt) < 0)
			return false;
	}
	return ret == AVERROR(EAGAIN) || ret == AVERROR_EOF;
}

static bool write_clip(const char *path, const struct clip_desc *desc)
{
	AVFormatContext *oc = NULL;
	AVCodecContext *venc = NULL;
	AVStream *vs = NULL;
	AVPacket *pkt = av_packet_alloc();
	AVFrame *vf = av_frame_alloc();
	bool ok = false;
	int count = 0;
	enum AVPixelFormat format = desc->alpha ? AV_PIX_FMT_YUVA420P : AV_PIX_FMT_YUV420P;

	if (!pkt || !vf || avformat_alloc_output_context2(&oc, NULL, desc->alpha ? "webm" : "matroska", path) < 0)
		goto done;

	const AVCodec *vcodec = avcodec_find_encoder_by_name(desc->encoder);
	if (!vcodec || !(venc = avcodec_alloc_context3(vcodec)))
		goto done;
	venc->width = W;
	venc->height = H;
	venc->pix_fmt = format;
	venc->time_base = (AVRational){1, FPS};
	venc->framerate = (AVRational){FPS, 1};
	venc->gop_size = GOP;
	venc->keyint_min = GOP;
	venc->max_b_frames = 0;
	if (strcmp(desc->encoder, "mpeg4") == 0) {
		venc->flags |= AV_CODEC_FLAG_QSCALE;
		venc->global_quality = FF_QP2LAMBDA * 2;
	} else {
		/* every frame decodable on its own timestamp: no hidden
		 * alt-ref frames, no lookahead; keyframes only every GOP */
		venc->bit_rate = 2000000;
		venc->qmin = 0;
		venc->qmax = 4;
		av_opt_set(venc->priv_data, "deadline", "realtime", 0);
		av_opt_set_int(venc->priv_data, "cpu-used", 8, 0);
		av_opt_set_int(venc->priv_data, "lag-in-frames", 0, 0);
		av_opt_set_int(venc->priv_data, "auto-alt-ref", 0, 0);
		if (strcmp(desc->encoder, "libvpx-vp9") == 0)
			av_opt_set_int(venc->priv_data, "lossless", 1, 0);
	}
	if (oc->oformat->flags & AVFMT_GLOBALHEADER)
		venc->flags |= AV_CODEC_FLAG_GLOBAL_HEADER;
	if (avcodec_open2(venc, vcodec, NULL) < 0)
		goto done;
	if (!(vs = avformat_new_stream(oc, NULL)) || avcodec_parameters_from_context(vs->codecpar, venc) < 0)
		goto done;
	vs->time_base = venc->time_base;
	if (desc->alpha)
		av_dict_set(&vs->metadata, "alpha_mode", "1", 0);

	if (avio_open(&oc->pb, path, AVIO_FLAG_WRITE) < 0)
		goto done;
	if (avformat_write_header(oc, NULL) < 0)
		goto done_io;

	vf->width = W;
	vf->height = H;
	vf->format = format;
	if (av_frame_get_buffer(vf, 0) < 0)
		goto done_io;

	for (int n = 0; n < CLIP_FRAMES; n++) {
		if (av_frame_make_writable(vf) < 0)
			goto done_io;
		draw_frame(vf, n);
		vf->pts = n;
		if (avcodec_send_frame(venc, vf) < 0 || !write_packets(oc, venc, vs, pkt, &count, desc->first_packet))
			goto done_io;
	}
	if (avcodec_send_frame(venc, NULL) < 0 || !write_packets(oc, venc, vs, pkt, &count, desc->first_packet))
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

/* What the demuxer sees: the alpha tag (what makes decode.c pick libvpx)
 * and the keyframe flag of the first packet. */
static bool check_clip(const char *path, const struct clip_desc *desc)
{
	AVFormatContext *ic = NULL;
	AVPacket *pkt = av_packet_alloc();
	bool ok = false;

	if (!pkt || avformat_open_input(&ic, path, NULL, NULL) < 0)
		goto done;
	if (avformat_find_stream_info(ic, NULL) < 0 || ic->nb_streams != 1)
		goto done;
	const AVDictionaryEntry *tag = av_dict_get(ic->streams[0]->metadata, "alpha_mode", NULL, 0);
	bool alpha = tag && strcmp(tag->value, "1") == 0;
	if (av_read_frame(ic, pkt) < 0)
		goto done;
	bool first_key = (pkt->flags & AV_PKT_FLAG_KEY) != 0;
	fprintf(stderr, "%s: alpha_mode %d, first packet %s\n", desc->name, alpha, first_key ? "key" : "not key");
	ok = alpha == desc->alpha && first_key == (desc->first_packet == 0);

done:
	av_packet_free(&pkt);
	avformat_close_input(&ic);
	return ok;
}

static void temp_path(char *buf, size_t size, const char *name)
{
	const char *tmp = getenv("TMPDIR");
	if (!tmp || !*tmp)
		tmp = getenv("TEMP");
	if (!tmp || !*tmp)
		tmp = "/tmp";
	snprintf(buf, size, "%s/obs-media-alpha-network-%llu-%s", tmp, (unsigned long long)os_gettime_ns(), name);
}

static int group_setup(void **state)
{
	UNUSED_PARAMETER(state);
	for (int i = 0; i < CLIP_COUNT; i++) {
		if (!avcodec_find_encoder_by_name(clips[i].encoder)) {
			fprintf(stderr, "FFmpeg has no %s encoder: the %s cases skip\n", clips[i].encoder,
				clips[i].name);
			continue;
		}
		temp_path(paths[i], sizeof(paths[i]), clips[i].name);
		if (!write_clip(paths[i], &clips[i]) || !check_clip(paths[i], &clips[i])) {
			fprintf(stderr, "could not write the test clip %s\n", clips[i].name);
			return -1;
		}
		written[i] = true;
	}
	return 0;
}

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	for (int i = 0; i < CLIP_COUNT; i++) {
		if (written[i])
			os_unlink(paths[i]);
	}
	return 0;
}

/* ------------------------------------------------------------------------ */
/* Player */

struct player {
	media_playback_t *mp;
	pthread_mutex_t mutex;

	int frames;
	int first_frame;
	int last_frame;
	enum video_format format;
	int alpha_left;
	int alpha_right;
	bool done; /* the clip's last frame was shown */
};

static void video_cb(void *opaque, struct obs_source_frame *f)
{
	struct player *p = opaque;
	if (f->width != W || f->height != H)
		return;
	int n = frame_number(f->data[0], f->linesize[0]);

	pthread_mutex_lock(&p->mutex);
	if (p->done) {
		pthread_mutex_unlock(&p->mutex);
		return;
	}
	if (!p->frames++) {
		p->first_frame = n;
		p->format = f->format;
		if (f->format == VIDEO_FORMAT_I40A) {
			const uint8_t *row = f->data[3] + (H / 2) * f->linesize[3];
			p->alpha_left = row[W / 4];
			p->alpha_right = row[3 * W / 4];
		}
	}
	p->last_frame = n;
	p->done = n == CLIP_FRAMES - 1;
	pthread_mutex_unlock(&p->mutex);
}

static void preload_cb(void *opaque, struct obs_source_frame *f)
{
	UNUSED_PARAMETER(opaque);
	UNUSED_PARAMETER(f);
}

static void stop_cb(void *opaque)
{
	UNUSED_PARAMETER(opaque);
}

/* Plays the clip up to its last frame (or for 3 s more than its length). */
static void play(struct player *p, enum clip c, bool local)
{
	memset(p, 0, sizeof(*p));
	pthread_mutex_init(&p->mutex, NULL);
	p->first_frame = -1;
	p->last_frame = -1;
	p->alpha_left = -1;
	p->alpha_right = -1;

	struct mp_media_info info = {
		.opaque = p,
		.v_cb = video_cb,
		.v_preload_cb = preload_cb,
		.stop_cb = stop_cb,
		.path = paths[c],
		.speed = 100,
		.force_range = VIDEO_RANGE_DEFAULT,
		.is_local_file = local,
		/* what ffmpeg_source passes for a URL (2 MB) */
		.buffering = local ? 0 : 2 * 1024 * 1024,
	};
	p->mp = media_playback_create(&info);
	assert_non_null(p->mp);
	media_playback_play(p->mp, true, false);

	for (int i = 0; i < (CLIP_FRAMES * 1000 / FPS + 3000) / 10; i++) {
		pthread_mutex_lock(&p->mutex);
		bool done = p->done;
		pthread_mutex_unlock(&p->mutex);
		if (done)
			break;
		os_sleep_ms(10);
	}
	media_playback_destroy(p->mp);
	pthread_mutex_destroy(&p->mutex);

	fprintf(stderr, "%s %s: %d frames shown, frames %d to %d, format %d, alpha left %d right %d%s\n", clips[c].name,
		local ? "as a local file" : "as a network input", p->frames, p->first_frame, p->last_frame,
		(int)p->format, p->alpha_left, p->alpha_right, p->done ? "" : " (the last frame was not shown)");
}

static void require_clip(enum clip c)
{
	if (!written[c])
		skip();
}

/* The whole clip is shown, from its first frame, with its alpha. */
static void expect_whole_clip(const struct player *p, enum clip c)
{
	if (!p->frames) {
		fprintf(stderr, "%s: no video frame was shown\n", clips[c].name);
		fail();
	}
	if (p->first_frame != 0 || p->last_frame != CLIP_FRAMES - 1 || p->frames < CLIP_FRAMES - 3) {
		fprintf(stderr, "%s: showed %d frames, %d to %d; the clip has %d, 0 to %d\n", clips[c].name, p->frames,
			p->first_frame, p->last_frame, CLIP_FRAMES, CLIP_FRAMES - 1);
		fail();
	}
	if (p->format != VIDEO_FORMAT_I40A || p->alpha_left < 235 || p->alpha_right > 20) {
		fprintf(stderr, "%s: format %d, alpha %d (opaque half) and %d (transparent half)\n", clips[c].name,
			(int)p->format, p->alpha_left, p->alpha_right);
		fail();
	}
}

/* A clip that starts between keyframes shows nothing before its first
 * keyframe (frame GOP), then shows the rest. */
static void expect_from_first_keyframe(const struct player *p, enum clip c)
{
	if (p->frames && p->first_frame != GOP) {
		fprintf(stderr, "%s: the first frame shown is %d, before the clip's first keyframe (frame %d)\n",
			clips[c].name, p->first_frame, GOP);
		fail();
	}
	if (!p->frames || p->last_frame != CLIP_FRAMES - 1) {
		fprintf(stderr, "%s: showed %d frames, up to frame %d; expected frames %d to %d\n", clips[c].name,
			p->frames, p->last_frame, GOP, CLIP_FRAMES - 1);
		fail();
	}
}

static void vp9_alpha_network_shows_video(void **state)
{
	UNUSED_PARAMETER(state);
	require_clip(VP9_ALPHA);
	struct player p;
	play(&p, VP9_ALPHA, false);
	expect_whole_clip(&p, VP9_ALPHA);
}

static void vp8_alpha_network_shows_video(void **state)
{
	UNUSED_PARAMETER(state);
	require_clip(VP8_ALPHA);
	struct player p;
	play(&p, VP8_ALPHA, false);
	expect_whole_clip(&p, VP8_ALPHA);
}

static void vp9_alpha_network_starts_at_first_keyframe(void **state)
{
	UNUSED_PARAMETER(state);
	require_clip(VP9_ALPHA_LATE);
	struct player p;
	play(&p, VP9_ALPHA_LATE, false);
	expect_from_first_keyframe(&p, VP9_ALPHA_LATE);
}

static void vp8_alpha_network_starts_at_first_keyframe(void **state)
{
	UNUSED_PARAMETER(state);
	require_clip(VP8_ALPHA_LATE);
	struct player p;
	play(&p, VP8_ALPHA_LATE, false);
	expect_from_first_keyframe(&p, VP8_ALPHA_LATE);
}

/* Guards. */
static void vp9_alpha_local_shows_video(void **state)
{
	UNUSED_PARAMETER(state);
	require_clip(VP9_ALPHA);
	struct player p;
	play(&p, VP9_ALPHA, true);
	expect_whole_clip(&p, VP9_ALPHA);
}

/* An FFmpeg decoder that flags its keyframes: the gate still drops the
 * frames before the first keyframe. */
static void mpeg4_network_starts_at_first_keyframe(void **state)
{
	UNUSED_PARAMETER(state);
	require_clip(MPEG4_LATE);
	struct player p;
	play(&p, MPEG4_LATE, false);
	expect_from_first_keyframe(&p, MPEG4_LATE);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(vp9_alpha_network_shows_video),
		cmocka_unit_test(vp8_alpha_network_shows_video),
		cmocka_unit_test(vp9_alpha_network_starts_at_first_keyframe),
		cmocka_unit_test(vp8_alpha_network_starts_at_first_keyframe),
		cmocka_unit_test(vp9_alpha_local_shows_video),
		cmocka_unit_test(mpeg4_network_starts_at_first_keyframe),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
