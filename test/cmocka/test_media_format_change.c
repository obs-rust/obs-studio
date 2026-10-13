/*
 * A Media Source must follow a stream whose frame size or pixel format
 * changes in the middle of the file (upstream obsproject/obs-studio PR #5010
 * by hellowanda, confirmed by WizardCM, derrod and tt2468): media-playback
 * built its scaler (SwsContext and output buffer) from the first frame that
 * needed conversion and never rebuilt it. When the frames got bigger,
 * sws_scale refused every one of them ("Slice parameters 0, 480 are
 * invalid") and the picture froze on the last frame of the first part; when
 * a part in a format the GPU takes directly followed, its frames were still
 * pushed through the old scaler and came out garbled, labelled with the old
 * format.
 *
 * The cases write Matroska files of PNG frames (every PNG frame carries its
 * own size and format) that switch size or format halfway, play them through
 * media-playback (the code behind ffmpeg_source) and check the size, format
 * and colours of the frames handed to the source after the switch. RGB24
 * frames need conversion (to BGRA); RGBA frames are used as they are.
 * Guards: frames that get smaller, and a switch from a direct format to one
 * that needs conversion. No display or graphics are needed.
 */

#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>

#include <obs.h>
#include <util/platform.h>
#include <util/threading.h>

#include <media-playback/media-playback.h>

#define FPS 30
#define PART_FRAMES 15 /* half a second per part */

#define SMALL_W 64
#define SMALL_H 48
#define BIG_W 128
#define BIG_H 96

enum colour { BLACK, RED, GREEN, BLUE, WHITE, OTHER };

static const char *colour_name(enum colour c)
{
	static const char *names[] = {"black", "red", "green", "blue", "white", "other"};
	return names[c];
}

static const uint8_t rgb_of[][3] = {
	[BLACK] = {0, 0, 0},  [RED] = {255, 0, 0},       [GREEN] = {0, 255, 0},
	[BLUE] = {0, 0, 255}, [WHITE] = {255, 255, 255},
};

/* A part of the clip: its size, its PNG pixel format and the colours of its
 * four quadrants (top left, top right, bottom left, bottom right). */
struct part {
	int w, h;
	enum AVPixelFormat fmt;
	enum colour quad[4];
};

#define SOLID(c) {c, c, c, c}
#define QUADS {RED, GREEN, BLUE, WHITE}

/* ------------------------------------------------------------------------ */
/* Test files */

static void draw_part(AVFrame *f, const struct part *p)
{
	int bpp = p->fmt == AV_PIX_FMT_RGBA ? 4 : 3;
	for (int y = 0; y < p->h; y++) {
		uint8_t *row = f->data[0] + (size_t)y * f->linesize[0];
		for (int x = 0; x < p->w; x++) {
			int q = (y >= p->h / 2 ? 2 : 0) + (x >= p->w / 2 ? 1 : 0);
			memcpy(row + x * bpp, rgb_of[p->quad[q]], 3);
			if (bpp == 4)
				row[x * bpp + 3] = 255;
		}
	}
}

static bool encode_part(AVFormatContext *oc, AVStream *st, const struct part *p, int64_t first_pts)
{
	const AVCodec *codec = avcodec_find_encoder(AV_CODEC_ID_PNG);
	if (!codec)
		return false;

	bool ok = false;
	AVCodecContext *enc = avcodec_alloc_context3(codec);
	AVFrame *f = av_frame_alloc();
	AVPacket *pkt = av_packet_alloc();
	if (!enc || !f || !pkt)
		goto done;

	enc->width = p->w;
	enc->height = p->h;
	enc->pix_fmt = p->fmt;
	enc->time_base = (AVRational){1, FPS};
	if (avcodec_open2(enc, codec, NULL) < 0)
		goto done;

	f->format = p->fmt;
	f->width = p->w;
	f->height = p->h;
	if (av_frame_get_buffer(f, 0) < 0)
		goto done;
	draw_part(f, p);

	for (int i = 0; i <= PART_FRAMES; i++) {
		int ret;
		if (i < PART_FRAMES) {
			f->pts = first_pts + i;
			ret = avcodec_send_frame(enc, f);
		} else {
			ret = avcodec_send_frame(enc, NULL);
		}
		if (ret < 0)
			goto done;
		while ((ret = avcodec_receive_packet(enc, pkt)) == 0) {
			pkt->stream_index = st->index;
			pkt->duration = 1;
			av_packet_rescale_ts(pkt, enc->time_base, st->time_base);
			if (av_interleaved_write_frame(oc, pkt) < 0)
				goto done;
		}
		if (ret != AVERROR(EAGAIN) && ret != AVERROR_EOF)
			goto done;
	}
	ok = true;

done:
	av_packet_free(&pkt);
	av_frame_free(&f);
	avcodec_free_context(&enc);
	return ok;
}

/* A Matroska file with one PNG video stream: `first`, then `second`. */
static bool write_clip(const char *path, const struct part *first, const struct part *second)
{
	AVFormatContext *oc = NULL;
	if (avformat_alloc_output_context2(&oc, NULL, "matroska", path) < 0)
		return false;

	bool ok = false;
	AVStream *st = avformat_new_stream(oc, NULL);
	if (!st)
		goto done;
	st->codecpar->codec_type = AVMEDIA_TYPE_VIDEO;
	st->codecpar->codec_id = AV_CODEC_ID_PNG;
	st->codecpar->width = first->w;
	st->codecpar->height = first->h;
	st->codecpar->format = first->fmt;
	st->time_base = (AVRational){1, 1000};
	st->avg_frame_rate = (AVRational){FPS, 1};

	if (avio_open(&oc->pb, path, AVIO_FLAG_WRITE) < 0)
		goto done;
	if (avformat_write_header(oc, NULL) < 0)
		goto done_io;
	if (!encode_part(oc, st, first, 0) || !encode_part(oc, st, second, PART_FRAMES))
		goto done_io;
	ok = av_write_trailer(oc) >= 0;

done_io:
	avio_closep(&oc->pb);
done:
	avformat_free_context(oc);
	return ok;
}

static void temp_path(char *buf, size_t size, const char *name)
{
	const char *tmp = getenv("TMPDIR");
	if (!tmp || !*tmp)
		tmp = getenv("TEMP");
	if (!tmp || !*tmp)
		tmp = "/tmp";
	snprintf(buf, size, "%s/obs-media-format-change-%llu-%s", tmp, (unsigned long long)os_gettime_ns(), name);
}

/* ------------------------------------------------------------------------ */
/* Player */

#define MAX_SEEN 256

struct seen {
	uint32_t w, h;
	enum video_format format;
	enum colour quad[4];
};

struct player {
	media_playback_t *mp;
	pthread_mutex_t mutex;
	struct seen seen[MAX_SEEN];
	size_t count;
};

static enum colour classify(const uint8_t *px, enum video_format format)
{
	uint8_t r, g, b;
	if (format == VIDEO_FORMAT_BGRA || format == VIDEO_FORMAT_BGRX) {
		b = px[0];
		g = px[1];
		r = px[2];
	} else if (format == VIDEO_FORMAT_RGBA) {
		r = px[0];
		g = px[1];
		b = px[2];
	} else {
		return OTHER;
	}
	for (enum colour c = BLACK; c < OTHER; c++) {
		if (abs(r - rgb_of[c][0]) < 16 && abs(g - rgb_of[c][1]) < 16 && abs(b - rgb_of[c][2]) < 16)
			return c;
	}
	return OTHER;
}

static void video_cb(void *opaque, struct obs_source_frame *f)
{
	struct player *p = opaque;
	struct seen s = {.w = f->width, .h = f->height, .format = f->format};

	for (int q = 0; q < 4; q++) {
		/* odd columns: an RGBA row read as RGB24 lands on an RGBA
		 * pixel boundary only every fourth pixel */
		uint32_t x = ((q & 1) ? f->width * 3 / 4 : f->width / 4) + 1;
		uint32_t y = (q & 2) ? f->height * 3 / 4 : f->height / 4;
		const uint8_t *row = f->data[0] + (size_t)y * f->linesize[0];
		if (f->flip)
			row = f->data[0] + (size_t)(f->height - 1 - y) * f->linesize[0];
		s.quad[q] = classify(row + (size_t)x * 4, f->format);
	}

	pthread_mutex_lock(&p->mutex);
	if (p->count < MAX_SEEN)
		p->seen[p->count++] = s;
	pthread_mutex_unlock(&p->mutex);
}

static size_t frames_seen(struct player *p)
{
	pthread_mutex_lock(&p->mutex);
	size_t n = p->count;
	pthread_mutex_unlock(&p->mutex);
	return n;
}

/* Plays the clip in a loop (the end-of-file hold needs a running core, which
 * these cases do without) until frames for both parts have been handed over
 * or 20 s have passed. */
static void play_clip(struct player *p, const char *path, bool full_decode)
{
	memset(p, 0, sizeof(*p));
	pthread_mutex_init(&p->mutex, NULL);

	struct mp_media_info info = {
		.opaque = p,
		.v_cb = video_cb,
		.path = path,
		.speed = 100,
		.force_range = VIDEO_RANGE_DEFAULT,
		.is_local_file = true,
		.full_decode = full_decode,
	};
	p->mp = media_playback_create(&info);
	assert_non_null(p->mp);
	media_playback_play(p->mp, true, false);

	for (int i = 0; i < 2000 && frames_seen(p) < 2 * PART_FRAMES + 2; i++)
		os_sleep_ms(10);

	media_playback_destroy(p->mp);
	pthread_mutex_destroy(&p->mutex);
}

static bool same_seen(const struct seen *a, const struct seen *b)
{
	return a->w == b->w && a->h == b->h && a->format == b->format && !memcmp(a->quad, b->quad, sizeof(a->quad));
}

static const char *format_name(enum video_format f)
{
	return f == VIDEO_FORMAT_BGRA ? "BGRA" : f == VIDEO_FORMAT_RGBA ? "RGBA" : "other";
}

static void print_seen(const struct player *p)
{
	/* runs of identical frames */
	for (size_t i = 0; i < p->count;) {
		size_t j = i;
		while (j < p->count && same_seen(&p->seen[i], &p->seen[j]))
			j++;
		const struct seen *s = &p->seen[i];
		fprintf(stderr, "  %zu x %ux%u %s %s/%s/%s/%s\n", j - i, s->w, s->h, format_name(s->format),
			colour_name(s->quad[0]), colour_name(s->quad[1]), colour_name(s->quad[2]),
			colour_name(s->quad[3]));
		i = j;
	}
}

/* Plays first then second and checks that frames of both parts reach the
 * source as they are in the file. */
static void play_switch(const char *name, const struct part *first, const struct part *second,
			enum video_format first_fmt, enum video_format second_fmt, bool full_decode)
{
	char path[512];
	temp_path(path, sizeof(path), "clip.mkv");
	if (!write_clip(path, first, second)) {
		os_unlink(path);
		fprintf(stderr, "%s: could not write the test clip (no PNG encoder or Matroska muxer?)\n", name);
		skip();
	}

	struct player *p = bzalloc(sizeof(*p));
	play_clip(p, path, full_decode);
	os_unlink(path);

	const struct part *parts[2] = {first, second};
	const enum video_format fmts[2] = {first_fmt, second_fmt};
	size_t got[2] = {0, 0};
	size_t wrong = 0;
	for (size_t i = 0; i < p->count; i++) {
		const struct seen *s = &p->seen[i];
		bool matched = false;
		for (int k = 0; k < 2; k++) {
			struct seen want = {.w = (uint32_t)parts[k]->w, .h = (uint32_t)parts[k]->h, .format = fmts[k]};
			memcpy(want.quad, parts[k]->quad, sizeof(want.quad));
			if (same_seen(s, &want)) {
				got[k]++;
				matched = true;
				break;
			}
		}
		if (!matched)
			wrong++;
	}

	fprintf(stderr, "%s%s: %zu frames, %zu of the first part, %zu of the second, %zu wrong\n", name,
		full_decode ? " (cached)" : "", p->count, got[0], got[1], wrong);
	print_seen(p);
	bfree(p);

	assert_true(got[0] > 0);
	if (got[1] == 0)
		fprintf(stderr, "%s: no frame of the second part (%dx%d) was shown as it is in the file\n", name,
			second->w, second->h);
	assert_true(got[1] > 0);
	if (wrong)
		fprintf(stderr, "%s: %zu frames were shown with the wrong size, format or colours\n", name, wrong);
	assert_int_equal(wrong, 0);
}

static const struct part small_red = {SMALL_W, SMALL_H, AV_PIX_FMT_RGB24, SOLID(RED)};
static const struct part big_blue = {BIG_W, BIG_H, AV_PIX_FMT_RGB24, SOLID(BLUE)};
static const struct part small_quads = {SMALL_W, SMALL_H, AV_PIX_FMT_RGB24, QUADS};
static const struct part small_quads_rgba = {SMALL_W, SMALL_H, AV_PIX_FMT_RGBA, QUADS};
static const struct part small_red_rgba = {SMALL_W, SMALL_H, AV_PIX_FMT_RGBA, SOLID(RED)};

/* RGB24 64x48 -> RGB24 128x96: the bigger frames were refused by the old
 * scaler and the picture froze. */
static void frames_get_bigger(void **state)
{
	UNUSED_PARAMETER(state);
	play_switch("frames_get_bigger", &small_red, &big_blue, VIDEO_FORMAT_BGRA, VIDEO_FORMAT_BGRA, false);
}

static void frames_get_bigger_cached(void **state)
{
	UNUSED_PARAMETER(state);
	play_switch("frames_get_bigger", &small_red, &big_blue, VIDEO_FORMAT_BGRA, VIDEO_FORMAT_BGRA, true);
}

/* RGB24 -> RGBA: the RGBA frames went through the RGB24 scaler and were
 * labelled BGRA. */
static void converted_then_direct_format(void **state)
{
	UNUSED_PARAMETER(state);
	play_switch("converted_then_direct_format", &small_red, &small_quads_rgba, VIDEO_FORMAT_BGRA, VIDEO_FORMAT_RGBA,
		    false);
}

/* Guard: frames that get smaller. */
static void frames_get_smaller(void **state)
{
	UNUSED_PARAMETER(state);
	play_switch("frames_get_smaller", &big_blue, &small_quads, VIDEO_FORMAT_BGRA, VIDEO_FORMAT_BGRA, false);
}

/* A direct format, then one that needs conversion: that part works, but
 * when the clip loops, the direct-format frames go through the scaler built
 * for the second part. */
static void direct_then_converted_format(void **state)
{
	UNUSED_PARAMETER(state);
	play_switch("direct_then_converted_format", &small_red_rgba, &small_quads, VIDEO_FORMAT_RGBA, VIDEO_FORMAT_BGRA,
		    false);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(frames_get_bigger),
		cmocka_unit_test(frames_get_bigger_cached),
		cmocka_unit_test(converted_then_direct_format),
		cmocka_unit_test(frames_get_smaller),
		cmocka_unit_test(direct_then_converted_format),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
