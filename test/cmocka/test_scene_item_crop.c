/* Pins the current C behavior of the scene item crop arithmetic.
 *
 * obs-scene.c.h sizes a cropped source with obs_scene_cropped_size()
 * (calc_cx and calc_cy), rescales a nested scene's crop with
 * obs_scene_scale_crop() when that scene changes size
 * (update_nested_scene_crop), moves the source by obs_scene_crop_offset()
 * when it is drawn (render_item), and clamps each side of a new crop with
 * obs_scene_clamp_crop() (obs_sceneitem_set_crop). The scene still reads and
 * writes the item and computes the rescale factors.
 *
 * The first cases call the four functions directly. None of them are
 * exported from libobs, so this executable compiles
 * libobs/obs-scene-crop.c.h itself and characterizes that C. They need no
 * display.
 *
 * The other cases go through the real libobs: a scene and an item over a
 * source of a fixed size, the exported obs_sceneitem_set_crop() and
 * obs_sceneitem_get_crop(), and the item's drawn size
 * (obs_sceneitem_get_box_scale, scale 1). A nested scene's crop is rescaled
 * only when the scene is rendered, so these need core video (run under
 * xvfb-run) and skip without a display. In the hybrid libobs they run the
 * swapped functions.
 *
 * Cases marked characterized, not endorsed, are what the C does today,
 * including the odd parts. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <float.h>
#include <limits.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <obs-scene-crop.h>
#include <graphics/vec2.h>
#include <util/platform.h>
#include <util/threading.h>

#define BOX_ID "scene_item_crop_box"
#define CANVAS_CX 1280
#define CANVAS_CY 720

static bool obs_up;

static uint32_t bits(float f)
{
	uint32_t u;
	memcpy(&u, &f, sizeof(u));
	return u;
}

static float from_bits(uint32_t u)
{
	float f;
	memcpy(&f, &u, sizeof(f));
	return f;
}

/* Bit for bit: -0 is not +0. */
static void check_float(float got, float want, const char *what)
{
	if (bits(got) == bits(want))
		return;

	fprintf(stderr, "%s: %.9g (0x%08x), expected %.9g (0x%08x)\n", what, got, bits(got), want, bits(want));
	fail();
}

static void check_size(uint32_t size, int crop1, int crop2, int bounds_crop1, int bounds_crop2, uint32_t want)
{
	uint32_t got = obs_scene_cropped_size(size, crop1, crop2, bounds_crop1, bounds_crop2);
	if (got == want)
		return;

	fprintf(stderr, "cropped_size(%u, %d, %d, %d, %d) is %u, expected %u\n", size, crop1, crop2, bounds_crop1,
		bounds_crop2, got, want);
	fail();
}

static void check_scale(int crop, float factor, int want)
{
	int got = obs_scene_scale_crop(crop, factor);
	if (got == want)
		return;

	fprintf(stderr, "scale_crop(%d, %.9g [0x%08x]) is %d, expected %d\n", crop, factor, bits(factor), got, want);
	fail();
}

static void check_offset(int crop, int bounds_crop, float want)
{
	char what[64];
	snprintf(what, sizeof(what), "crop_offset(%d, %d)", crop, bounds_crop);
	check_float(obs_scene_crop_offset(crop, bounds_crop), want, what);
}

static void check_clamp(int crop, int want)
{
	int got = obs_scene_clamp_crop(crop);
	if (got == want)
		return;

	fprintf(stderr, "clamp_crop(%d) is %d, expected %d\n", crop, got, want);
	fail();
}

/* ------------------------------------------------------------------------ */
/* The functions on their own. */

/* A crop that sums to the size leaves 0; one more pixel leaves 2. */
static void cropped_size_at_the_size(void **state)
{
	(void)state;

	check_size(200, 0, 0, 0, 0, 200);
	check_size(200, 10, 30, 0, 0, 160);
	check_size(200, 10, 30, 5, 5, 150);
	check_size(200, 0, 0, 7, 3, 190);
	check_size(200, 100, 100, 0, 0, 0);
	check_size(200, 50, 50, 50, 50, 0);
	check_size(200, 199, 0, 0, 0, 1);
	check_size(200, 100, 101, 0, 0, 2);
	check_size(200, 0, 0, 0, 201, 2);
	check_size(0, 0, 0, 0, 0, 0);
	check_size(0, 1, 0, 0, 0, 2);
	check_size(1, 0, 0, 0, 1, 0);
	check_size(2, 0, 0, 0, 0, 2);
	/* 2 px wide either way: by the crop or because it is 2 px. */
	check_size(3, 1, 0, 0, 0, 2);
}

/* The sum is 64 bits, so the four crops can be past the int range. */
static void cropped_size_past_the_int_range(void **state)
{
	(void)state;

	check_size(UINT32_MAX, 0, 0, 0, 0, UINT32_MAX);
	check_size(UINT32_MAX, INT_MAX, 0, 0, 0, (uint32_t)INT_MAX + 1);
	check_size(UINT32_MAX, INT_MAX, 1, 0, 0, (uint32_t)INT_MAX);
	check_size(UINT32_MAX, INT_MAX, INT_MAX, 0, 0, 1);
	/* Exactly UINT32_MAX: 0 left. */
	check_size(UINT32_MAX, INT_MAX, INT_MAX, 1, 0, 0);
	/* 2^32: one past the largest size. */
	check_size(UINT32_MAX, INT_MAX, INT_MAX, 1, 1, 2);
	check_size(UINT32_MAX, INT_MAX, INT_MAX, INT_MAX, INT_MAX, 2);
	check_size(200, INT_MAX, INT_MAX, INT_MAX, INT_MAX, 2);
	check_size((uint32_t)INT_MAX + 1, INT_MAX, 1, 0, 0, 0);
}

/* A negative sum crops all of it. A negative crop that the others make up
 * for is subtracted. Characterized, not endorsed: the setter never stores a
 * negative crop, but the bounds crop is not clamped there. */
static void cropped_size_negative(void **state)
{
	(void)state;

	check_size(200, -1, 0, 0, 0, 2);
	check_size(200, -100, 0, 0, 0, 2);
	check_size(200, INT_MIN, INT_MIN, 0, 0, 2);
	check_size(200, INT_MIN, INT_MIN, INT_MIN, INT_MIN, 2);
	check_size(UINT32_MAX, INT_MIN, INT_MAX, 0, 0, 2);
	check_size(200, -10, 50, 0, 0, 160);
	check_size(200, -10, 10, 0, 0, 200);
	check_size(200, INT_MIN, INT_MAX, 1, 0, 200);
	check_size(200, 0, 0, -5, 25, 180);
}

/* The rescale of a nested scene's crop, truncated toward 0. */
static void scale_crop_truncates(void **state)
{
	(void)state;

	check_scale(0, 4000.0f, 0);
	check_scale(100, 1.0f, 100);
	check_scale(100, 2.0f, 200);
	check_scale(100, 0.5f, 50);
	check_scale(10, 0.33f, 3);
	check_scale(-10, 0.33f, -3);
	check_scale(3, 1.5f, 4);
	check_scale(-3, 1.5f, -4);
	check_scale(1, 0.999999940f, 0);
	check_scale(-1, 0.999999940f, 0);
	check_scale(5, 0.0f, 0);
	/* -0 is 0. */
	check_scale(5, -0.0f, 0);
	check_scale(5, -1.0f, -5);
	check_scale(1000000000, 2.0f, 2000000000);
	/* Characterized, not endorsed: the crop is a float first, so a crop
	 * past 2^24 loses its low bits even at a factor of 1. */
	check_scale(16777217, 1.0f, 16777216);
	check_scale(2147483520, 1.0f, 2147483520);
	check_scale(-2147483520, 1.0f, -2147483520);
	check_scale(2147483583, 1.0f, 2147483520);
	/* The smallest factors: the product is 0 or below 1. */
	check_scale(INT_MAX, FLT_MIN, 0);
	check_scale(INT_MAX, from_bits(1), 0);
	check_scale(INT_MIN, from_bits(0x80000001u), 0);
}

/* NaN gives 0. The infinities and huge factors give the nearest int. */
static void scale_crop_nan_inf_huge(void **state)
{
	(void)state;

	check_scale(5, NAN, 0);
	check_scale(-5, -NAN, 0);
	check_scale(0, NAN, 0);
	check_scale(INT_MAX, from_bits(0x7fc00001u), 0);
	check_scale(INT_MIN, from_bits(0xff800001u), 0);
	check_scale(1, INFINITY, INT_MAX);
	check_scale(-1, INFINITY, INT_MIN);
	check_scale(1, -INFINITY, INT_MIN);
	check_scale(-1, -INFINITY, INT_MAX);
	/* 0 x inf is NaN. */
	check_scale(0, INFINITY, 0);
	check_scale(0, -INFINITY, 0);
	check_scale(1, FLT_MAX, INT_MAX);
	check_scale(-1, FLT_MAX, INT_MIN);
	check_scale(1, -FLT_MAX, INT_MIN);
	check_scale(INT_MAX, FLT_MAX, INT_MAX);
	/* The product overflows to inf. */
	check_scale(INT_MIN, FLT_MAX, INT_MIN);
	check_scale(INT_MIN, -FLT_MAX, INT_MAX);
	check_scale(1000000000, 4000.0f, INT_MAX);
	check_scale(-1000000000, 4000.0f, INT_MIN);
	check_scale(2, 1e30f, INT_MAX);
}

/* At INT_MIN and INT_MAX. (float)INT_MAX is 2^31, so INT_MAX x 1 is at the
 * upper edge, and -2^31 is the lower edge, both included. */
static void scale_crop_int_edges(void **state)
{
	(void)state;

	check_scale(INT_MAX, 1.0f, INT_MAX);
	check_scale(INT_MIN, 1.0f, INT_MIN);
	check_scale(INT_MIN, -1.0f, INT_MAX);
	check_scale(INT_MAX, -1.0f, INT_MIN);
	check_scale(INT_MAX, 0.5f, 1073741824);
	check_scale(INT_MIN, 0.5f, -1073741824);
	check_scale(INT_MAX, 0.0f, 0);
	check_scale(INT_MIN, 0.0f, 0);
	/* The largest float below 2^31, on both sides. */
	check_scale(1, 2147483520.0f, 2147483520);
	check_scale(-1, 2147483520.0f, -2147483520);
	check_scale(1, 2147483648.0f, INT_MAX);
	check_scale(-1, 2147483648.0f, INT_MIN);
	check_scale(INT_MAX, 0.99999994f, 2147483520);
	check_scale(INT_MIN, 0.99999994f, -2147483520);
	check_scale(INT_MAX, 1.00000012f, INT_MAX);
	check_scale(INT_MIN, 1.00000012f, INT_MIN);
}

/* The offset is the 64-bit sum as a float, rounded to the nearest. */
static void crop_offset_sums(void **state)
{
	(void)state;

	check_offset(0, 0, 0.0f);
	check_offset(10, 5, 15.0f);
	check_offset(-10, 5, -5.0f);
	check_offset(INT_MAX, 0, 2147483648.0f);
	check_offset(INT_MAX, 1, 2147483648.0f);
	check_offset(INT_MAX, INT_MAX, 4294967294.0f);
	check_offset(INT_MIN, 0, -2147483648.0f);
	check_offset(INT_MIN, INT_MIN, -4294967296.0f);
	check_offset(INT_MAX, INT_MIN, -1.0f);
	/* Rounded: past 2^24 not every int is a float. */
	check_offset(16777217, 0, 16777216.0f);
	check_offset(16777219, 0, 16777220.0f);
	check_offset(16777216, 1, 16777216.0f);
}

/* A negative crop is 0. */
static void clamp_crop_values(void **state)
{
	(void)state;

	check_clamp(0, 0);
	check_clamp(5, 5);
	check_clamp(INT_MAX, INT_MAX);
	check_clamp(-1, 0);
	check_clamp(-100, 0);
	check_clamp(INT_MIN, 0);
}

/* ------------------------------------------------------------------------ */
/* Through the real libobs. */

/* A source of a fixed size, from its settings, that draws nothing. */
struct box {
	volatile long cx;
	volatile long cy;
};

static const char *box_name(void *type_data)
{
	(void)type_data;
	return "box";
}

static void *box_create(obs_data_t *settings, obs_source_t *source)
{
	(void)source;
	struct box *box = bzalloc(sizeof(*box));
	box->cx = (long)obs_data_get_int(settings, "cx");
	box->cy = (long)obs_data_get_int(settings, "cy");
	return box;
}

static void box_destroy(void *data)
{
	bfree(data);
}

static uint32_t box_width(void *data)
{
	struct box *box = data;
	return (uint32_t)os_atomic_load_long(&box->cx);
}

static uint32_t box_height(void *data)
{
	struct box *box = data;
	return (uint32_t)os_atomic_load_long(&box->cy);
}

static void box_render(void *data, gs_effect_t *effect)
{
	(void)data;
	(void)effect;
}

/* Waits until the video thread has rendered the output scene (which applies
 * pending transforms and rescales nested scene crops) a few times. */
static void wait_for_ticks(void)
{
	uint32_t start = obs_get_total_frames();
	for (int i = 0; i < 500 && obs_get_total_frames() - start < 3; i++)
		os_sleep_ms(10);
}

static obs_source_t *create_box(const char *name, int cx, int cy)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_int(settings, "cx", cx);
	obs_data_set_int(settings, "cy", cy);
	obs_source_t *box = obs_source_create(BOX_ID, name, settings, NULL);
	obs_data_release(settings);
	assert_non_null(box);
	return box;
}

/* A scene of its own size, as a scene collection gives it. */
static void set_scene_size(obs_source_t *scene, int cx, int cy)
{
	obs_data_t *settings = obs_source_get_settings(scene);
	obs_data_set_bool(settings, "custom_size", true);
	obs_data_set_int(settings, "cx", cx);
	obs_data_set_int(settings, "cy", cy);
	obs_data_release(settings);
	obs_source_load(scene);
}

static void set_crop(obs_sceneitem_t *item, int left, int top, int right, int bottom)
{
	struct obs_sceneitem_crop crop = {left, top, right, bottom};
	obs_sceneitem_set_crop(item, &crop);
}

/* Prints the crop and the drawn size, and returns the number of values
 * that differ from the expected ones. */
static int check_item(const char *what, obs_sceneitem_t *item, int left, int top, int right, int bottom, float cx,
		      float cy)
{
	struct obs_sceneitem_crop crop;
	struct vec2 size;
	obs_sceneitem_get_crop(item, &crop);
	obs_sceneitem_get_box_scale(item, &size);

	int wrong = 0;
	if (crop.left != left || crop.top != top || crop.right != right || crop.bottom != bottom) {
		fprintf(stderr, "%s: crop is %d, %d, %d, %d (left, top, right, bottom), expected %d, %d, %d, %d\n",
			what, crop.left, crop.top, crop.right, crop.bottom, left, top, right, bottom);
		wrong++;
	}
	if (size.x != cx || size.y != cy) {
		fprintf(stderr, "%s: the item is drawn %g x %g, expected %g x %g\n", what, size.x, size.y, cx, cy);
		wrong++;
	}
	return wrong;
}

/* The setter clamps each negative side to 0 and keeps the others; the item
 * is drawn from the clamped crop. */
static void setter_clamps_negative_crops(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_scene_t *scene = obs_scene_create("clamp scene");
	obs_source_t *box = create_box("clamp box", 200, 100);
	obs_sceneitem_t *item = obs_scene_add(scene, box);
	obs_set_output_source(0, obs_scene_get_source(scene));
	wait_for_ticks();

	int wrong = 0;
	set_crop(item, -100, -1, INT_MIN, 5);
	wait_for_ticks();
	wrong += check_item("negative sides", item, 0, 0, 0, 5, 200.0f, 95.0f);

	set_crop(item, 10, INT_MIN, -1, -2147483647);
	wait_for_ticks();
	wrong += check_item("other negative sides", item, 10, 0, 0, 0, 190.0f, 100.0f);

	/* Equal after the clamp, so nothing changes. */
	set_crop(item, 10, -7, 0, 0);
	wait_for_ticks();
	wrong += check_item("negative again", item, 10, 0, 0, 0, 190.0f, 100.0f);

	set_crop(item, 10, 20, 30, 40);
	wait_for_ticks();
	wrong += check_item("in range", item, 10, 20, 30, 40, 160.0f, 40.0f);

	set_crop(item, 0, 0, 0, 0);
	wait_for_ticks();
	wrong += check_item("none", item, 0, 0, 0, 0, 200.0f, 100.0f);

	obs_set_output_source(0, NULL);
	obs_sceneitem_remove(item);
	obs_source_release(box);
	obs_scene_release(scene);
	wait_for_ticks();
	assert_int_equal(wrong, 0);
}

/* Through the setter: a crop that sums to the size draws the item 0 wide;
 * one more pixel, or a sum past the int range, draws it 2 px. */
static void setter_crop_sums(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_scene_t *scene = obs_scene_create("sum scene");
	obs_source_t *box = create_box("sum box", 200, 100);
	obs_sceneitem_t *item = obs_scene_add(scene, box);
	obs_set_output_source(0, obs_scene_get_source(scene));
	wait_for_ticks();

	int wrong = 0;
	set_crop(item, 100, 0, 100, 0);
	wait_for_ticks();
	wrong += check_item("sum equal to the width", item, 100, 0, 100, 0, 0.0f, 100.0f);

	set_crop(item, 100, 40, 101, 60);
	wait_for_ticks();
	wrong += check_item("one over the width, equal to the height", item, 100, 40, 101, 60, 2.0f, 0.0f);

	set_crop(item, 0, 100, 0, 1);
	wait_for_ticks();
	wrong += check_item("one over the height", item, 0, 100, 0, 1, 200.0f, 2.0f);

	set_crop(item, INT_MAX, INT_MAX, INT_MAX, INT_MAX);
	wait_for_ticks();
	wrong += check_item("sums past the int range", item, INT_MAX, INT_MAX, INT_MAX, INT_MAX, 2.0f, 2.0f);

	obs_set_output_source(0, NULL);
	obs_sceneitem_remove(item);
	obs_source_release(box);
	obs_scene_release(scene);
	wait_for_ticks();
	assert_int_equal(wrong, 0);
}

/* A nested scene grows and shrinks: its item's crop is rescaled by the
 * factor of the change, truncated, each time it is rendered at a new size.
 * Characterized, not endorsed: the truncation is lossy, so growing then
 * shrinking back does not give the crop back, and a crop saturated at
 * INT_MAX does not shrink back to the crop it came from. */
static void nested_scene_grows_and_shrinks(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_scene_t *inner_scene = obs_scene_create("inner crop scene");
	obs_source_t *inner = obs_scene_get_source(inner_scene);
	set_scene_size(inner, 100, 100);
	obs_scene_t *outer = obs_scene_create("outer crop scene");
	obs_sceneitem_t *item = obs_scene_add(outer, inner);
	obs_set_output_source(0, obs_scene_get_source(outer));
	wait_for_ticks();

	int wrong = 0;
	set_crop(item, 10, 20, 30, 40);
	wait_for_ticks();
	wrong += check_item("100x100", item, 10, 20, 30, 40, 60.0f, 40.0f);

	/* x 4 and x 2. */
	set_scene_size(inner, 400, 200);
	wait_for_ticks();
	wrong += check_item("grown to 400x200", item, 40, 40, 120, 80, 240.0f, 80.0f);

	/* x 0.25 on both. */
	set_scene_size(inner, 100, 50);
	wait_for_ticks();
	wrong += check_item("shrunk to 100x50", item, 10, 10, 30, 20, 60.0f, 20.0f);

	/* x 1.5 and x 3, exact. */
	set_scene_size(inner, 150, 150);
	wait_for_ticks();
	wrong += check_item("grown to 150x150", item, 15, 30, 45, 60, 90.0f, 60.0f);

	/* x 2/3 is 0.6666667 in float, a little above, so 15, 45, 30 and 60
	 * give 10, 30, 20 and 40 back. */
	set_scene_size(inner, 100, 100);
	wait_for_ticks();
	wrong += check_item("back to 100x100", item, 10, 20, 30, 40, 60.0f, 40.0f);

	/* x 0.25 and back x 4: 10 x 0.25 is 2, which is 8 back, not 10; 30 is
	 * 7, then 28. 20 and 40 come back. */
	set_scene_size(inner, 25, 25);
	wait_for_ticks();
	wrong += check_item("shrunk to 25x25", item, 2, 5, 7, 10, 16.0f, 10.0f);
	set_scene_size(inner, 100, 100);
	wait_for_ticks();
	wrong += check_item("back to 100x100 again", item, 8, 20, 28, 40, 64.0f, 40.0f);

	/* Saturates at INT_MAX growing; shrinking back starts from INT_MAX. */
	set_crop(item, 1000000000, 0, 0, 0);
	wait_for_ticks();
	set_scene_size(inner, 400000, 100);
	wait_for_ticks();
	wrong += check_item("grown past the int range", item, INT_MAX, 0, 0, 0, 2.0f, 100.0f);
	set_scene_size(inner, 100, 100);
	wait_for_ticks();
	wrong += check_item("shrunk from INT_MAX", item, 536870, 0, 0, 0, 2.0f, 100.0f);

	obs_set_output_source(0, NULL);
	obs_sceneitem_remove(item);
	obs_scene_release(outer);
	obs_scene_release(inner_scene);
	wait_for_ticks();
	assert_int_equal(wrong, 0);
}

static int setup(void **state)
{
	(void)state;
	/* obs_startup() needs a display on Linux (run under xvfb-run); the
	 * real-libobs cases skip without one. */
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		fprintf(stderr, "real-libobs cases skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 60,
		.fps_den = 1,
		.base_width = CANVAS_CX,
		.base_height = CANVAS_CY,
		.output_width = CANVAS_CX,
		.output_height = CANVAS_CY,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	int ret = obs_reset_video(&ovi);
	if (ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "real-libobs cases skipped: obs_reset_video failed: %d\n", ret);
		obs_shutdown();
		obs_up = false;
		return 0;
	}

	struct obs_source_info info = {
		.id = BOX_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_CUSTOM_DRAW,
		.get_name = box_name,
		.create = box_create,
		.destroy = box_destroy,
		.get_width = box_width,
		.get_height = box_height,
		.video_render = box_render,
	};
	obs_register_source(&info);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(cropped_size_at_the_size),
		cmocka_unit_test(cropped_size_past_the_int_range),
		cmocka_unit_test(cropped_size_negative),
		cmocka_unit_test(scale_crop_truncates),
		cmocka_unit_test(scale_crop_nan_inf_huge),
		cmocka_unit_test(scale_crop_int_edges),
		cmocka_unit_test(crop_offset_sums),
		cmocka_unit_test(clamp_crop_values),
		cmocka_unit_test(setter_clamps_negative_crops),
		cmocka_unit_test(setter_crop_sums),
		cmocka_unit_test(nested_scene_grows_and_shrinks),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
