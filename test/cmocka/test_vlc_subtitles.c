/*
 * VLC Video Source must not show subtitles while "Enable subtitles" is off
 * (upstream obsproject/obs-studio#14199).
 *
 * With the setting off, vlcs_update() adds no subtitle option to the media,
 * so libVLC selects subtitle tracks the way it does by default: a track
 * flagged Default in the container, or a subtitle file next to the video,
 * is shown even though the user turned subtitles off.
 *
 * The test writes an AVI file with black uncompressed video and a subtitle
 * file of the same name next to it (libVLC loads it automatically), plays it
 * through the real vlc_source module and measures the contrast of the
 * program output: the video is one flat colour, so contrast is subtitle
 * text.
 *
 * The cases need core video (run under xvfb-run) and skip without a display
 * or when libVLC is not installed; the test then exits with 77 so ctest
 * reports it as skipped.
 */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <obs.h>
#include <obs-module.h>
#include <util/platform.h>
#include <util/threading.h>

#define WIDTH 320
#define HEIGHT 240
#define FPS 10
#define FILE_SECONDS 20

/* ------------------------------------------------------------------------ */
/* AVI file with one uncompressed (BI_RGB, 24 bit) black video stream         */

static void put_u16(FILE *f, uint16_t v)
{
	fwrite(&v, 2, 1, f);
}

static void put_u32(FILE *f, uint32_t v)
{
	fwrite(&v, 4, 1, f);
}

static void put_fourcc(FILE *f, const char *cc)
{
	fwrite(cc, 1, 4, f);
}

static bool write_avi(const char *path)
{
	FILE *f = fopen(path, "wb");
	if (!f)
		return false;

	const uint32_t frames = FPS * FILE_SECONDS;
	const uint32_t frame_bytes = WIDTH * HEIGHT * 3;

	const uint32_t strl_size = 4 + (8 + 56) + (8 + 40);
	const uint32_t hdrl_size = 4 + (8 + 56) + (8 + strl_size);
	const uint32_t movi_size = 4 + frames * (8 + frame_bytes);
	const uint32_t idx1_size = frames * 16;
	const uint32_t riff_size = 4 + (8 + hdrl_size) + (8 + movi_size) + (8 + idx1_size);

	put_fourcc(f, "RIFF");
	put_u32(f, riff_size);
	put_fourcc(f, "AVI ");

	put_fourcc(f, "LIST");
	put_u32(f, hdrl_size);
	put_fourcc(f, "hdrl");

	put_fourcc(f, "avih");
	put_u32(f, 56);
	put_u32(f, 1000000 / FPS);     /* microseconds per frame */
	put_u32(f, frame_bytes * FPS); /* max bytes per second */
	put_u32(f, 0);                 /* padding granularity */
	put_u32(f, 0x10);              /* AVIF_HASINDEX */
	put_u32(f, frames);            /* total frames */
	put_u32(f, 0);                 /* initial frames */
	put_u32(f, 1);                 /* streams */
	put_u32(f, frame_bytes);       /* suggested buffer size */
	put_u32(f, WIDTH);
	put_u32(f, HEIGHT);
	for (int i = 0; i < 4; i++)
		put_u32(f, 0);

	put_fourcc(f, "LIST");
	put_u32(f, strl_size);
	put_fourcc(f, "strl");

	put_fourcc(f, "strh");
	put_u32(f, 56);
	put_fourcc(f, "vids");
	put_u32(f, 0); /* handler: BI_RGB */
	put_u32(f, 0); /* flags */
	put_u16(f, 0); /* priority */
	put_u16(f, 0); /* language */
	put_u32(f, 0); /* initial frames */
	put_u32(f, 1); /* scale */
	put_u32(f, FPS);
	put_u32(f, 0);      /* start */
	put_u32(f, frames); /* length */
	put_u32(f, frame_bytes);
	put_u32(f, 0xFFFFFFFF); /* quality */
	put_u32(f, 0);          /* sample size */
	put_u16(f, 0);          /* rcFrame */
	put_u16(f, 0);
	put_u16(f, WIDTH);
	put_u16(f, HEIGHT);

	put_fourcc(f, "strf"); /* BITMAPINFOHEADER */
	put_u32(f, 40);
	put_u32(f, 40);
	put_u32(f, WIDTH);
	put_u32(f, HEIGHT);
	put_u16(f, 1);  /* planes */
	put_u16(f, 24); /* bits per pixel */
	put_u32(f, 0);  /* BI_RGB */
	put_u32(f, frame_bytes);
	put_u32(f, 0);
	put_u32(f, 0);
	put_u32(f, 0);
	put_u32(f, 0);

	put_fourcc(f, "LIST");
	put_u32(f, movi_size);
	put_fourcc(f, "movi");

	uint8_t *black = calloc(1, frame_bytes);
	for (uint32_t i = 0; i < frames; i++) {
		put_fourcc(f, "00db");
		put_u32(f, frame_bytes);
		fwrite(black, 1, frame_bytes, f);
	}
	free(black);

	/* offsets are relative to the "movi" fourcc */
	put_fourcc(f, "idx1");
	put_u32(f, idx1_size);
	uint32_t offset = 4;
	for (uint32_t i = 0; i < frames; i++) {
		put_fourcc(f, "00db");
		put_u32(f, 0x10); /* AVIIF_KEYFRAME */
		put_u32(f, offset);
		put_u32(f, frame_bytes);
		offset += 8 + frame_bytes;
	}

	bool ok = ferror(f) == 0;
	return fclose(f) == 0 && ok;
}

/* Subtitle shown for the whole file. */
static bool write_srt(const char *path)
{
	FILE *f = fopen(path, "w");
	if (!f)
		return false;
	fprintf(f, "1\n00:00:00,000 --> 00:01:00,000\nWWWW MMMM WWWW\n\n");
	bool ok = ferror(f) == 0;
	return fclose(f) == 0 && ok;
}

/* ------------------------------------------------------------------------ */
/* Program output                                                           */

struct capture {
	pthread_mutex_t mutex;
	uint64_t frames;       /* frames since the last reset */
	uint32_t max_contrast; /* highest contrast of one of them */
};

#define BLOCK 20

/* Contrast of the output frame's luma: the average of its brightest
 * BLOCK x BLOCK block minus that of its darkest. The video is one flat
 * colour, so it has none; subtitle text raises the blocks it covers. */
static uint32_t frame_contrast(const struct video_data *frame)
{
	uint32_t lo = UINT32_MAX;
	uint32_t hi = 0;
	for (uint32_t by = 0; by + BLOCK <= HEIGHT; by += BLOCK) {
		for (uint32_t bx = 0; bx + BLOCK <= WIDTH; bx += BLOCK) {
			uint32_t sum = 0;
			for (uint32_t y = by; y < by + BLOCK; y++) {
				const uint8_t *row = frame->data[0] + (size_t)y * frame->linesize[0];
				for (uint32_t x = bx; x < bx + BLOCK; x++)
					sum += row[x];
			}
			uint32_t avg = sum / (BLOCK * BLOCK);
			if (avg < lo)
				lo = avg;
			if (avg > hi)
				hi = avg;
		}
	}
	return hi - lo;
}

static void video_cb(void *param, struct video_data *frame)
{
	struct capture *cap = param;
	uint32_t contrast = frame_contrast(frame);

	pthread_mutex_lock(&cap->mutex);
	cap->frames++;
	if (contrast > cap->max_contrast)
		cap->max_contrast = contrast;
	pthread_mutex_unlock(&cap->mutex);
}

/* ------------------------------------------------------------------------ */
/* libobs setup                                                             */

/* libobs is up (a display is available) */
static bool obs_up;
/* ... and the module registered vlc_source (libVLC is installed) */
static bool vlc_ready;

static int start_obs(void **state)
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
		.base_width = WIDTH,
		.base_height = HEIGHT,
		.output_width = WIDTH,
		.output_height = HEIGHT,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	if (obs_reset_video(&ovi) != OBS_VIDEO_SUCCESS)
		return 0;

	struct obs_audio_info oai = {.samples_per_sec = 48000, .speakers = SPEAKERS_STEREO};
	if (!obs_reset_audio(&oai))
		return 0;

	obs_up = true;

	obs_module_t *mod = NULL;
	if (obs_open_module(&mod, VLC_MODULE_PATH, VLC_DATA_PATH) != MODULE_SUCCESS || !obs_init_module(mod))
		return 0;

	/* the module only registers vlc_source when libVLC is installed */
	vlc_ready = obs_source_get_display_name("vlc_source") != NULL;
	return 0;
}

static int stop_obs(void **state)
{
	UNUSED_PARAMETER(state);
	if (obs_initialized())
		obs_shutdown();
	return 0;
}

/* ------------------------------------------------------------------------ */
/* Playing the file                                                          */

#define SETTLE_MS 2000
#define MEASURE_MS 1500

/* Plays the file with "Enable subtitles" set to each of `enable` in turn and
 * stores the highest contrast of one output frame for each step in
 * `contrast`. Without `with_srt` the file has no subtitle. */
static void play(const bool *enable, uint32_t *contrast, size_t num, bool with_srt)
{
	const char *tmp = getenv("TMPDIR") ? getenv("TMPDIR") : "/tmp";
	unsigned long long id = (unsigned long long)os_gettime_ns();
	char avi[512];
	char srt[512];
	snprintf(avi, sizeof(avi), "%s/obs-vlc-sub-%llu.avi", tmp, id);
	snprintf(srt, sizeof(srt), "%s/obs-vlc-sub-%llu.srt", tmp, id);
	if (!write_avi(avi) || (with_srt && !write_srt(srt))) {
		fprintf(stderr, "could not write %s / %s\n", avi, srt);
		os_unlink(avi);
		os_unlink(srt);
		fail();
	}

	obs_data_t *settings = obs_data_create();
	obs_data_array_t *list = obs_data_array_create();
	obs_data_t *item = obs_data_create();
	obs_data_set_string(item, "value", avi);
	obs_data_array_push_back(list, item);
	obs_data_release(item);
	obs_data_set_array(settings, "playlist", list);
	obs_data_array_release(list);
	obs_data_set_string(settings, "playback_behavior", "always_play");
	obs_data_set_bool(settings, "loop", false);
	obs_data_set_int(settings, "subtitle", 1);
	obs_data_set_bool(settings, "subtitle_enable", enable[0]);

	struct capture cap;
	memset(&cap, 0, sizeof(cap));
	pthread_mutex_init(&cap.mutex, NULL);
	obs_add_raw_video_callback(NULL, video_cb, &cap);

	obs_source_t *src = obs_source_create("vlc_source", "vlc", settings, NULL);
	if (!src) {
		obs_data_release(settings);
		obs_remove_raw_video_callback(video_cb, &cap);
		pthread_mutex_destroy(&cap.mutex);
		os_unlink(avi);
		os_unlink(srt);
		fprintf(stderr, "could not create vlc_source\n");
		fail();
	}
	obs_set_output_source(0, src);

	for (size_t i = 0; i < num; i++) {
		if (i > 0) {
			obs_data_set_bool(settings, "subtitle_enable", enable[i]);
			obs_source_update(src, settings);
		}
		os_sleep_ms(SETTLE_MS);

		pthread_mutex_lock(&cap.mutex);
		cap.frames = 0;
		cap.max_contrast = 0;
		pthread_mutex_unlock(&cap.mutex);

		os_sleep_ms(MEASURE_MS);

		pthread_mutex_lock(&cap.mutex);
		uint64_t frames = cap.frames;
		contrast[i] = cap.max_contrast;
		pthread_mutex_unlock(&cap.mutex);

		fprintf(stderr, "step %zu: subtitles %s, %llu frames, contrast %u\n", i + 1,
			enable[i] ? "enabled" : "disabled", (unsigned long long)frames, contrast[i]);
		assert_true(frames > 0);
	}

	obs_set_output_source(0, NULL);
	obs_remove_raw_video_callback(video_cb, &cap);
	obs_source_release(src);
	obs_data_release(settings);
	pthread_mutex_destroy(&cap.mutex);
	os_unlink(avi);
	os_unlink(srt);
}

/* Subtitle text raises the blocks it covers by well over this (about 40 in
 * the Tier 2 container); the flat video alone stays near 0. */
#define TEXT_CONTRAST 15

/* ------------------------------------------------------------------------ */
/* Cases                                                                    */

/* Guard: without a subtitle the output stays flat, so contrast is
 * subtitle text and nothing else. */
static void video_alone_is_flat(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip(); /* no display */
	if (!vlc_ready)
		skip(); /* libVLC is not installed */

	static const bool enable[] = {true};
	uint32_t contrast[1];
	play(enable, contrast, 1, false);
	if (contrast[0] >= TEXT_CONTRAST) {
		fprintf(stderr, "the video without a subtitle is not flat\n");
		fail();
	}
}

/* Guard: with "Enable subtitles" on, the subtitle is shown, so the
 * measurement can see subtitle text at all. */
static void enabled_subtitles_show(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	if (!vlc_ready)
		skip();

	static const bool enable[] = {true};
	uint32_t contrast[1];
	play(enable, contrast, 1, true);
	if (contrast[0] < TEXT_CONTRAST) {
		fprintf(stderr, "subtitles are enabled but no subtitle text was rendered\n");
		fail();
	}
}

/* The reported bug: "Enable subtitles" is off, the subtitle still shows. */
static void disabled_subtitles_hidden(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	if (!vlc_ready)
		skip();

	static const bool enable[] = {false};
	uint32_t contrast[1];
	play(enable, contrast, 1, true);
	if (contrast[0] >= TEXT_CONTRAST) {
		fprintf(stderr, "subtitles are disabled but subtitle text was rendered\n");
		fail();
	}
}

/* Turning the setting off on a playing source hides them, turning it on
 * again shows them. */
static void toggling_subtitles_follows_setting(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	if (!vlc_ready)
		skip();

	static const bool enable[] = {true, false, true};
	uint32_t contrast[3];
	play(enable, contrast, 3, true);
	bool ok = true;
	for (size_t i = 0; i < 3; i++) {
		if ((contrast[i] >= TEXT_CONTRAST) != enable[i]) {
			fprintf(stderr, "step %zu: subtitles %s, but text was %srendered\n", i + 1,
				enable[i] ? "enabled" : "disabled", contrast[i] >= TEXT_CONTRAST ? "" : "not ");
			ok = false;
		}
	}
	if (!ok)
		fail();
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(video_alone_is_flat),
		cmocka_unit_test(enabled_subtitles_show),
		cmocka_unit_test(disabled_subtitles_hidden),
		cmocka_unit_test(toggling_subtitles_follows_setting),
	};

	int failed = cmocka_run_group_tests(tests, start_obs, stop_obs);
	if (failed)
		return failed;
	return vlc_ready ? 0 : 77;
}
