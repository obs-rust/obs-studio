/*
 * A Stinger transition must play its whole clip
 * (upstream obsproject/obs-studio#9755, also #11089): the transition ends
 * after the duration the file's header gives, so a clip whose header says it
 * is shorter than it is, or a clip with no duration in its header (Matroska
 * or WebM written to a pipe or by a live recorder), was cut off and the
 * scene switched to the next one before the clip had finished.
 *
 * The cases write a short clip with libavformat/libavcodec (FFV1 in
 * Matroska): every frame one colour, the last frame another. They run the
 * real Stinger transition (the built obs-transitions module) from scene A
 * to scene B through the real ffmpeg_source and record what the program
 * output shows. The last frame of the clip must appear, and scene B must not
 * appear before it. With "Preload video to RAM" the clip is decoded into
 * memory first, a separate playback path. The clips are checked by demuxing them again, so the
 * header the transition sees is the one the case expects. They need core
 * video, so they run under xvfb-run and skip without a display.
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
#include <libavutil/mathematics.h>

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
#define FPS 10
#define FRAMES 16
#define CLIP_MS (FRAMES * 1000 / FPS)

enum colour { SCENE_A, SCENE_B, BODY, LAST, NOTHING, UNKNOWN };

static const uint8_t rgb[4][3] = {
	[SCENE_A] = {220, 40, 40},
	[SCENE_B] = {40, 220, 40},
	[BODY] = {40, 40, 220},
	[LAST] = {230, 230, 40},
};

static bool obs_up;
static obs_module_t *transitions;
static char good_path[512];
static char short_path[512];
static char none_path[512];

/* ------------------------------------------------------------------------ */
/* clips */

struct pipe_out {
	FILE *f;
};

static int pipe_write(void *opaque, const uint8_t *buf, int size)
{
	struct pipe_out *p = opaque;
	return fwrite(buf, 1, (size_t)size, p->f) == (size_t)size ? size : AVERROR(EIO);
}

/* Writes the clip as FFV1 (BGR0) in Matroska. With `seekable` false the
 * muxer cannot go back to fill in the duration, as when it writes to a pipe,
 * and the file has none. */
static bool write_clip(const char *path, bool seekable)
{
	const AVCodec *codec = avcodec_find_encoder(AV_CODEC_ID_FFV1);
	if (!codec) {
		fprintf(stderr, "no FFV1 encoder\n");
		return false;
	}

	AVFormatContext *oc = NULL;
	if (avformat_alloc_output_context2(&oc, NULL, "matroska", path) < 0)
		return false;

	bool ok = false;
	struct pipe_out out = {0};
	uint8_t *io_buf = NULL;
	AVCodecContext *enc = avcodec_alloc_context3(codec);
	AVFrame *frame = av_frame_alloc();
	AVPacket *pkt = av_packet_alloc();
	AVStream *st = avformat_new_stream(oc, NULL);
	if (!enc || !frame || !pkt || !st)
		goto out;

	enc->width = W;
	enc->height = H;
	enc->pix_fmt = AV_PIX_FMT_BGR0;
	enc->time_base = (AVRational){1, FPS};
	enc->framerate = (AVRational){FPS, 1};
	if (oc->oformat->flags & AVFMT_GLOBALHEADER)
		enc->flags |= AV_CODEC_FLAG_GLOBAL_HEADER;
	if (avcodec_open2(enc, codec, NULL) < 0)
		goto out;
	if (avcodec_parameters_from_context(st->codecpar, enc) < 0)
		goto out;
	st->time_base = enc->time_base;
	st->avg_frame_rate = enc->framerate;

	frame->format = enc->pix_fmt;
	frame->width = W;
	frame->height = H;
	if (av_frame_get_buffer(frame, 0) < 0)
		goto out;

	if (seekable) {
		if (avio_open(&oc->pb, path, AVIO_FLAG_WRITE) < 0)
			goto out;
	} else {
		out.f = fopen(path, "wb");
		io_buf = av_malloc(4096);
		if (!out.f || !io_buf)
			goto out;
		oc->pb = avio_alloc_context(io_buf, 4096, 1, &out, NULL, pipe_write, NULL);
		if (!oc->pb)
			goto out;
		io_buf = NULL;
		oc->pb->seekable = 0;
	}

	if (avformat_write_header(oc, NULL) < 0)
		goto out_io;

	int ret;
	for (int i = 0; i <= FRAMES; i++) {
		if (i < FRAMES) {
			if (av_frame_make_writable(frame) < 0)
				goto out_io;
			const uint8_t *c = rgb[i == FRAMES - 1 ? LAST : BODY];
			for (int y = 0; y < H; y++) {
				uint8_t *p = frame->data[0] + (size_t)y * frame->linesize[0];
				for (int x = 0; x < W; x++) {
					p[x * 4 + 0] = c[2];
					p[x * 4 + 1] = c[1];
					p[x * 4 + 2] = c[0];
					p[x * 4 + 3] = 255;
				}
			}
			frame->pts = i;
			frame->duration = 1;
			ret = avcodec_send_frame(enc, frame);
		} else {
			ret = avcodec_send_frame(enc, NULL);
		}
		if (ret < 0)
			goto out_io;

		while ((ret = avcodec_receive_packet(enc, pkt)) == 0) {
			pkt->duration = 1;
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
	if (seekable) {
		avio_closep(&oc->pb);
	} else if (oc->pb) {
		avio_flush(oc->pb);
		av_freep(&oc->pb->buffer);
		avio_context_free(&oc->pb);
	}
out:
	if (out.f && fclose(out.f) != 0)
		ok = false;
	av_free(io_buf);
	av_packet_free(&pkt);
	av_frame_free(&frame);
	avcodec_free_context(&enc);
	avformat_free_context(oc);
	return ok;
}

static uint32_t ebml_id(const uint8_t *p)
{
	return ((uint32_t)p[0] << 8) | p[1];
}

/* Rewrites the duration in the Matroska Segment Info (an 8-byte float in
 * milliseconds, as FFmpeg writes it) to `ms`. */
static bool set_mkv_duration(const char *path, double ms)
{
	static const uint8_t info_id[4] = {0x15, 0x49, 0xA9, 0x66};
	bool ok = false;
	uint8_t buf[4096];

	FILE *f = fopen(path, "r+b");
	if (!f)
		return false;
	size_t n = fread(buf, 1, sizeof(buf), f);

	for (size_t i = 0; i + 4 <= n && !ok; i++) {
		if (memcmp(buf + i, info_id, 4) != 0)
			continue;
		for (size_t j = i + 4; j + 11 <= n && j < i + 256; j++) {
			if (ebml_id(buf + j) != 0x4489 || buf[j + 2] != 0x88)
				continue;
			union {
				double d;
				uint64_t u;
			} v = {.d = ms};
			uint8_t be[8];
			for (int k = 0; k < 8; k++)
				be[k] = (uint8_t)(v.u >> (56 - 8 * k));
			ok = fseek(f, (long)(j + 3), SEEK_SET) == 0 && fwrite(be, 1, 8, f) == 8;
			break;
		}
	}

	if (fclose(f) != 0)
		ok = false;
	return ok;
}

/* What the demuxer reports for the file: its duration (AV_NOPTS_VALUE when
 * the header has none) and how many video packets it holds. */
static bool probe(const char *path, int64_t *duration, int *packets)
{
	AVFormatContext *fmt = NULL;
	if (avformat_open_input(&fmt, path, NULL, NULL) < 0)
		return false;
	if (avformat_find_stream_info(fmt, NULL) < 0) {
		avformat_close_input(&fmt);
		return false;
	}
	*duration = fmt->duration;
	*packets = 0;

	AVPacket *pkt = av_packet_alloc();
	while (pkt && av_read_frame(fmt, pkt) >= 0) {
		(*packets)++;
		av_packet_unref(pkt);
	}
	av_packet_free(&pkt);
	avformat_close_input(&fmt);
	return true;
}

static void unlink_clips(void)
{
	if (*good_path)
		os_unlink(good_path);
	if (*short_path)
		os_unlink(short_path);
	if (*none_path)
		os_unlink(none_path);
}

static bool make_clips(void)
{
	const char *tmp = getenv("TMPDIR") ? getenv("TMPDIR") : "/tmp";
	unsigned long long id = (unsigned long long)os_gettime_ns();
	snprintf(good_path, sizeof(good_path), "%s/obs-stinger-good-%llu.mkv", tmp, id);
	snprintf(short_path, sizeof(short_path), "%s/obs-stinger-short-%llu.mkv", tmp, id);
	snprintf(none_path, sizeof(none_path), "%s/obs-stinger-none-%llu.mkv", tmp, id);

	if (!write_clip(good_path, true) || !write_clip(short_path, true) || !write_clip(none_path, false)) {
		fprintf(stderr, "could not write the test clips\n");
		return false;
	}
	/* the header says half of what the clip holds */
	if (!set_mkv_duration(short_path, CLIP_MS / 2.0)) {
		fprintf(stderr, "could not rewrite the clip's duration\n");
		return false;
	}

	/* check what the files hold, as the media source will read them */
	int64_t good_dur, short_dur, none_dur;
	int good_n, short_n, none_n;
	if (!probe(good_path, &good_dur, &good_n) || !probe(short_path, &short_dur, &short_n) ||
	    !probe(none_path, &none_dur, &none_n)) {
		fprintf(stderr, "could not read the test clips back\n");
		return false;
	}
	fprintf(stderr, "clips: %d frames, %d ms; header durations: good %lld us, short %lld us, none %s\n", FRAMES,
		CLIP_MS, (long long)good_dur, (long long)short_dur, none_dur == AV_NOPTS_VALUE ? "unset" : "set");

	if (good_n != FRAMES || short_n != FRAMES || none_n != FRAMES) {
		fprintf(stderr, "the clips hold %d/%d/%d frames, not %d\n", good_n, short_n, none_n, FRAMES);
		return false;
	}
	if (llabs(good_dur - (int64_t)CLIP_MS * 1000) > 50000) {
		fprintf(stderr, "the good clip's header says %lld us\n", (long long)good_dur);
		return false;
	}
	if (llabs(short_dur - (int64_t)CLIP_MS * 500) > 50000) {
		fprintf(stderr, "the short clip's header says %lld us\n", (long long)short_dur);
		return false;
	}
	if (none_dur != AV_NOPTS_VALUE) {
		fprintf(stderr, "the clip without a duration has one: %lld us\n", (long long)none_dur);
		return false;
	}
	return true;
}

/* ------------------------------------------------------------------------ */
/* a solid colour source for the two scenes */

struct solid {
	obs_source_t *source;
	uint32_t argb;
};

static const char *solid_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "solid";
}

static void solid_update(void *data, obs_data_t *settings)
{
	struct solid *s = data;
	s->argb = (uint32_t)obs_data_get_int(settings, "color");
}

static void *solid_create(obs_data_t *settings, obs_source_t *source)
{
	struct solid *s = bzalloc(sizeof(*s));
	s->source = source;
	solid_update(s, settings);
	return s;
}

static void solid_destroy(void *data)
{
	bfree(data);
}

static uint32_t solid_size(void *data)
{
	UNUSED_PARAMETER(data);
	return W;
}

static void solid_render(void *data, gs_effect_t *effect)
{
	struct solid *s = data;
	UNUSED_PARAMETER(effect);

	gs_effect_t *solid = obs_get_base_effect(OBS_EFFECT_SOLID);
	gs_eparam_t *color = gs_effect_get_param_by_name(solid, "color");
	struct vec4 c;
	vec4_from_rgba(&c, s->argb);
	gs_effect_set_vec4(color, &c);
	while (gs_effect_loop(solid, "Solid"))
		gs_draw_sprite(NULL, 0, W, H);
}

static struct obs_source_info solid_source = {
	.id = "test_solid",
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_CUSTOM_DRAW,
	.get_name = solid_name,
	.create = solid_create,
	.destroy = solid_destroy,
	.update = solid_update,
	.get_width = solid_size,
	.get_height = solid_size,
	.video_render = solid_render,
};

static obs_scene_t *solid_scene(const char *name, enum colour colour)
{
	obs_data_t *settings = obs_data_create();
	/* 0xAABBGGRR, as color properties store it */
	obs_data_set_int(settings, "color",
			 0xFF000000 | ((uint32_t)rgb[colour][2] << 16) | ((uint32_t)rgb[colour][1] << 8) |
				 rgb[colour][0]);
	obs_source_t *src = obs_source_create("test_solid", name, settings, NULL);
	obs_data_release(settings);

	obs_scene_t *scene = obs_scene_create(name);
	obs_scene_add(scene, src);
	obs_source_release(src);
	return scene;
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
	obs_register_source(&solid_source);

	if (obs_open_module(&transitions, TRANSITIONS_MODULE_PATH, TRANSITIONS_DATA_PATH) != MODULE_SUCCESS ||
	    !obs_init_module(transitions)) {
		fprintf(stderr, "could not load the obs-transitions module\n");
		obs_shutdown();
		return -1;
	}

	if (!make_clips()) {
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

/* ------------------------------------------------------------------------ */
/* running the transition */

#define MAX_SEEN 2048

struct watch {
	pthread_mutex_t mutex;
	size_t count;
	int seen[MAX_SEEN];
};

static bool near(const uint8_t *bgra, const uint8_t c[3])
{
	return abs(bgra[2] - c[0]) < 30 && abs(bgra[1] - c[1]) < 30 && abs(bgra[0] - c[2]) < 30;
}

static int classify(const uint8_t *bgra)
{
	if (bgra[0] < 20 && bgra[1] < 20 && bgra[2] < 20)
		return NOTHING;
	for (int i = SCENE_A; i <= LAST; i++) {
		if (near(bgra, rgb[i]))
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

/* Runs the stinger with `path` from scene A to scene B and records every
 * program output frame from the start of the transition until a while after
 * it has ended. */
static void run_stinger(const char *path, bool preload, struct watch *w)
{
	obs_scene_t *a = solid_scene("scene A", SCENE_A);
	obs_scene_t *b = solid_scene("scene B", SCENE_B);

	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, "path", path);
	obs_data_set_int(settings, "tp_type", 0);
	obs_data_set_int(settings, "transition_point", 300);
	obs_data_set_bool(settings, "preload", preload);
	obs_source_t *stinger = obs_source_create_private("obs_stinger_transition", "stinger", settings);
	obs_data_release(settings);
	if (!stinger) {
		fprintf(stderr, "could not create the stinger transition\n");
		fail();
	}

	obs_transition_set(stinger, obs_scene_get_source(a));
	obs_set_output_source(0, stinger);

	/* let the stinger open its clip (and decode it all with preload) */
	os_sleep_ms(preload ? 1500 : 700);

	memset(w, 0, sizeof(*w));
	pthread_mutex_init(&w->mutex, NULL);
	obs_add_raw_video_callback(NULL, raw_video, w);

	if (!obs_transition_start(stinger, OBS_TRANSITION_MODE_AUTO, 300, obs_scene_get_source(b))) {
		fprintf(stderr, "the transition did not start\n");
		fail();
	}

	/* long enough for the whole clip, a slow start and the end */
	os_sleep_ms(CLIP_MS + 2000);

	obs_remove_raw_video_callback(raw_video, w);
	obs_set_output_source(0, NULL);
	obs_source_release(stinger);
	obs_source_remove(obs_scene_get_source(a));
	obs_source_remove(obs_scene_get_source(b));
	obs_scene_release(a);
	obs_scene_release(b);
	pthread_mutex_destroy(&w->mutex);
}

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
	static const char *names[] = {"A", "B", "clip", "last", "nothing", "?"};
	fprintf(stderr, "%s:", what);
	for (size_t i = 0; i < n; i++)
		fprintf(stderr, " %sx%zu", names[runs[i].what], runs[i].frames);
	fprintf(stderr, "\n");
}

/* The clip must play to its last frame, scene B must not show before that,
 * and the transition must end on scene B. */
static void check(const char *what, const struct watch *w)
{
	static struct run runs[MAX_SEEN];
	size_t n = collapse(w, runs, MAX_SEEN);
	log_runs(what, runs, n);

	size_t body = 0, last = 0;
	bool b_before_last = false;
	for (size_t i = 0; i < w->count; i++) {
		if (w->seen[i] == BODY)
			body++;
		else if (w->seen[i] == LAST)
			last++;
		else if (w->seen[i] == SCENE_B && !last)
			b_before_last = true;
	}

	if (!body) {
		fprintf(stderr, "%s: the stinger clip was never shown\n", what);
		fail();
	}
	if (!last) {
		fprintf(stderr,
			"%s: the transition ended after %zu output frames of the clip, before its last frame "
			"(the clip is %d ms, %d output frames)\n",
			what, body, CLIP_MS, CLIP_MS * 30 / 1000);
		fail();
	}
	if (b_before_last) {
		fprintf(stderr, "%s: scene B showed before the clip's last frame\n", what);
		fail();
	}
	if (w->count == 0 || w->seen[w->count - 1] != SCENE_B) {
		fprintf(stderr, "%s: the transition did not end on scene B\n", what);
		fail();
	}
}

/* The report: the header says the clip is half as long as it is. */
static void short_header_plays_whole_clip(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	static struct watch w;
	run_stinger(short_path, false, &w);
	check("short header", &w);
}

/* A clip whose header has no duration at all (Matroska or WebM written to a
 * pipe or by a live recorder). */
static void no_duration_plays_whole_clip(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	static struct watch w;
	run_stinger(none_path, false, &w);
	check("no duration", &w);
}

/* The same with "Preload video to RAM". */
static void short_header_preloaded_plays_whole_clip(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	static struct watch w;
	run_stinger(short_path, true, &w);
	check("short header, preloaded", &w);
}

/* Guard: a clip with a correct header plays whole and ends on scene B. */
static void good_header_plays_whole_clip(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	static struct watch w;
	run_stinger(good_path, false, &w);
	check("good header", &w);
}

/* A clip with a correct header and "Preload video to RAM": the preloaded
 * clip stopped at its last frame without showing it (the cached path missed
 * the fix for upstream obsproject/obs-studio#10425). */
static void good_header_preloaded_plays_whole_clip(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	static struct watch w;
	run_stinger(good_path, true, &w);
	check("good header, preloaded", &w);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(short_header_plays_whole_clip),
		cmocka_unit_test(no_duration_plays_whole_clip),
		cmocka_unit_test(short_header_preloaded_plays_whole_clip),
		cmocka_unit_test(good_header_plays_whole_clip),
		cmocka_unit_test(good_header_preloaded_plays_whole_clip),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
