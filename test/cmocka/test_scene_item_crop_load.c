/* Scene item crops and group sizes that come from a scene collection out of
 * range.
 *
 * obs_sceneitem_set_crop() clamps a negative crop to 0, but loading a scene
 * collection stored the crop unclamped, so a hand-edited or generated
 * collection with "crop_left": -100 drew the item 2 px wide. A crop past
 * the int range came back with the other sign. The sizes derived from the
 * crop and from a group's content were plain C casts and sums, undefined
 * when out of range: a nested scene that grew turned a big crop negative
 * (and the item 2 px wide), and a group whose content spans more than 2^32
 * px got a size of 0.
 *
 * Each case loads sources and scenes from a JSON string with
 * obs_load_sources(), as a scene collection is loaded, lets the video thread
 * tick, and checks the crop (obs_sceneitem_get_crop), the item's size
 * (obs_sceneitem_get_box_scale, scale 1) and the group's size. The sum and
 * the casts on their own are in test_scene_crop_steps, built with UBSan. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <obs.h>
#include <graphics/vec2.h>
#include <util/platform.h>
#include <util/threading.h>

#define BOX_ID "scene_crop_load_box"
#define CANVAS_CX 1280
#define CANVAS_CY 720

static bool obs_up;

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
 * pending transforms and resizes groups) a few times. */
static void wait_for_ticks(void)
{
	uint32_t start = obs_get_total_frames();
	for (int i = 0; i < 500 && obs_get_total_frames() - start < 3; i++)
		os_sleep_ms(10);
}

#define MAX_LOADED 8

struct loaded {
	obs_source_t *sources[MAX_LOADED];
	size_t count;
};

static void keep_source(void *param, obs_source_t *source)
{
	struct loaded *loaded = param;
	if (loaded->count < MAX_LOADED)
		loaded->sources[loaded->count++] = obs_source_get_ref(source);
}

/* Loads `json` ({"sources": [...]}) the way a scene collection is loaded and
 * shows the scene named `output` on channel 0. */
static struct loaded load_collection(const char *json, const char *output)
{
	struct loaded loaded = {0};
	obs_data_t *data = obs_data_create_from_json(json);
	assert_non_null(data);
	obs_data_array_t *sources = obs_data_get_array(data, "sources");
	assert_non_null(sources);
	obs_load_sources(sources, keep_source, &loaded);
	obs_data_array_release(sources);
	obs_data_release(data);

	obs_source_t *scene = obs_get_source_by_name(output);
	assert_non_null(scene);
	obs_set_output_source(0, scene);
	obs_source_release(scene);
	wait_for_ticks();
	return loaded;
}

static void unload(struct loaded *loaded)
{
	obs_set_output_source(0, NULL);
	for (size_t i = 0; i < loaded->count; i++) {
		obs_source_remove(loaded->sources[i]);
		obs_source_release(loaded->sources[i]);
	}
	wait_for_ticks();
}

static obs_sceneitem_t *find_item(const char *scene_name, const char *source_name)
{
	obs_source_t *source = obs_get_source_by_name(scene_name);
	assert_non_null(source);
	obs_scene_t *scene = obs_scene_from_source(source);
	if (!scene)
		scene = obs_group_from_source(source);
	assert_non_null(scene);
	obs_sceneitem_t *item = obs_scene_find_source(scene, source_name);
	obs_source_release(source);
	assert_non_null(item);
	return item;
}

/* Prints the crop and the drawn size, and returns the number of values
 * that differ from the expected ones. */
static int check_item(const char *what, obs_sceneitem_t *item, const struct obs_sceneitem_crop *want_crop,
		      float want_cx, float want_cy)
{
	struct obs_sceneitem_crop crop;
	struct vec2 size;
	obs_sceneitem_get_crop(item, &crop);
	obs_sceneitem_get_box_scale(item, &size);

	int wrong = 0;
	if (crop.left != want_crop->left || crop.top != want_crop->top || crop.right != want_crop->right ||
	    crop.bottom != want_crop->bottom) {
		fprintf(stderr, "%s: crop is %d, %d, %d, %d (left, top, right, bottom), expected %d, %d, %d, %d\n",
			what, crop.left, crop.top, crop.right, crop.bottom, want_crop->left, want_crop->top,
			want_crop->right, want_crop->bottom);
		wrong++;
	}
	if (size.x != want_cx || size.y != want_cy) {
		fprintf(stderr, "%s: the item is drawn %g x %g, expected %g x %g\n", what, size.x, size.y, want_cx,
			want_cy);
		wrong++;
	}
	return wrong;
}

/* ------------------------------------------------------------------------ */

/* A negative crop on load is 0, as obs_sceneitem_set_crop() makes it: the
 * 200x100 box is drawn whole, not 2 px wide and 2 px high. */
static void negative_crop_is_clamped_on_load(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct loaded loaded = load_collection(
		"{\"sources\": ["
		"{\"id\": \"" BOX_ID "\", \"name\": \"neg box\", \"settings\": {\"cx\": 200, \"cy\": 100}},"
		"{\"id\": \"scene\", \"name\": \"neg scene\", \"settings\": {\"items\": ["
		"{\"name\": \"neg box\", \"visible\": true, \"pos\": {\"x\": 0, \"y\": 0},"
		" \"scale\": {\"x\": 1, \"y\": 1},"
		" \"crop_left\": -100, \"crop_top\": -1, \"crop_right\": -2147483648, \"crop_bottom\": 0}"
		"]}}]}",
		"neg scene");

	struct obs_sceneitem_crop want = {0, 0, 0, 0};
	int wrong = check_item("negative crop", find_item("neg scene", "neg box"), &want, 200.0f, 100.0f);
	unload(&loaded);
	assert_int_equal(wrong, 0);
}

/* A crop past the int range (only a generated file has one) is the nearest
 * int, then clamped: 3000000000 is INT_MAX (larger than the box, so 2 px
 * wide, as any crop larger than the source is), -3000000000 is 0. Before,
 * they came back as -1294967296 and 1294967296. */
static void out_of_range_crop_saturates_on_load(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct loaded loaded = load_collection(
		"{\"sources\": ["
		"{\"id\": \"" BOX_ID "\", \"name\": \"huge box\", \"settings\": {\"cx\": 200, \"cy\": 100}},"
		"{\"id\": \"scene\", \"name\": \"huge scene\", \"settings\": {\"items\": ["
		"{\"name\": \"huge box\", \"visible\": true, \"pos\": {\"x\": 0, \"y\": 0},"
		" \"scale\": {\"x\": 1, \"y\": 1},"
		" \"crop_left\": 3000000000, \"crop_top\": -3000000000, \"crop_right\": 0, \"crop_bottom\": 0}"
		"]}}]}",
		"huge scene");

	struct obs_sceneitem_crop want = {INT_MAX, 0, 0, 0};
	int wrong = check_item("crop past the int range", find_item("huge scene", "huge box"), &want, 2.0f, 100.0f);
	unload(&loaded);
	assert_int_equal(wrong, 0);
}

/* Crops in range whose sum is past the int range: the sum is larger than the
 * box, so it is drawn 2 px wide and high, and the crops are kept. */
static void crop_sum_past_int_range(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct loaded loaded = load_collection(
		"{\"sources\": ["
		"{\"id\": \"" BOX_ID "\", \"name\": \"sum box\", \"settings\": {\"cx\": 200, \"cy\": 100}},"
		"{\"id\": \"scene\", \"name\": \"sum scene\", \"settings\": {\"items\": ["
		"{\"name\": \"sum box\", \"visible\": true, \"pos\": {\"x\": 0, \"y\": 0},"
		" \"scale\": {\"x\": 1, \"y\": 1},"
		" \"crop_left\": 2147483647, \"crop_top\": 2147483647,"
		" \"crop_right\": 2147483647, \"crop_bottom\": 2147483647}"
		"]}}]}",
		"sum scene");

	struct obs_sceneitem_crop want = {INT_MAX, INT_MAX, INT_MAX, INT_MAX};
	int wrong = check_item("crop sum past the int range", find_item("sum scene", "sum box"), &want, 2.0f, 2.0f);
	unload(&loaded);
	assert_int_equal(wrong, 0);
}

/* A nested scene grows from 1x1 to 4000x4000: its item's crop grows with
 * it. 1000000000 x 4000 is past the int range, so the crop is INT_MAX;
 * before, the cast made it INT_MIN, which drew the item 2 px wide. */
static void nested_scene_crop_saturates_when_it_grows(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct loaded loaded =
		load_collection("{\"sources\": ["
				"{\"id\": \"scene\", \"name\": \"inner scene\","
				" \"settings\": {\"custom_size\": true, \"cx\": 1, \"cy\": 1, \"items\": []}},"
				"{\"id\": \"scene\", \"name\": \"outer scene\", \"settings\": {\"items\": ["
				"{\"name\": \"inner scene\", \"visible\": true, \"pos\": {\"x\": 0, \"y\": 0},"
				" \"scale\": {\"x\": 1, \"y\": 1},"
				" \"crop_left\": 1000000000, \"crop_top\": 0, \"crop_right\": 0, \"crop_bottom\": 0}"
				"]}}]}",
				"outer scene");

	obs_sceneitem_t *item = find_item("outer scene", "inner scene");
	struct obs_sceneitem_crop want = {1000000000, 0, 0, 0};
	int wrong = check_item("nested scene, 1x1", item, &want, 2.0f, 1.0f);

	/* Load the inner scene again with its new size, as a scene collection
	 * reload does. */
	obs_source_t *inner = obs_get_source_by_name("inner scene");
	assert_non_null(inner);
	obs_data_t *settings = obs_source_get_settings(inner);
	obs_data_set_int(settings, "cx", 4000);
	obs_data_set_int(settings, "cy", 4000);
	obs_data_release(settings);
	obs_source_load(inner);
	obs_source_release(inner);
	wait_for_ticks();

	assert_int_equal(obs_source_get_width(obs_sceneitem_get_source(item)), 4000);
	want.left = INT_MAX;
	wrong += check_item("nested scene, grown to 4000x4000", item, &want, 2.0f, 4000.0f);
	unload(&loaded);
	assert_int_equal(wrong, 0);
}

/* A group whose children are 1e20 px apart: its size is past the uint32_t
 * range, so it is UINT32_MAX. Before, the cast made it 0 here. */
static void huge_group_size_saturates(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct loaded loaded = load_collection(
		"{\"sources\": ["
		"{\"id\": \"" BOX_ID "\", \"name\": \"near box\", \"settings\": {\"cx\": 200, \"cy\": 100}},"
		"{\"id\": \"" BOX_ID "\", \"name\": \"far box\", \"settings\": {\"cx\": 200, \"cy\": 100}},"
		"{\"id\": \"group\", \"name\": \"huge group\", \"settings\": {\"items\": ["
		"{\"name\": \"near box\", \"visible\": true, \"pos\": {\"x\": 0, \"y\": 0},"
		" \"scale\": {\"x\": 1, \"y\": 1}},"
		"{\"name\": \"far box\", \"visible\": true, \"pos\": {\"x\": 1e20, \"y\": 1e20},"
		" \"scale\": {\"x\": 1, \"y\": 1}}"
		"]}},"
		"{\"id\": \"scene\", \"name\": \"group scene\", \"settings\": {\"items\": ["
		"{\"name\": \"huge group\", \"visible\": true, \"pos\": {\"x\": 0, \"y\": 0},"
		" \"scale\": {\"x\": 1, \"y\": 1}}"
		"]}}]}",
		"group scene");

	/* Moving a child makes the group compute its size from its children. */
	obs_sceneitem_t *near_item = find_item("huge group", "near box");
	struct vec2 pos;
	vec2_set(&pos, 0.0f, 0.0f);
	obs_sceneitem_set_pos(near_item, &pos);
	wait_for_ticks();

	obs_source_t *group = obs_get_source_by_name("huge group");
	assert_non_null(group);
	uint32_t cx = obs_source_get_width(group);
	uint32_t cy = obs_source_get_height(group);
	obs_source_release(group);
	if (cx != UINT32_MAX || cy != UINT32_MAX)
		fprintf(stderr, "huge group: size is %u x %u, expected %u x %u\n", cx, cy, UINT32_MAX, UINT32_MAX);
	unload(&loaded);
	assert_int_equal(cx, UINT32_MAX);
	assert_int_equal(cy, UINT32_MAX);
}

/* Control: crops in range load as they are. */
static void crop_in_range_loads_as_is(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct loaded loaded = load_collection(
		"{\"sources\": ["
		"{\"id\": \"" BOX_ID "\", \"name\": \"ok box\", \"settings\": {\"cx\": 200, \"cy\": 100}},"
		"{\"id\": \"scene\", \"name\": \"ok scene\", \"settings\": {\"items\": ["
		"{\"name\": \"ok box\", \"visible\": true, \"pos\": {\"x\": 0, \"y\": 0},"
		" \"scale\": {\"x\": 1, \"y\": 1},"
		" \"crop_left\": 10, \"crop_top\": 20, \"crop_right\": 30, \"crop_bottom\": 40}"
		"]}}]}",
		"ok scene");

	struct obs_sceneitem_crop want = {10, 20, 30, 40};
	int wrong = check_item("crop in range", find_item("ok scene", "ok box"), &want, 160.0f, 40.0f);
	unload(&loaded);
	assert_int_equal(wrong, 0);
}

static int setup(void **state)
{
	(void)state;
	/* obs_startup() needs a display on Linux (run under xvfb-run); the
	 * cases skip without one. */
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
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
		fprintf(stderr, "skipped: obs_reset_video failed: %d\n", ret);
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
		cmocka_unit_test(negative_crop_is_clamped_on_load),
		cmocka_unit_test(out_of_range_crop_saturates_on_load),
		cmocka_unit_test(crop_sum_past_int_range),
		cmocka_unit_test(nested_scene_crop_saturates_when_it_grows),
		cmocka_unit_test(huge_group_size_saturates),
		cmocka_unit_test(crop_in_range_loads_as_is),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
