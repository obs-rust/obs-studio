/* Regression test for obsproject/obs-studio#10403 (reported by k223223): a
 * Color Correction filter at its default (neutral) settings turns some
 * colours of a macOS Screen Capture black or transparent.
 *
 * Screen Capture draws in the high-precision sRGB space (GS_CS_SRGB_16F):
 * linear sRGB in a half-float texture, converted from the Display P3 the
 * capture delivers. Colours outside the sRGB gamut (a P3 green) come out
 * with negative channels, and rounding gives tiny negative channels to
 * saturated in-gamut colours too. The filter renders such a source in that
 * space, and its shader raised every channel to the gamma power, which is
 * undefined for a negative base: pow() returns NaN on the GPU, the colour
 * matrix carries the NaN into alpha, and the pixel ends up black or
 * transparent. At neutral settings the gamma is 1, so the filter should
 * leave the picture as it is.
 *
 * The test registers a source that, like Screen Capture, draws in
 * GS_CS_SRGB_16F when it may, and paints its left half the linear sRGB of
 * the P3 green primary (-0.225, 1.042, -0.079) and its right half grey. It
 * reads the program back as BGRA (raw video callback) with and without the
 * filter:
 *
 *   - high precision, neutral filter: the picture must match the unfiltered
 *     source within rounding;
 *   - high precision, gamma changed: the green must stay green (the gamma
 *     curve is mirrored for negative values, as for extended sRGB);
 *   - control, 8-bit sRGB source with in-gamut colours: neutral filter and
 *     unfiltered match (they did before the fix too).
 *
 * Core video needs a graphics module and libobs needs an X display on Linux,
 * so the cases skip without one (run under xvfb-run). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <math.h>
#include <stdio.h>
#include <stdlib.h>

#include <obs.h>
#include <obs-module.h>
#include <graphics/vec4.h>
#include <util/platform.h>
#include <util/threading.h>

#define BOX_ID "test_color_correction_16f_box"
#define BASE 64
/* 8-bit rounding, plus the float error of the neutral colour matrix */
#define TOLERANCE 2

static bool obs_up;
static obs_module_t *filters_module;

/* ------------------------------------------------------------------------- */
/* a box: left half one colour, right half another, in a chosen space */

struct box {
	struct vec4 left;
	struct vec4 right;
	bool high_precision;
};

static const char *box_name(void *type_data)
{
	(void)type_data;
	return "box";
}

static void box_update(void *data, obs_data_t *settings)
{
	struct box *b = data;
	b->high_precision = obs_data_get_bool(settings, "high_precision");
	vec4_set(&b->left, (float)obs_data_get_double(settings, "lr"), (float)obs_data_get_double(settings, "lg"),
		 (float)obs_data_get_double(settings, "lb"), 1.0f);
	vec4_set(&b->right, (float)obs_data_get_double(settings, "rr"), (float)obs_data_get_double(settings, "rg"),
		 (float)obs_data_get_double(settings, "rb"), 1.0f);
}

static void *box_create(obs_data_t *settings, obs_source_t *source)
{
	(void)source;
	struct box *b = bzalloc(sizeof(*b));
	box_update(b, settings);
	return b;
}

static void box_destroy(void *data)
{
	bfree(data);
}

static uint32_t box_size(void *data)
{
	(void)data;
	return BASE;
}

static void draw_half(gs_effect_t *solid, const struct vec4 *color, float x)
{
	gs_effect_set_vec4(gs_effect_get_param_by_name(solid, "color"), color);
	gs_matrix_push();
	gs_matrix_translate3f(x, 0.0f, 0.0f);
	while (gs_effect_loop(solid, "Solid"))
		gs_draw_sprite(NULL, 0, BASE / 2, BASE);
	gs_matrix_pop();
}

/* The colours are linear: Solid writes them as they are, into a linear
 * half-float target (GS_CS_SRGB_16F) or an sRGB-encoding 8-bit one. */
static void box_render(void *data, gs_effect_t *effect)
{
	struct box *b = data;
	(void)effect;
	gs_effect_t *solid = obs_get_base_effect(OBS_EFFECT_SOLID);
	const bool previous = gs_framebuffer_srgb_enabled();
	gs_enable_framebuffer_srgb(true);
	draw_half(solid, &b->left, 0.0f);
	draw_half(solid, &b->right, (float)(BASE / 2));
	gs_enable_framebuffer_srgb(previous);
}

/* Like Screen Capture: high precision when the caller allows it. */
static enum gs_color_space box_color_space(void *data, size_t count, const enum gs_color_space *preferred_spaces)
{
	struct box *b = data;
	if (b->high_precision) {
		for (size_t i = 0; i < count; i++) {
			if (preferred_spaces[i] == GS_CS_SRGB_16F)
				return GS_CS_SRGB_16F;
		}
	}
	return GS_CS_SRGB;
}

static void register_types(void)
{
	struct obs_source_info box = {
		.id = BOX_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_CUSTOM_DRAW | OBS_SOURCE_SRGB,
		.get_name = box_name,
		.create = box_create,
		.destroy = box_destroy,
		.update = box_update,
		.get_width = box_size,
		.get_height = box_size,
		.video_render = box_render,
		.video_get_color_space = box_color_space,
	};
	obs_register_source(&box);
}

/* ------------------------------------------------------------------------- */
/* the program read back as BGRA: the pixel at the middle of each half */

struct probe {
	pthread_mutex_t mutex;
	long frames;
	uint8_t left[4];
	uint8_t right[4];
};

static struct probe probe;

static void probe_frame(void *param, struct video_data *frame)
{
	struct probe *p = param;
	const uint8_t *row = frame->data[0] + (size_t)(BASE / 2) * frame->linesize[0];
	pthread_mutex_lock(&p->mutex);
	for (int c = 0; c < 4; c++) {
		p->left[c] = row[(BASE / 4) * 4 + c];
		p->right[c] = row[(3 * BASE / 4) * 4 + c];
	}
	p->frames++;
	pthread_mutex_unlock(&p->mutex);
}

struct picture {
	/* B, G, R, A as the frame stores them */
	uint8_t left[4];
	uint8_t right[4];
};

/* Puts the box on the program, with a Color Correction filter (settings
 * given) or without one (NULL), and returns the picture after it settles. */
static void run_scene(const struct vec4 *left, const struct vec4 *right, bool high_precision,
		      obs_data_t *filter_settings, struct picture *pic)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_bool(settings, "high_precision", high_precision);
	obs_data_set_double(settings, "lr", left->x);
	obs_data_set_double(settings, "lg", left->y);
	obs_data_set_double(settings, "lb", left->z);
	obs_data_set_double(settings, "rr", right->x);
	obs_data_set_double(settings, "rg", right->y);
	obs_data_set_double(settings, "rb", right->z);
	obs_source_t *src = obs_source_create(BOX_ID, "box", settings, NULL);
	obs_data_release(settings);
	assert_non_null(src);

	obs_source_t *filter = NULL;
	if (filter_settings) {
		filter = obs_source_create_private("color_filter_v2", "Color Correction", filter_settings);
		assert_non_null(filter);
		obs_source_filter_add(src, filter);
	}

	obs_scene_t *scene = obs_scene_create("scene");
	obs_scene_add(scene, src);
	obs_set_output_source(0, obs_scene_get_source(scene));

	pthread_mutex_lock(&probe.mutex);
	const long start = probe.frames;
	pthread_mutex_unlock(&probe.mutex);
	for (int i = 0; i < 300; i++) {
		pthread_mutex_lock(&probe.mutex);
		bool ready = probe.frames >= start + 10;
		pthread_mutex_unlock(&probe.mutex);
		if (ready)
			break;
		os_sleep_ms(10);
	}
	pthread_mutex_lock(&probe.mutex);
	assert_true(probe.frames >= start + 10);
	for (int c = 0; c < 4; c++) {
		pic->left[c] = probe.left[c];
		pic->right[c] = probe.right[c];
	}
	pthread_mutex_unlock(&probe.mutex);

	fprintf(stderr, "%s, %s: left BGRA %3u %3u %3u %3u, right BGRA %3u %3u %3u %3u\n",
		high_precision ? "16F source" : "8-bit source", filter_settings ? "with filter" : "no filter   ",
		pic->left[0], pic->left[1], pic->left[2], pic->left[3], pic->right[0], pic->right[1], pic->right[2],
		pic->right[3]);

	obs_set_output_source(0, NULL);
	obs_source_remove(obs_scene_get_source(scene));
	obs_scene_release(scene);
	if (filter) {
		obs_source_filter_remove(src, filter);
		obs_source_release(filter);
	}
	obs_source_release(src);
}

static void assert_same_picture(const struct picture *a, const struct picture *b)
{
	for (int c = 0; c < 4; c++) {
		assert_true(abs((int)a->left[c] - (int)b->left[c]) <= TOLERANCE);
		assert_true(abs((int)a->right[c] - (int)b->right[c]) <= TOLERANCE);
	}
}

/* linear sRGB of the Display P3 green primary: outside the sRGB gamut */
static const struct vec4 p3_green = {.x = -0.2249f, .y = 1.0421f, .z = -0.0786f, .w = 1.0f};
static const struct vec4 grey = {.x = 0.5f, .y = 0.5f, .z = 0.5f, .w = 1.0f};

/* The report: on a high-precision source the neutral filter must leave the
 * picture as it is, not turn the green black or transparent. */
static void neutral_filter_keeps_high_precision_source(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct picture plain, filtered;
	run_scene(&p3_green, &grey, true, NULL, &plain);
	obs_data_t *neutral = obs_data_create();
	run_scene(&p3_green, &grey, true, neutral, &filtered);
	obs_data_release(neutral);

	/* the unfiltered source shows an opaque, full green */
	assert_true(plain.left[1] > 250 && plain.left[3] == 255);
	assert_same_picture(&plain, &filtered);
}

/* With the gamma changed the out-of-gamut green must still come out as
 * green: the gamma curve applies to the magnitude of a negative channel. */
static void gamma_keeps_high_precision_colour(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct picture filtered;
	obs_data_t *gamma = obs_data_create();
	obs_data_set_double(gamma, "gamma", 0.5);
	run_scene(&p3_green, &grey, true, gamma, &filtered);
	obs_data_release(gamma);

	assert_true(filtered.left[0] <= TOLERANCE);
	assert_true(filtered.left[1] > 250);
	assert_true(filtered.left[2] <= TOLERANCE);
	assert_int_equal(filtered.left[3], 255);
	/* grey gets lighter with a positive gamma setting */
	assert_true(filtered.right[1] > 195 && filtered.right[3] == 255);
}

/* Control: an 8-bit sRGB source is left as it is by the neutral filter. */
static void neutral_filter_keeps_8bit_source(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	const struct vec4 blue = {.x = 0.05f, .y = 0.3f, .z = 0.8f, .w = 1.0f};
	struct picture plain, filtered;
	run_scene(&blue, &grey, false, NULL, &plain);
	obs_data_t *neutral = obs_data_create();
	run_scene(&blue, &grey, false, neutral, &filtered);
	obs_data_release(neutral);

	assert_int_equal(plain.left[3], 255);
	assert_same_picture(&plain, &filtered);
}

static int group_setup(void **state)
{
	(void)state;
	pthread_mutex_init(&probe.mutex, NULL);

	const char *display = getenv("DISPLAY");
	if (!display || !*display) {
		fprintf(stderr, "skipped: no display\n");
		return 0;
	}
	if (!obs_startup("en-US", NULL, NULL)) {
		fprintf(stderr, "skipped: obs_startup failed\n");
		return 0;
	}

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = BASE,
		.base_height = BASE,
		.output_width = BASE,
		.output_height = BASE,
		.output_format = VIDEO_FORMAT_BGRA,
		.colorspace = VIDEO_CS_SRGB,
		.range = VIDEO_RANGE_FULL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	int ret = obs_reset_video(&ovi);
	if (ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "skipped: obs_reset_video failed: %d\n", ret);
		obs_shutdown();
		return 0;
	}

	register_types();

	if (obs_open_module(&filters_module, FILTERS_MODULE_PATH, FILTERS_DATA_PATH) != MODULE_SUCCESS ||
	    !obs_init_module(filters_module)) {
		fprintf(stderr, "could not load the obs-filters module\n");
		obs_shutdown();
		return -1;
	}

	obs_add_raw_video_callback(NULL, probe_frame, &probe);
	obs_up = true;
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up) {
		obs_remove_raw_video_callback(probe_frame, &probe);
		obs_shutdown();
		obs_up = false;
	}
	pthread_mutex_destroy(&probe.mutex);
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(neutral_filter_keeps_high_precision_source),
		cmocka_unit_test(gamma_keeps_high_precision_colour),
		cmocka_unit_test(neutral_filter_keeps_8bit_source),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
