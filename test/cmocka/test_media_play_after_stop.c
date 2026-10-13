/*
 * A stopped or ended Media Source must play again when it is told to play
 * (upstream obsproject/obs-websocket#1046, obsproject/obs-studio#4252).
 *
 * obs_source_media_stop() stops the player thread, and
 * obs_source_media_play_pause(source, false) only unpauses a player that is
 * still running, so after Stop (or after the file ended) Play did nothing:
 * the source reported OBS_MEDIA_STATE_PLAYING and emitted media_started, but
 * its time stood still and it showed no picture. obs-websocket's
 * TriggerMediaInputAction PLAY and the "Play" hotkey both go through
 * obs_source_media_play_pause().
 *
 * When Stop and Play come at once (a script, or a hotkey pressed twice), the
 * player thread handles the stop only after playback has started again and
 * reports it as the end of the file: the source plays but says it ended.
 *
 * The test plays a generated clip through the real ffmpeg_source in the
 * program scene and checks what a user sees: the media time and the frames
 * libobs outputs. It needs core video, so it runs under xvfb-run and skips
 * without a display.
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

/* obs-ffmpeg-source.c is compiled into this test; it only needs strings. */
const char *obs_module_text(const char *val)
{
	return val;
}

extern struct obs_source_info ffmpeg_source;

#define W 64
#define H 64
#define FPS 10
#define CLIP_FRAMES 40 /* 4 s */
#define CLIP_MS (CLIP_FRAMES * 1000 / FPS)

/* Every clip frame is bright; the output after a stop is black. */
#define Y_FRAME 200
#define Y_DARK_MAX 40

static bool obs_up;
static char clip_path[512];

static bool write_clip(const char *path)
{
	FILE *f = fopen(path, "wb");
	if (!f)
		return false;

	fprintf(f, "YUV4MPEG2 W%d H%d F%d:1 Ip A1:1 C420jpeg\n", W, H, FPS);

	static uint8_t y[W * H], uv[(W / 2) * (H / 2)];
	memset(y, Y_FRAME, sizeof(y));
	memset(uv, 128, sizeof(uv));
	for (int i = 0; i < CLIP_FRAMES; i++) {
		fputs("FRAME\n", f);
		fwrite(y, 1, sizeof(y), f);
		fwrite(uv, 1, sizeof(uv), f);
		fwrite(uv, 1, sizeof(uv), f);
	}

	fclose(f);
	return true;
}

struct watch {
	pthread_mutex_t mutex;
	uint8_t luma;
	uint64_t frames;
};

static void raw_video(void *param, struct video_data *frame)
{
	struct watch *w = param;

	pthread_mutex_lock(&w->mutex);
	w->luma = frame->data[0][(H / 2) * frame->linesize[0] + W / 2];
	w->frames++;
	pthread_mutex_unlock(&w->mutex);
}

static bool showing_picture(struct watch *w)
{
	pthread_mutex_lock(&w->mutex);
	bool bright = w->luma > Y_DARK_MAX;
	pthread_mutex_unlock(&w->mutex);
	return bright;
}

struct player {
	bool started;
	obs_source_t *media;
	obs_scene_t *scene;
	struct watch w;
};

/* One player per case. The per-test teardown releases it, also when a case
 * fails half way: a source left behind would keep calling into freed memory. */
static struct player cur;

static void player_start(struct player *p)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_bool(settings, "is_local_file", true);
	obs_data_set_string(settings, "local_file", clip_path);
	obs_data_set_bool(settings, "looping", false);
	obs_data_set_bool(settings, "restart_on_activate", false);
	obs_data_set_bool(settings, "clear_on_media_end", false);
	obs_data_set_bool(settings, "close_when_inactive", false);
	obs_data_set_bool(settings, "hw_decode", false);
	p->media = obs_source_create("ffmpeg_source", "media", settings, NULL);
	obs_data_release(settings);
	assert_non_null(p->media);

	p->started = true;
	p->scene = obs_scene_create("with media");
	obs_scene_add(p->scene, p->media);

	memset(&p->w, 0, sizeof(p->w));
	pthread_mutex_init(&p->w.mutex, NULL);
	obs_add_raw_video_callback(NULL, raw_video, &p->w);
	obs_set_output_source(0, obs_scene_get_source(p->scene));
}

static void player_stop(struct player *p)
{
	if (!p->started)
		return;
	p->started = false;
	obs_remove_raw_video_callback(raw_video, &p->w);
	obs_set_output_source(0, NULL);
	/* the main canvas holds a reference to its scenes until they are removed */
	obs_source_remove(obs_scene_get_source(p->scene));
	obs_scene_release(p->scene);
	obs_source_release(p->media);
	pthread_mutex_destroy(&p->w.mutex);
}

static bool wait_state(obs_source_t *media, enum obs_media_state want, int ms)
{
	for (int i = 0; i < ms / 10; i++) {
		if (obs_source_media_get_state(media) == want)
			return true;
		os_sleep_ms(10);
	}
	return false;
}

/* Warm up: the clip is playing, its time moves and its picture is out. */
static void wait_playing(struct player *p)
{
	bool playing = wait_state(p->media, OBS_MEDIA_STATE_PLAYING, 3000);
	int64_t t = 0;
	for (int i = 0; i < 300 && t < 500; i++) {
		os_sleep_ms(10);
		t = obs_source_media_get_time(p->media);
	}
	bool picture = false;
	for (int i = 0; i < 300 && !picture; i++) {
		os_sleep_ms(10);
		picture = showing_picture(&p->w);
	}
	if (!playing || t < 500 || !picture) {
		fprintf(stderr, "setup failed: playing %d, time %lld ms, picture %d\n", playing, (long long)t, picture);
		fail();
	}
}

/* After Play, what the user sees: state Playing, the time moving by at least
 * 500 ms within 2 s, and the clip's picture in the output. */
static void expect_plays(struct player *p, const char *what)
{
	int64_t t0 = obs_source_media_get_time(p->media);
	int64_t t = t0;
	for (int i = 0; i < 200 && t - t0 < 500; i++) {
		os_sleep_ms(10);
		t = obs_source_media_get_time(p->media);
	}
	bool picture = false;
	for (int i = 0; i < 100 && !picture; i++) {
		picture = showing_picture(&p->w);
		if (!picture)
			os_sleep_ms(10);
	}
	enum obs_media_state state = obs_source_media_get_state(p->media);

	fprintf(stderr, "%s: state %d, time %lld -> %lld ms, picture %d\n", what, (int)state, (long long)t0,
		(long long)t, picture);

	if (state != OBS_MEDIA_STATE_PLAYING || t - t0 < 500 || !picture) {
		fprintf(stderr, "%s: the source did not play (state %d, time moved %lld ms, picture %d)\n", what,
			(int)state, (long long)(t - t0), picture);
		fail();
	}
}

/* obs-websocket#1046: TriggerMediaInputAction STOP, then PLAY. */
static void play_after_stop(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	player_start(&cur);
	wait_playing(&cur);

	obs_source_media_stop(cur.media);
	bool stopped = wait_state(cur.media, OBS_MEDIA_STATE_STOPPED, 2000);
	bool dark = false;
	for (int i = 0; i < 200 && !dark; i++) {
		os_sleep_ms(10);
		dark = !showing_picture(&cur.w);
	}
	if (!stopped || !dark) {
		fprintf(stderr, "stop failed: stopped %d, output cleared %d\n", stopped, dark);
		fail();
	}
	/* Let the player thread handle the stop: it sleeps up to one frame
	 * (100 ms here, 200 ms at most) before it looks at the request. */
	os_sleep_ms(500);

	obs_source_media_play_pause(cur.media, false);
	os_sleep_ms(200);
	expect_plays(&cur, "play after stop");
}

/* A script sending Stop and then Play at once: both run on the same video
 * tick, before the player thread has handled the stop. The late stop must
 * not report the playing source as ended. */
static void play_right_after_stop(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	player_start(&cur);
	wait_playing(&cur);

	obs_source_media_stop(cur.media);
	obs_source_media_play_pause(cur.media, false);
	os_sleep_ms(500);
	expect_plays(&cur, "stop and play at once");
}

/* obs-studio#4252: Play on a source whose file has ended starts it over. */
static void play_after_end(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	player_start(&cur);
	wait_playing(&cur);

	obs_source_media_set_time(cur.media, CLIP_MS - 500);
	if (!wait_state(cur.media, OBS_MEDIA_STATE_ENDED, 4000)) {
		fprintf(stderr, "setup failed: the clip did not end\n");
		fail();
	}

	obs_source_media_play_pause(cur.media, false);
	os_sleep_ms(200);
	expect_plays(&cur, "play after end");
}

/* Guard: Play after Pause resumes where it paused, it does not start over. */
static void play_after_pause_resumes(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	player_start(&cur);
	wait_playing(&cur);
	os_sleep_ms(500);

	obs_source_media_play_pause(cur.media, true);
	bool paused = wait_state(cur.media, OBS_MEDIA_STATE_PAUSED, 2000);
	os_sleep_ms(300);
	int64_t at_pause = obs_source_media_get_time(cur.media);
	os_sleep_ms(300);
	int64_t still = obs_source_media_get_time(cur.media);
	if (!paused || still != at_pause || at_pause < 700) {
		fprintf(stderr, "pause failed: paused %d, time %lld then %lld ms\n", paused, (long long)at_pause,
			(long long)still);
		fail();
	}

	obs_source_media_play_pause(cur.media, false);
	os_sleep_ms(200);
	int64_t resumed = obs_source_media_get_time(cur.media);
	expect_plays(&cur, "play after pause");

	fprintf(stderr, "paused at %lld ms, resumed at %lld ms\n", (long long)at_pause, (long long)resumed);
	if (resumed < at_pause - 100) {
		fprintf(stderr, "play after pause started over instead of resuming\n");
		fail();
	}
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
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	struct obs_audio_info oai = {.samples_per_sec = 48000, .speakers = SPEAKERS_STEREO};
	if (obs_reset_video(&ovi) != OBS_VIDEO_SUCCESS || !obs_reset_audio(&oai)) {
		obs_shutdown();
		return 0;
	}

	snprintf(clip_path, sizeof(clip_path), "%s/obs-media-play-after-stop-%llu.y4m",
		 getenv("TMPDIR") ? getenv("TMPDIR") : "/tmp", (unsigned long long)os_gettime_ns());
	if (!write_clip(clip_path)) {
		fprintf(stderr, "could not write the test clip %s\n", clip_path);
		obs_shutdown();
		return -1;
	}

	obs_register_source(&ffmpeg_source);
	obs_up = true;
	return 0;
}

static int test_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	player_stop(&cur);
	return 0;
}

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);

	if (obs_up) {
		obs_shutdown();
		os_unlink(clip_path);
	}
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_teardown(play_after_stop, test_teardown),
		cmocka_unit_test_teardown(play_right_after_stop, test_teardown),
		cmocka_unit_test_teardown(play_after_end, test_teardown),
		cmocka_unit_test_teardown(play_after_pause_resumes, test_teardown),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
