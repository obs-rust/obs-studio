/* A deinterlace mode outside enum obs_deinterlace_mode
 * (obs-rust/obs-studio#2090).
 *
 * obs_source_set_deinterlace_mode() took every mode but
 * OBS_DEINTERLACE_MODE_DISABLE as "deinterlacing on", and get_effect() has no
 * effect for a mode it does not know, so an async source with such a mode
 * drew nothing. A scene collection stores the mode as an integer, so an
 * edited or generated collection with "deinterlace_mode": 99 loaded a black
 * camera or media source; a mode past the int range was cut to its low 32
 * bits. A mode outside the enum is now disabled, on load and in the setter,
 * and the getter and the saved collection say so.
 *
 * Each case loads an async test source from obs_data with obs_load_source(),
 * as a scene collection is loaded, or creates one and calls the setter,
 * shows it on channel 0, pushes solid frames and counts the output frames
 * that show the source's colour, read back through a raw video callback.
 * Every valid mode, DISABLE included, loads and switches as before, and the
 * field order loads as before.
 *
 * Needs core video, so it runs (under xvfb-run) on Linux and the cases skip
 * without a display. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <obs.h>
#include <util/platform.h>
#include <util/threading.h>

#define SOURCE_ID "deinterlace_mode_load_source"
#define W 64
#define H 64
#define CANVAS_FPS 30

/* The source's colour (BGRA) and how far an output pixel may be from it (the
 * linear modes go through sRGB textures). */
#define COLOR_B 40
#define COLOR_G 160
#define COLOR_R 220
#define TOLERANCE 8

/* A check pushes frames at the canvas rate until MIN_DRAWN output frames
 * have shown the source's colour, for at most MAX_PUSH_FRAMES (3 s). A drawn
 * source gets there in a few frames, or after a fraction of a second when a
 * new mode or a loaded machine delays its first frames; a source that draws
 * nothing never does. */
#define MAX_PUSH_FRAMES 90
#define MIN_DRAWN 3

static bool obs_up;

struct watch {
	pthread_mutex_t mutex;
	bool recording;
	size_t ticks;
	size_t drawn;
	/* What each recorded output frame showed: 'o' the source's colour,
	 * '.' black, 'x' anything else (the last such pixel is kept). */
	char trace[64];
	uint8_t other[3];
};

static struct watch watch;

static bool close_to(int v, int want)
{
	return v >= want - TOLERANCE && v <= want + TOLERANCE;
}

static void raw_video(void *param, struct video_data *frame)
{
	struct watch *w = param;
	const uint8_t *px = frame->data[0] + (H / 2) * frame->linesize[0] + (W / 2) * 4;
	bool drawn = close_to(px[0], COLOR_B) && close_to(px[1], COLOR_G) && close_to(px[2], COLOR_R);

	bool black = px[0] < TOLERANCE && px[1] < TOLERANCE && px[2] < TOLERANCE;

	pthread_mutex_lock(&w->mutex);
	if (w->recording) {
		if (w->ticks < sizeof(w->trace) - 1)
			w->trace[w->ticks] = drawn ? 'o' : black ? '.' : 'x';
		if (!drawn && !black)
			memcpy(w->other, px, 3);
		w->ticks++;
		if (drawn)
			w->drawn++;
	}
	pthread_mutex_unlock(&w->mutex);
}

static const char *test_source_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "deinterlace mode test source";
}

static void *test_source_create(obs_data_t *settings, obs_source_t *source)
{
	UNUSED_PARAMETER(settings);
	return source;
}

static void test_source_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static struct obs_source_info test_source = {
	.id = SOURCE_ID,
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = OBS_SOURCE_ASYNC_VIDEO,
	.get_name = test_source_name,
	.create = test_source_create,
	.destroy = test_source_destroy,
};

/* Loads a source from `json` the way a scene collection loads it. */
static obs_source_t *load_source(const char *json)
{
	obs_data_t *data = obs_data_create_from_json(json);
	assert_non_null(data);
	obs_source_t *source = obs_load_source(data);
	obs_data_release(data);
	assert_non_null(source);
	obs_source_load2(source);
	return source;
}

static void unload_source(obs_source_t *source)
{
	obs_source_remove(source);
	obs_source_release(source);
}

/* Shows `source` alone on channel 0, pushes solid frames at the canvas rate
 * (see MAX_PUSH_FRAMES) and returns the output frames that showed them. */
static size_t count_drawn(obs_source_t *source)
{
	static uint8_t pixels[W * H * 4];
	for (size_t i = 0; i < sizeof(pixels); i += 4) {
		pixels[i + 0] = COLOR_B;
		pixels[i + 1] = COLOR_G;
		pixels[i + 2] = COLOR_R;
		pixels[i + 3] = 255;
	}

	obs_scene_t *scene = obs_scene_create_private("deinterlace mode scene");
	obs_scene_add(scene, source);
	obs_set_output_source(0, obs_scene_get_source(scene));

	pthread_mutex_lock(&watch.mutex);
	watch.ticks = 0;
	watch.drawn = 0;
	memset(watch.trace, 0, sizeof(watch.trace));
	watch.recording = true;
	pthread_mutex_unlock(&watch.mutex);

	const uint64_t period = 1000000000ULL / CANVAS_FPS;
	const uint64_t start = os_gettime_ns();
	for (int n = 0; n < MAX_PUSH_FRAMES; n++) {
		uint64_t ts = start + n * period;
		struct obs_source_frame frame = {
			.data = {pixels},
			.linesize = {W * 4},
			.width = W,
			.height = H,
			.format = VIDEO_FORMAT_BGRA,
			.full_range = true,
			.timestamp = ts,
		};
		os_sleepto_ns(ts);
		obs_source_output_video(source, &frame);

		pthread_mutex_lock(&watch.mutex);
		const bool enough = watch.drawn >= MIN_DRAWN;
		pthread_mutex_unlock(&watch.mutex);
		if (enough)
			break;
	}

	pthread_mutex_lock(&watch.mutex);
	watch.recording = false;
	const size_t ticks = watch.ticks;
	const size_t drawn = watch.drawn;
	char trace[sizeof(watch.trace)];
	memcpy(trace, watch.trace, sizeof(trace));
	const uint8_t other[3] = {watch.other[0], watch.other[1], watch.other[2]};
	pthread_mutex_unlock(&watch.mutex);

	obs_set_output_source(0, NULL);
	obs_scene_release(scene);
	/* Drop the last frame, so the next check starts from a source that
	 * shows nothing. */
	obs_source_output_video(source, NULL);

	fprintf(stderr, "  %zu of %zu output frames showed the source: %s\n", drawn, ticks, trace);
	if (strchr(trace, 'x'))
		fprintf(stderr, "  last other pixel (B, G, R): %d, %d, %d\n", other[0], other[1], other[2]);
	return drawn;
}

static int64_t saved_int(obs_source_t *source, const char *name)
{
	obs_data_t *saved = obs_save_source(source);
	assert_non_null(saved);
	int64_t value = obs_data_get_int(saved, name);
	obs_data_release(saved);
	return value;
}

/* Checks a source the mode `want` should be in: the getter, the saved
 * collection, and that its frames are drawn. Returns the number of checks
 * that failed. */
static int check_source(const char *what, obs_source_t *source, enum obs_deinterlace_mode want)
{
	int wrong = 0;
	enum obs_deinterlace_mode mode = obs_source_get_deinterlace_mode(source);
	int64_t saved = saved_int(source, "deinterlace_mode");

	fprintf(stderr, "%s:\n", what);
	if (mode != want) {
		fprintf(stderr, "  obs_source_get_deinterlace_mode() is %d, expected %d\n", (int)mode, (int)want);
		wrong++;
	}
	if (saved != (int64_t)want) {
		fprintf(stderr, "  saved \"deinterlace_mode\" is %lld, expected %d\n", (long long)saved, (int)want);
		wrong++;
	}
	if (count_drawn(source) < MIN_DRAWN) {
		fprintf(stderr, "  the source draws nothing\n");
		wrong++;
	}
	return wrong;
}

static int check_loaded(const char *json, enum obs_deinterlace_mode want)
{
	obs_source_t *source = load_source(json);
	int wrong = check_source(json, source, want);
	unload_source(source);
	return wrong;
}

/* ------------------------------------------------------------------------ */

/* "deinterlace_mode": 99 in a scene collection: the source is drawn, not
 * black, and its mode is DISABLE. */
static void loaded_out_of_range_mode_is_disabled(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	int wrong = check_loaded("{\"id\": \"" SOURCE_ID "\", \"name\": \"mode 99\", \"deinterlace_mode\": 99}",
				 OBS_DEINTERLACE_MODE_DISABLE);
	wrong += check_loaded("{\"id\": \"" SOURCE_ID "\", \"name\": \"mode 9\", \"deinterlace_mode\": 9}",
			      OBS_DEINTERLACE_MODE_DISABLE);
	assert_int_equal(wrong, 0);
}

static void loaded_negative_mode_is_disabled(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	int wrong = check_loaded("{\"id\": \"" SOURCE_ID "\", \"name\": \"mode -1\", \"deinterlace_mode\": -1}",
				 OBS_DEINTERLACE_MODE_DISABLE);
	assert_int_equal(wrong, 0);
}

/* 2^32 + 2 is no mode. Cut to its low 32 bits it was RETRO (2). */
static void loaded_mode_past_int_range_is_disabled(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	int wrong = check_loaded("{\"id\": \"" SOURCE_ID
				 "\", \"name\": \"mode 2^32 + 2\", \"deinterlace_mode\": 4294967298}",
				 OBS_DEINTERLACE_MODE_DISABLE);
	assert_int_equal(wrong, 0);
}

/* The setter, on a source with deinterlacing off. */
static void set_out_of_range_mode_is_disabled(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_source_t *source = obs_source_create(SOURCE_ID, "set 99", NULL, NULL);
	assert_non_null(source);
	obs_source_set_deinterlace_mode(source, (enum obs_deinterlace_mode)99);
	int wrong = check_source("set 99", source, OBS_DEINTERLACE_MODE_DISABLE);
	obs_source_set_deinterlace_mode(source, (enum obs_deinterlace_mode) - 1);
	wrong += check_source("set -1", source, OBS_DEINTERLACE_MODE_DISABLE);
	unload_source(source);
	assert_int_equal(wrong, 0);
}

/* The setter, on a source that deinterlaces: the mode switches to DISABLE
 * (as setting DISABLE does), not to a mode with no effect. */
static void set_out_of_range_mode_after_a_mode_is_disabled(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_source_t *source = obs_source_create(SOURCE_ID, "yadif then 99", NULL, NULL);
	assert_non_null(source);
	obs_source_set_deinterlace_mode(source, OBS_DEINTERLACE_MODE_YADIF);
	int wrong = check_source("set yadif", source, OBS_DEINTERLACE_MODE_YADIF);
	obs_source_set_deinterlace_mode(source, (enum obs_deinterlace_mode)99);
	wrong += check_source("then set 99", source, OBS_DEINTERLACE_MODE_DISABLE);
	obs_source_set_deinterlace_mode(source, OBS_DEINTERLACE_MODE_BLEND_2X);
	wrong += check_source("then set blend 2x", source, OBS_DEINTERLACE_MODE_BLEND_2X);
	unload_source(source);
	assert_int_equal(wrong, 0);
}

/* Every valid mode, DISABLE included, loads as it is stored, and the setter
 * switches between them as before. */
static void valid_modes_unchanged(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	int wrong = check_loaded("{\"id\": \"" SOURCE_ID "\", \"name\": \"no mode\"}", OBS_DEINTERLACE_MODE_DISABLE);

	obs_source_t *set = obs_source_create(SOURCE_ID, "set each mode", NULL, NULL);
	assert_non_null(set);
	for (int mode = OBS_DEINTERLACE_MODE_DISABLE; mode <= OBS_DEINTERLACE_MODE_YADIF_2X; mode++) {
		char json[160];
		snprintf(json, sizeof(json), "{\"id\": \"%s\", \"name\": \"mode %d\", \"deinterlace_mode\": %d}",
			 SOURCE_ID, mode, mode);
		wrong += check_loaded(json, (enum obs_deinterlace_mode)mode);

		char what[64];
		snprintf(what, sizeof(what), "set mode %d", mode);
		obs_source_set_deinterlace_mode(set, (enum obs_deinterlace_mode)mode);
		wrong += check_source(what, set, (enum obs_deinterlace_mode)mode);
	}
	unload_source(set);
	assert_int_equal(wrong, 0);
}

/* The field order loads as before: 0 is top field first, any other value
 * bottom field first, and the source is drawn either way. */
static void field_order_unchanged(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	static const struct {
		int stored;
		enum obs_deinterlace_field_order want;
	} cases[] = {
		{0, OBS_DEINTERLACE_FIELD_ORDER_TOP},
		{1, OBS_DEINTERLACE_FIELD_ORDER_BOTTOM},
		{5, OBS_DEINTERLACE_FIELD_ORDER_BOTTOM},
		{-1, OBS_DEINTERLACE_FIELD_ORDER_BOTTOM},
	};

	int wrong = 0;
	for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++) {
		char json[192];
		snprintf(json, sizeof(json),
			 "{\"id\": \"%s\", \"name\": \"order %d\", \"deinterlace_mode\": %d,"
			 " \"deinterlace_field_order\": %d}",
			 SOURCE_ID, cases[i].stored, (int)OBS_DEINTERLACE_MODE_YADIF, cases[i].stored);
		obs_source_t *source = load_source(json);
		enum obs_deinterlace_field_order order = obs_source_get_deinterlace_field_order(source);
		int64_t saved = saved_int(source, "deinterlace_field_order");
		if (order != cases[i].want || saved != (int64_t)cases[i].want) {
			fprintf(stderr, "%s: field order %d, saved %lld, expected %d\n", json, (int)order,
				(long long)saved, (int)cases[i].want);
			wrong++;
		}
		wrong += check_source(json, source, OBS_DEINTERLACE_MODE_YADIF);
		unload_source(source);
	}
	assert_int_equal(wrong, 0);
}

static int setup(void **state)
{
	(void)state;
	pthread_mutex_init(&watch.mutex, NULL);

	/* obs_startup() needs a display on Linux (run under xvfb-run); the
	 * cases skip without one. */
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = CANVAS_FPS,
		.fps_den = 1,
		.base_width = W,
		.base_height = H,
		.output_width = W,
		.output_height = H,
		.output_format = VIDEO_FORMAT_BGRA,
		.colorspace = VIDEO_CS_SRGB,
		.range = VIDEO_RANGE_FULL,
		.scale_type = OBS_SCALE_POINT,
	};
	int ret = obs_reset_video(&ovi);
	if (ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "skipped: obs_reset_video failed: %d\n", ret);
		obs_shutdown();
		obs_up = false;
		return 0;
	}

	obs_register_source(&test_source);
	obs_add_raw_video_callback(NULL, raw_video, &watch);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (obs_up) {
		obs_remove_raw_video_callback(raw_video, &watch);
		obs_shutdown();
	}
	pthread_mutex_destroy(&watch.mutex);
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(loaded_out_of_range_mode_is_disabled),
		cmocka_unit_test(loaded_negative_mode_is_disabled),
		cmocka_unit_test(loaded_mode_past_int_range_is_disabled),
		cmocka_unit_test(set_out_of_range_mode_is_disabled),
		cmocka_unit_test(set_out_of_range_mode_after_a_mode_is_disabled),
		cmocka_unit_test(valid_modes_unchanged),
		cmocka_unit_test(field_order_unchanged),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
