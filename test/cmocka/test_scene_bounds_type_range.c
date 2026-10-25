/* Regression test for obs-rust/obs-studio#2281: a scene item bounds type
 * outside OBS_BOUNDS_NONE..OBS_BOUNDS_MAX_ONLY was stored as it came.
 *
 * compute_item_transform() treats any bounds type but OBS_BOUNDS_NONE as
 * bounded, and obs_bounds_fit_scale() leaves the scale alone for a type it
 * does not know, so such an item kept its own scale but was placed inside a
 * box the size of its bounds, with that box's alignment: it drew somewhere
 * no valid bounds type puts it, and the transform dialog showed no bounds
 * type for it.
 *
 * A bounds type comes in four ways, each tested here through the real
 * libobs:
 *   - a scene collection (scene_load_item(), through obs_load_source() and
 *     obs_source_load2(), as obs_load_sources() does);
 *   - frontend undo data (load_transform_states(), through
 *     obs_scene_load_transform_states());
 *   - obs_sceneitem_set_bounds_type();
 *   - obs_sceneitem_set_info2().
 *
 * A value outside the enum must read back as OBS_BOUNDS_NONE and the item
 * must draw exactly like the same item with OBS_BOUNDS_NONE (draw and box
 * matrices compared bit for bit). Every valid value must read back
 * unchanged and draw like the same item set to that value directly.
 *
 * The items sit in a scene (not a group), so the setters update the
 * transform at once; a loaded scene is ticked first. Needs core video (run
 * under xvfb-run); the cases skip without a display. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <graphics/matrix4.h>
#include <graphics/vec4.h>
#include <util/platform.h>

#define BOX_ID "scene_bounds_type_range_box"
#define CANVAS_CX 1280
#define CANVAS_CY 720

static bool obs_up;

struct box {
	uint32_t cx;
	uint32_t cy;
};

static const char *box_name(void *type_data)
{
	(void)type_data;
	return "box";
}

static void *box_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	(void)source;
	struct box *box = bzalloc(sizeof(*box));
	box->cx = 200;
	box->cy = 100;
	return box;
}

static void box_destroy(void *data)
{
	bfree(data);
}

static uint32_t box_width(void *data)
{
	return ((struct box *)data)->cx;
}

static uint32_t box_height(void *data)
{
	return ((struct box *)data)->cy;
}

static void box_render(void *data, gs_effect_t *effect)
{
	(void)data;
	(void)effect;
}

/* Waits until the video thread has ticked the scenes a few times. */
static void wait_for_ticks(void)
{
	uint32_t start = obs_get_total_frames();
	for (int i = 0; i < 500 && obs_get_total_frames() - start < 3; i++)
		os_sleep_ms(10);
}

/* A scene with two items over the same 200x100 source: the subject, whose
 * bounds type each case sets, and the reference, set directly to the type
 * the subject should end up with. Both have a scale of 1.5, top-left
 * alignment, and an 800x600 bounding box aligned to its centre, so any
 * bounding box moves the item. */
struct fixture {
	obs_source_t *box;
	obs_scene_t *scene;
	obs_sceneitem_t *subject;
	obs_sceneitem_t *reference;
};

static void configure(obs_sceneitem_t *item)
{
	struct vec2 pos;
	struct vec2 scale;
	struct vec2 bounds;
	vec2_set(&pos, 100.0f, 50.0f);
	vec2_set(&scale, 1.5f, 1.5f);
	vec2_set(&bounds, 800.0f, 600.0f);

	obs_sceneitem_set_pos(item, &pos);
	obs_sceneitem_set_scale(item, &scale);
	obs_sceneitem_set_alignment(item, OBS_ALIGN_LEFT | OBS_ALIGN_TOP);
	obs_sceneitem_set_bounds(item, &bounds);
	obs_sceneitem_set_bounds_alignment(item, OBS_ALIGN_CENTER);
	obs_sceneitem_set_bounds_type(item, OBS_BOUNDS_NONE);
}

static void fixture_create(struct fixture *f)
{
	f->box = obs_source_create(BOX_ID, "bounds range box", NULL, NULL);
	assert_non_null(f->box);
	f->scene = obs_scene_create("bounds range scene");
	assert_non_null(f->scene);
	f->subject = obs_scene_add(f->scene, f->box);
	f->reference = obs_scene_add(f->scene, f->box);
	assert_non_null(f->subject);
	assert_non_null(f->reference);
	configure(f->subject);
	configure(f->reference);
	wait_for_ticks();
}

static void fixture_destroy(struct fixture *f)
{
	obs_sceneitem_remove(f->subject);
	obs_sceneitem_remove(f->reference);
	obs_source_remove(obs_scene_get_source(f->scene));
	obs_scene_release(f->scene);
	obs_source_release(f->box);
	wait_for_ticks();
}

static bool is_valid(int64_t value)
{
	return value >= OBS_BOUNDS_NONE && value <= OBS_BOUNDS_MAX_ONLY;
}

static enum obs_bounds_type expected_type(int64_t value)
{
	return is_valid(value) ? (enum obs_bounds_type)value : OBS_BOUNDS_NONE;
}

/* The item's bounds type and both of its matrices, bit for bit, against the
 * reference set to the expected type. Returns the number of mismatches. */
static int check_item(const char *path, int64_t value, struct fixture *f, obs_sceneitem_t *item)
{
	enum obs_bounds_type want = expected_type(value);
	obs_sceneitem_set_bounds_type(f->reference, want);

	int wrong = 0;
	enum obs_bounds_type got = obs_sceneitem_get_bounds_type(item);
	if (got != want) {
		fprintf(stderr, "%s, bounds type %lld: reads back as %lld, expected %d\n", path, (long long)value,
			(long long)got, (int)want);
		wrong++;
	}

	struct obs_transform_info info;
	obs_sceneitem_get_info2(item, &info);
	if (info.bounds_type != want) {
		fprintf(stderr, "%s, bounds type %lld: get_info2 gives %lld, expected %d\n", path, (long long)value,
			(long long)info.bounds_type, (int)want);
		wrong++;
	}

	struct matrix4 got_draw, want_draw, got_box, want_box;
	obs_sceneitem_get_draw_transform(item, &got_draw);
	obs_sceneitem_get_draw_transform(f->reference, &want_draw);
	obs_sceneitem_get_box_transform(item, &got_box);
	obs_sceneitem_get_box_transform(f->reference, &want_box);

	if (memcmp(&got_draw, &want_draw, sizeof(got_draw)) != 0) {
		fprintf(stderr,
			"%s, bounds type %lld: drawn at (%g, %g) scale (%g, %g), "
			"expected (%g, %g) scale (%g, %g) like bounds type %d\n",
			path, (long long)value, got_draw.t.x, got_draw.t.y, got_draw.x.x, got_draw.y.y, want_draw.t.x,
			want_draw.t.y, want_draw.x.x, want_draw.y.y, (int)want);
		wrong++;
	}
	if (memcmp(&got_box, &want_box, sizeof(got_box)) != 0) {
		fprintf(stderr,
			"%s, bounds type %lld: box at (%g, %g) size (%g, %g), "
			"expected (%g, %g) size (%g, %g) like bounds type %d\n",
			path, (long long)value, got_box.t.x, got_box.t.y, got_box.x.x, got_box.y.y, want_box.t.x,
			want_box.t.y, want_box.x.x, want_box.y.y, (int)want);
		wrong++;
	}
	return wrong;
}

/* Every valid value, then values outside the enum: just past each end, far
 * past, and an int64 that truncates to a valid value (2) when cast to int. */
static const int64_t load_values[] = {
	OBS_BOUNDS_NONE,
	OBS_BOUNDS_STRETCH,
	OBS_BOUNDS_SCALE_INNER,
	OBS_BOUNDS_SCALE_OUTER,
	OBS_BOUNDS_SCALE_TO_WIDTH,
	OBS_BOUNDS_SCALE_TO_HEIGHT,
	OBS_BOUNDS_MAX_ONLY,
	OBS_BOUNDS_MAX_ONLY + 1,
	99,
	-1,
	INT_MAX,
	INT_MIN,
	(int64_t)UINT32_MAX + 3,
	INT64_MIN,
};

/* What a C caller can pass in the enum. */
static const int setter_values[] = {
	OBS_BOUNDS_NONE,
	OBS_BOUNDS_STRETCH,
	OBS_BOUNDS_SCALE_INNER,
	OBS_BOUNDS_SCALE_OUTER,
	OBS_BOUNDS_SCALE_TO_WIDTH,
	OBS_BOUNDS_SCALE_TO_HEIGHT,
	OBS_BOUNDS_MAX_ONLY,
	OBS_BOUNDS_MAX_ONLY + 1,
	99,
	-1,
	INT_MAX,
	INT_MIN,
};

#define COUNT(a) (sizeof(a) / sizeof((a)[0]))

/* The comparison has teeth: with this item, each bounds type but NONE
 * draws somewhere else than NONE. */
static void valid_types_draw_differently(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct fixture f;
	fixture_create(&f);

	struct matrix4 none_draw;
	obs_sceneitem_get_draw_transform(f.reference, &none_draw);

	int wrong = 0;
	for (int type = OBS_BOUNDS_STRETCH; type <= OBS_BOUNDS_MAX_ONLY; type++) {
		struct matrix4 draw;
		obs_sceneitem_set_bounds_type(f.subject, (enum obs_bounds_type)type);
		obs_sceneitem_get_draw_transform(f.subject, &draw);
		if (memcmp(&draw, &none_draw, sizeof(draw)) == 0) {
			fprintf(stderr, "bounds type %d draws like NONE; the test cannot tell them apart\n", type);
			wrong++;
		}
	}

	fixture_destroy(&f);
	assert_int_equal(wrong, 0);
}

static void set_bounds_type_range(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct fixture f;
	fixture_create(&f);

	int wrong = 0;
	for (size_t i = 0; i < COUNT(setter_values); i++) {
		obs_sceneitem_set_bounds_type(f.subject, OBS_BOUNDS_STRETCH);
		obs_sceneitem_set_bounds_type(f.subject, (enum obs_bounds_type)setter_values[i]);
		wrong += check_item("obs_sceneitem_set_bounds_type", setter_values[i], &f, f.subject);
	}

	fixture_destroy(&f);
	assert_int_equal(wrong, 0);
}

static void set_info2_range(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct fixture f;
	fixture_create(&f);

	int wrong = 0;
	for (size_t i = 0; i < COUNT(setter_values); i++) {
		obs_sceneitem_set_bounds_type(f.subject, OBS_BOUNDS_STRETCH);
		struct obs_transform_info info;
		obs_sceneitem_get_info2(f.subject, &info);
		info.bounds_type = (enum obs_bounds_type)setter_values[i];
		obs_sceneitem_set_info2(f.subject, &info);
		wrong += check_item("obs_sceneitem_set_info2", setter_values[i], &f, f.subject);
	}

	fixture_destroy(&f);
	assert_int_equal(wrong, 0);
}

/* The frontend's undo data: obs_scene_save_transform_states() with the
 * subject's bounds type replaced, loaded back. */
static void undo_transform_states_range(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct fixture f;
	fixture_create(&f);
	int64_t subject_id = obs_sceneitem_get_id(f.subject);

	int wrong = 0;
	for (size_t i = 0; i < COUNT(load_values); i++) {
		obs_sceneitem_set_bounds_type(f.subject, OBS_BOUNDS_STRETCH);
		obs_data_t *states = obs_scene_save_transform_states(f.scene, true);
		obs_data_array_t *scenes = obs_data_get_array(states, "scenes_and_groups");
		obs_data_t *scene_data = obs_data_array_item(scenes, 0);
		obs_data_array_t *items = obs_data_get_array(scene_data, "items");

		bool found = false;
		for (size_t j = 0; j < obs_data_array_count(items); j++) {
			obs_data_t *item_data = obs_data_array_item(items, j);
			if (obs_data_get_int(item_data, "id") == subject_id) {
				obs_data_set_int(item_data, "bounds_type", load_values[i]);
				found = true;
			}
			obs_data_release(item_data);
		}
		assert_true(found);

		obs_scene_load_transform_states(obs_data_get_json(states));
		wrong += check_item("obs_scene_load_transform_states", load_values[i], &f, f.subject);

		obs_data_array_release(items);
		obs_data_release(scene_data);
		obs_data_array_release(scenes);
		obs_data_release(states);
	}

	fixture_destroy(&f);
	assert_int_equal(wrong, 0);
}

/* A scene collection: the scene saved with the subject's bounds type
 * replaced, loaded as a new scene, and its copy of the subject compared
 * with the reference in the original scene (same canvas, same source). */
static void scene_load_range(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct fixture f;
	fixture_create(&f);
	int64_t subject_id = obs_sceneitem_get_id(f.subject);

	int wrong = 0;
	for (size_t i = 0; i < COUNT(load_values); i++) {
		obs_sceneitem_set_bounds_type(f.subject, OBS_BOUNDS_STRETCH);
		obs_data_t *saved = obs_save_source(obs_scene_get_source(f.scene));
		assert_non_null(saved);
		obs_data_erase(saved, "uuid");
		char name[64];
		snprintf(name, sizeof(name), "bounds range loaded %zu", i);
		obs_data_set_string(saved, "name", name);

		obs_data_t *settings = obs_data_get_obj(saved, "settings");
		obs_data_array_t *items = obs_data_get_array(settings, "items");
		bool found = false;
		for (size_t j = 0; j < obs_data_array_count(items); j++) {
			obs_data_t *item_data = obs_data_array_item(items, j);
			if (obs_data_get_int(item_data, "id") == subject_id) {
				obs_data_set_int(item_data, "bounds_type", load_values[i]);
				found = true;
			}
			obs_data_release(item_data);
		}
		assert_true(found);

		/* As obs_load_sources() does: create, then load the items. */
		obs_source_t *loaded = obs_load_source(saved);
		assert_non_null(loaded);
		obs_source_load2(loaded);
		obs_scene_t *loaded_scene = obs_scene_from_source(loaded);
		assert_non_null(loaded_scene);
		obs_sceneitem_t *item = obs_scene_find_sceneitem_by_id(loaded_scene, subject_id);
		assert_non_null(item);
		wait_for_ticks();
		obs_sceneitem_force_update_transform(item);

		wrong += check_item("scene_load_item", load_values[i], &f, item);

		obs_source_remove(loaded);
		obs_source_release(loaded);
		obs_data_array_release(items);
		obs_data_release(settings);
		obs_data_release(saved);
	}

	fixture_destroy(&f);
	assert_int_equal(wrong, 0);
}

static int setup(void **state)
{
	(void)state;
	/* obs_startup() needs a display on Linux (run under xvfb-run); the
	 * cases skip without one. */
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
		cmocka_unit_test(valid_types_draw_differently),
		cmocka_unit_test(set_bounds_type_range),
		cmocka_unit_test(set_info2_range),
		cmocka_unit_test(undo_transform_states_range),
		cmocka_unit_test(scene_load_range),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
