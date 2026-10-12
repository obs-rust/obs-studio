/* Regression test for obsproject/obs-studio#5478 (reported by Sbssa, with
 * Avasam and JDR-Ninja seeing the same) and obsproject/obs-studio#9558
 * (reported by timoschwarzer, confirmed by Fenrirthviti): Copy Transform and
 * Paste Transform do not work with items inside groups.
 *
 * Paste Transform walks the items of the current scene with
 * obs_scene_enum_items(), which does not descend into groups, so a selected
 * item inside a group is never pasted. Copy Transform reads the item's
 * transform with obs_sceneitem_get_info2(), which for an item inside a group
 * is relative to the group, so pasting it onto an item outside the group
 * puts that item somewhere else on the canvas.
 *
 * The test builds a scene with a moved, scaled and rotated group holding two
 * items, and two items outside it. It copies and pastes with the Qt-free
 * helpers the frontend calls, waits for the video thread to update the
 * group, and compares where the items are on the canvas. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <math.h>
#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <graphics/matrix4.h>
#include <util/platform.h>

#include "transform-clipboard.h"

#define BOX_ID "transform_clipboard_box"
#define TOLERANCE 0.5f

static bool obs_up;

static const char *box_name(void *type_data)
{
	(void)type_data;
	return "box";
}

static void *box_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void box_destroy(void *data)
{
	(void)data;
}

static uint32_t box_width(void *data)
{
	(void)data;
	return 200;
}

static uint32_t box_height(void *data)
{
	(void)data;
	return 100;
}

struct fixture {
	obs_scene_t *scene;
	obs_source_t *sources[4];
	obs_sceneitem_t *group;
	obs_sceneitem_t *in1; /* inside the group */
	obs_sceneitem_t *in2; /* inside the group */
	obs_sceneitem_t *out1;
	obs_sceneitem_t *out2;
};

/* Corners of an item's box on the canvas, in the order (0,0), (1,0), (0,1),
 * (1,1) of the box. */
static void canvas_corners(struct fixture *f, obs_sceneitem_t *item, struct vec3 corners[4])
{
	struct matrix4 box, draw;
	obs_sceneitem_t *group = obs_sceneitem_get_group(f->scene, item);

	obs_sceneitem_get_box_transform(item, &box);
	if (group)
		obs_sceneitem_get_draw_transform(group, &draw);

	for (int i = 0; i < 4; i++) {
		vec3_set(&corners[i], (float)(i & 1), (float)(i >> 1), 0.0f);
		vec3_transform(&corners[i], &corners[i], &box);
		if (group)
			vec3_transform(&corners[i], &corners[i], &draw);
	}
}

static float corners_distance(struct fixture *f, obs_sceneitem_t *a, obs_sceneitem_t *b)
{
	struct vec3 ca[4], cb[4];
	float max = 0.0f;

	canvas_corners(f, a, ca);
	canvas_corners(f, b, cb);
	for (int i = 0; i < 4; i++) {
		float d = vec3_dist(&ca[i], &cb[i]);
		if (d > max)
			max = d;
	}
	return max;
}

/* The video thread updates the transform of items inside a group and
 * resizes the group on its ticks. Waits until a matches b on the canvas, or
 * one second. */
static float wait_for_match(struct fixture *f, obs_sceneitem_t *a, obs_sceneitem_t *b)
{
	float d = corners_distance(f, a, b);
	for (int i = 0; i < 50 && d > TOLERANCE; i++) {
		os_sleep_ms(20);
		d = corners_distance(f, a, b);
	}
	/* Let a pending group resize land too, and check again. */
	os_sleep_ms(40);
	return corners_distance(f, a, b);
}

static void print_corners(struct fixture *f, const char *what, obs_sceneitem_t *item)
{
	struct vec3 c[4];
	canvas_corners(f, item, c);
	fprintf(stderr, "  %s: (%.1f, %.1f) (%.1f, %.1f) (%.1f, %.1f) (%.1f, %.1f)\n", what, c[0].x, c[0].y, c[1].x,
		c[1].y, c[2].x, c[2].y, c[3].x, c[3].y);
}

static obs_sceneitem_t *add_box(struct fixture *f, int idx, float x, float y, float scale, float rot)
{
	char name[32];
	snprintf(name, sizeof(name), "box %d", idx + 1);
	f->sources[idx] = obs_source_create(BOX_ID, name, NULL, NULL);
	assert_non_null(f->sources[idx]);

	obs_sceneitem_t *item = obs_scene_add(f->scene, f->sources[idx]);
	assert_non_null(item);

	struct vec2 v;
	vec2_set(&v, x, y);
	obs_sceneitem_set_pos(item, &v);
	vec2_set(&v, scale, scale);
	obs_sceneitem_set_scale(item, &v);
	obs_sceneitem_set_rot(item, rot);
	obs_sceneitem_set_alignment(item, OBS_ALIGN_TOP | OBS_ALIGN_LEFT);
	return item;
}

static void fixture_create(struct fixture *f)
{
	memset(f, 0, sizeof(*f));
	f->scene = obs_scene_create("transform clipboard");
	assert_non_null(f->scene);
	obs_set_output_source(0, obs_scene_get_source(f->scene));

	f->in1 = add_box(f, 0, 100.0f, 80.0f, 1.0f, 0.0f);
	f->in2 = add_box(f, 1, 400.0f, 300.0f, 0.5f, 0.0f);
	f->out1 = add_box(f, 2, 1200.0f, 700.0f, 1.5f, 10.0f);
	f->out2 = add_box(f, 3, 50.0f, 900.0f, 0.75f, 0.0f);

	struct obs_sceneitem_crop crop = {12, 4, 0, 8};
	obs_sceneitem_set_crop(f->in1, &crop);
	struct obs_sceneitem_crop crop2 = {0, 0, 30, 20};
	obs_sceneitem_set_crop(f->out1, &crop2);

	obs_sceneitem_t *items[] = {f->in1, f->in2};
	f->group = obs_scene_insert_group(f->scene, "group", items, 2);
	assert_non_null(f->group);
	os_sleep_ms(60);

	/* Move, scale and rotate the group, as a user arranging a layout
	 * would. */
	struct vec2 v;
	obs_sceneitem_set_alignment(f->group, OBS_ALIGN_TOP | OBS_ALIGN_LEFT);
	vec2_set(&v, 700.0f, 150.0f);
	obs_sceneitem_set_pos(f->group, &v);
	vec2_set(&v, 1.5f, 1.5f);
	obs_sceneitem_set_scale(f->group, &v);
	obs_sceneitem_set_rot(f->group, 30.0f);
	os_sleep_ms(60);
}

static void fixture_destroy(struct fixture *f)
{
	obs_set_output_source(0, NULL);
	obs_source_remove(obs_scene_get_source(f->scene));
	obs_scene_release(f->scene);
	for (int i = 0; i < 4; i++)
		obs_source_release(f->sources[i]);
}

static void select_only(struct fixture *f, obs_sceneitem_t *a, obs_sceneitem_t *b)
{
	obs_sceneitem_t *all[] = {f->group, f->in1, f->in2, f->out1, f->out2};
	for (size_t i = 0; i < sizeof(all) / sizeof(all[0]); i++)
		obs_sceneitem_select(all[i], all[i] == a || all[i] == b);
}

static void copy_paste(struct fixture *f, obs_sceneitem_t *from)
{
	struct obs_transform_info info;
	struct obs_sceneitem_crop crop;
	transform_clipboard_copy(f->scene, from, &info, &crop);
	transform_clipboard_paste(f->scene, &info, &crop);
}

static bool same_crop(obs_sceneitem_t *a, obs_sceneitem_t *b)
{
	struct obs_sceneitem_crop ca, cb;
	obs_sceneitem_get_crop(a, &ca);
	obs_sceneitem_get_crop(b, &cb);
	return ca.left == cb.left && ca.top == cb.top && ca.right == cb.right && ca.bottom == cb.bottom;
}

/* Checks that item ended up where from is on the canvas, with its crop. */
static bool check_lands_on(struct fixture *f, const char *what, obs_sceneitem_t *item, obs_sceneitem_t *from)
{
	float d = wait_for_match(f, item, from);
	bool crop_ok = same_crop(item, from);
	if (d <= TOLERANCE && crop_ok)
		return true;

	fprintf(stderr, "%s: the pasted item is %.1f px away from the copied one on the canvas%s\n", what, d,
		crop_ok ? "" : ", and its crop differs");
	print_corners(f, "copied", from);
	print_corners(f, "pasted", item);
	return false;
}

/* Sbssa's and timoschwarzer's case: paste the transform of an item outside
 * the group onto an item inside it. */
static void paste_onto_item_in_group(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct fixture f;
	fixture_create(&f);
	select_only(&f, f.in1, NULL);
	copy_paste(&f, f.out1);
	bool ok = check_lands_on(&f, "outside -> inside a group", f.in1, f.out1);
	fixture_destroy(&f);
	if (!ok)
		fail();
}

/* Sbssa's case: copy from an item inside the group, paste onto one outside
 * it. */
static void copy_from_item_in_group(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct fixture f;
	fixture_create(&f);
	select_only(&f, f.out2, NULL);
	copy_paste(&f, f.in1);
	bool ok = check_lands_on(&f, "inside a group -> outside", f.out2, f.in1);
	fixture_destroy(&f);
	if (!ok)
		fail();
}

/* Avasam's case: two items in the same group. */
static void paste_within_group(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct fixture f;
	fixture_create(&f);
	select_only(&f, f.in2, NULL);
	copy_paste(&f, f.in1);
	bool ok = check_lands_on(&f, "inside a group -> same group", f.in2, f.in1);
	fixture_destroy(&f);
	if (!ok)
		fail();
}

/* JDR-Ninja's case: one paste onto several selected items, inside and
 * outside the group. */
static void paste_onto_several_items(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct fixture f;
	fixture_create(&f);
	select_only(&f, f.in2, f.out2);
	copy_paste(&f, f.out1);
	bool ok = check_lands_on(&f, "outside -> several, item in the group", f.in2, f.out1);
	ok = check_lands_on(&f, "outside -> several, item outside", f.out2, f.out1) && ok;
	fixture_destroy(&f);
	if (!ok)
		fail();
}

/* The group and one of its items selected: the group is pasted first and the
 * item still lands where the copied item is. */
static void paste_onto_group_and_its_item(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct fixture f;
	fixture_create(&f);
	select_only(&f, f.group, f.in1);
	copy_paste(&f, f.out1);
	bool ok = check_lands_on(&f, "outside -> group and its item", f.in1, f.out1);

	struct vec2 scale, want;
	obs_sceneitem_get_scale(f.group, &scale);
	obs_sceneitem_get_scale(f.out1, &want);
	float rot = obs_sceneitem_get_rot(f.group);
	if (fabsf(scale.x - want.x) > 0.001f || fabsf(scale.y - want.y) > 0.001f ||
	    fabsf(rot - obs_sceneitem_get_rot(f.out1)) > 0.001f) {
		fprintf(stderr, "outside -> group and its item: the group got scale (%.3f, %.3f) rot %.1f\n", scale.x,
			scale.y, rot);
		ok = false;
	}
	fixture_destroy(&f);
	if (!ok)
		fail();
}

/* Sbssa's layout uses a bounding box ("Size: 1600x900"). Paste a bounding
 * box into the scaled group and copy it back out. */
static void bounding_box_into_group_and_out(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct fixture f;
	fixture_create(&f);

	struct vec2 bounds;
	vec2_set(&bounds, 400.0f, 240.0f);
	obs_sceneitem_set_bounds_type(f.out1, OBS_BOUNDS_STRETCH);
	obs_sceneitem_set_bounds(f.out1, &bounds);

	select_only(&f, f.in1, NULL);
	copy_paste(&f, f.out1);
	bool ok = check_lands_on(&f, "bounding box, outside -> inside a group", f.in1, f.out1);

	select_only(&f, f.out2, NULL);
	copy_paste(&f, f.in1);
	ok = check_lands_on(&f, "bounding box, inside a group -> outside", f.out2, f.in1) && ok;
	fixture_destroy(&f);
	if (!ok)
		fail();
}

/* Control: between two items outside any group, as before. */
static void paste_between_top_level_items(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct fixture f;
	fixture_create(&f);
	select_only(&f, f.out2, NULL);
	copy_paste(&f, f.out1);
	bool ok = check_lands_on(&f, "outside -> outside", f.out2, f.out1);
	fixture_destroy(&f);
	if (!ok)
		fail();
}

/* Control: locked and unselected items inside a group are left alone. */
static void locked_and_unselected_items_in_group_untouched(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct fixture f;
	fixture_create(&f);

	struct obs_transform_info before1, before2, after1, after2;
	obs_sceneitem_get_info2(f.in1, &before1);
	obs_sceneitem_get_info2(f.in2, &before2);

	obs_sceneitem_set_locked(f.in1, true);
	select_only(&f, f.in1, NULL);
	copy_paste(&f, f.out1);
	os_sleep_ms(100);

	obs_sceneitem_get_info2(f.in1, &after1);
	obs_sceneitem_get_info2(f.in2, &after2);
	bool ok = memcmp(&before1.pos, &after1.pos, sizeof(before1.pos)) == 0 &&
		  memcmp(&before1.scale, &after1.scale, sizeof(before1.scale)) == 0 && before1.rot == after1.rot &&
		  memcmp(&before2.pos, &after2.pos, sizeof(before2.pos)) == 0 &&
		  memcmp(&before2.scale, &after2.scale, sizeof(before2.scale)) == 0 && before2.rot == after2.rot;
	if (!ok)
		fprintf(stderr, "a locked or unselected item inside the group was changed\n");
	fixture_destroy(&f);
	if (!ok)
		fail();
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
		.base_width = 1920,
		.base_height = 1080,
		.output_width = 1920,
		.output_height = 1080,
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
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = box_name,
		.create = box_create,
		.destroy = box_destroy,
		.get_width = box_width,
		.get_height = box_height,
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
		cmocka_unit_test(paste_onto_item_in_group),
		cmocka_unit_test(copy_from_item_in_group),
		cmocka_unit_test(paste_within_group),
		cmocka_unit_test(paste_onto_several_items),
		cmocka_unit_test(paste_onto_group_and_its_item),
		cmocka_unit_test(bounding_box_into_group_and_out),
		cmocka_unit_test(paste_between_top_level_items),
		cmocka_unit_test(locked_and_unselected_items_in_group_untouched),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
