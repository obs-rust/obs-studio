/* Regression test for obsproject/obs-studio#12171 (reported by SquibsLand,
 * reproduced and bisected by Penwy): with Studio Mode's "Duplicate Scene"
 * option, the Program view shows some sources at the wrong size, while the
 * Preview of the same scene is right. Penwy found it only happens to scene
 * items whose saved scale reference ("scale_ref") differs from the current
 * canvas size, and bisected it to the change that moved scene items to
 * relative coordinates.
 *
 * A scene item's scale is stored relative to the canvas size it was set
 * at (scale_ref) and is rescaled to the current canvas when drawn.
 * Studio Mode shows the program through a private copy of the scene made by
 * obs_scene_duplicate(). That copy took the item's relative scale but kept
 * the scale reference a fresh item gets (the current canvas size), so an
 * item saved at 1920x1080 on a 2560x1440 canvas is drawn at 1920x1080 in the
 * copy instead of filling the canvas. "Duplicate Scene" in the scene list
 * and ungrouping take the same path.
 *
 * The test loads a scene the way a scene collection is loaded, with an item
 * saved on a 1920x1080 canvas, on a 2560x1440 canvas; duplicates it the way
 * Studio Mode does; renders both scenes and measures the drawn box.
 *
 * Showing a scene in the preview first hid the bug, because the copy also
 * takes the original's draw transform and keeps it until something makes it
 * recompute. SquibsLand found that this workaround fails for a stopped media
 * source, which has no size until it plays; one case covers that. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <math.h>
#include <stdio.h>
#include <stdlib.h>

#include <obs.h>
#include <graphics/vec4.h>
#include <util/platform.h>
#include <util/threading.h>

#define BOX_ID "scene_duplicate_scale_ref_box"
#define CANVAS_CX 2560
#define CANVAS_CY 1440
#define BOX_CX 1920
#define BOX_CY 1080

static bool obs_up;

static const char *box_name(void *type_data)
{
	(void)type_data;
	return "box";
}

/* A box of BOX_CX x BOX_CY, or of no size while "stopped" is set, like a
 * media source that has not started playing. */
struct box {
	volatile bool stopped;
};

static void box_update(void *data, obs_data_t *settings)
{
	struct box *box = data;
	os_atomic_set_bool(&box->stopped, obs_data_get_bool(settings, "stopped"));
}

static void *box_create(obs_data_t *settings, obs_source_t *source)
{
	(void)source;
	struct box *box = bzalloc(sizeof(*box));
	box_update(box, settings);
	return box;
}

static void box_destroy(void *data)
{
	bfree(data);
}

static uint32_t box_width(void *data)
{
	struct box *box = data;
	return os_atomic_load_bool(&box->stopped) ? 0 : BOX_CX;
}

static uint32_t box_height(void *data)
{
	struct box *box = data;
	return os_atomic_load_bool(&box->stopped) ? 0 : BOX_CY;
}

static void box_render(void *data, gs_effect_t *effect)
{
	struct box *box = data;
	(void)effect;
	if (os_atomic_load_bool(&box->stopped))
		return;
	gs_effect_t *solid = obs_get_base_effect(OBS_EFFECT_SOLID);
	struct vec4 white;
	vec4_set(&white, 1.0f, 1.0f, 1.0f, 1.0f);
	gs_effect_set_vec4(gs_effect_get_param_by_name(solid, "color"), &white);
	while (gs_effect_loop(solid, "Solid"))
		gs_draw_sprite(NULL, 0, BOX_CX, BOX_CY);
}

/* Lets the video thread drop the references it takes while ticking, then
 * waits for the deferred destroys (scenes release their items there), so
 * nothing is still being torn down when the next case or obs_shutdown()
 * runs. */
static void settle(void)
{
	uint32_t start = obs_get_total_frames();
	for (int i = 0; i < 500 && obs_get_total_frames() - start < 2; i++)
		os_sleep_ms(10);
	obs_wait_for_destroy_queue();
}

struct box_size {
	int cx;
	int cy;
};

/* Renders the scene on a transparent canvas and returns the size of the
 * area the box covers. */
static struct box_size measure(obs_scene_t *scene)
{
	struct box_size size = {0, 0};
	uint8_t *data;
	uint32_t linesize;

	obs_enter_graphics();
	gs_texrender_t *tr = gs_texrender_create(GS_RGBA, GS_ZS_NONE);
	gs_stagesurf_t *stage = gs_stagesurface_create(CANVAS_CX, CANVAS_CY, GS_RGBA);

	if (gs_texrender_begin(tr, CANVAS_CX, CANVAS_CY)) {
		struct vec4 clear;
		vec4_zero(&clear);
		gs_clear(GS_CLEAR_COLOR, &clear, 0.0f, 0);
		gs_ortho(0.0f, (float)CANVAS_CX, 0.0f, (float)CANVAS_CY, -100.0f, 100.0f);
		gs_blend_state_push();
		gs_reset_blend_state();
		obs_source_video_render(obs_scene_get_source(scene));
		gs_blend_state_pop();
		gs_texrender_end(tr);

		gs_stage_texture(stage, gs_texrender_get_texture(tr));
		if (gs_stagesurface_map(stage, &data, &linesize)) {
			int min_x = CANVAS_CX, min_y = CANVAS_CY, max_x = -1, max_y = -1;
			for (int y = 0; y < CANVAS_CY; y++) {
				const uint8_t *row = data + (size_t)y * linesize;
				for (int x = 0; x < CANVAS_CX; x++) {
					if (row[x * 4 + 3] < 128)
						continue;
					if (x < min_x)
						min_x = x;
					if (x > max_x)
						max_x = x;
					if (y < min_y)
						min_y = y;
					if (y > max_y)
						max_y = y;
				}
			}
			gs_stagesurface_unmap(stage);
			if (max_x >= 0) {
				size.cx = max_x - min_x + 1;
				size.cy = max_y - min_y + 1;
			}
		}
	}

	gs_stagesurface_destroy(stage);
	gs_texrender_destroy(tr);
	obs_leave_graphics();
	return size;
}

/* A scene as saved in a scene collection: one top-left aligned item at the
 * canvas origin, scale 1.0 relative to the canvas size it was saved at. */
static obs_source_t *load_scene(const char *name, const char *box, int ref_cx, int ref_cy)
{
	char json[1024];
	snprintf(json, sizeof(json),
		 "{\"id\": \"scene\", \"versioned_id\": \"scene\", \"name\": \"%s\","
		 " \"settings\": {\"items\": [{\"name\": \"%s\", \"visible\": true,"
		 " \"align\": 5, \"pos_rel\": {\"x\": %f, \"y\": -1.0},"
		 " \"scale_rel\": {\"x\": 1.0, \"y\": 1.0},"
		 " \"scale_ref\": {\"x\": %d.0, \"y\": %d.0}}]}}",
		 name, box, -16.0 / 9.0, ref_cx, ref_cy);
	obs_data_t *data = obs_data_create_from_json(json);
	obs_source_t *source = obs_load_source(data);
	obs_data_release(data);
	/* Scenes load their items in the second pass, as in obs_load_sources(). */
	obs_source_load2(source);
	return source;
}

static bool same_size(struct box_size a, struct box_size b)
{
	return abs(a.cx - b.cx) <= 1 && abs(a.cy - b.cy) <= 1;
}

/* The size the item is drawn at on the canvas: its saved scale (1.0) rescaled
 * from the canvas it was saved on to the current one. */
static struct box_size expected_size(int ref_cy)
{
	struct box_size size = {
		(int)lround((double)BOX_CX * CANVAS_CY / ref_cy),
		(int)lround((double)BOX_CY * CANVAS_CY / ref_cy),
	};
	return size;
}

static void check_duplicate(enum obs_scene_duplicate_type type, int ref_cx, int ref_cy, const char *what)
{
	char box_src[64], scene_name[64];
	snprintf(box_src, sizeof(box_src), "box (%s)", what);
	snprintf(scene_name, sizeof(scene_name), "scene (%s)", what);

	obs_source_t *box = obs_source_create(BOX_ID, box_src, NULL, NULL);
	assert_non_null(box);
	obs_source_t *source = load_scene(scene_name, box_src, ref_cx, ref_cy);
	assert_non_null(source);
	obs_scene_t *scene = obs_scene_from_source(source);
	assert_non_null(scene);

	obs_scene_t *dup = obs_scene_duplicate(scene, scene_name, type);
	assert_non_null(dup);

	struct box_size original = measure(scene);
	struct box_size copy = measure(dup);
	fprintf(stderr, "%s: scene draws the box at %dx%d, its duplicate at %dx%d\n", what, original.cx, original.cy,
		copy.cx, copy.cy);

	struct box_size expected = expected_size(ref_cy);
	bool ok = same_size(original, expected) && same_size(copy, expected);

	/* A public duplicate ("Duplicate Scene") joins the canvas like any
	 * scene, so it is removed the same way. */
	obs_source_remove(obs_scene_get_source(dup));
	obs_scene_release(dup);
	obs_source_remove(source);
	obs_source_release(source);
	obs_source_remove(box);
	obs_source_release(box);
	settle();

	if (!ok) {
		fprintf(stderr, "%s: expected both at %dx%d\n", what, expected.cx, expected.cy);
		fail();
	}
}

/* Studio Mode program view with "Duplicate Scene" on. */
static void studio_mode_program_keeps_saved_scale(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	check_duplicate(OBS_SCENE_DUP_PRIVATE_REFS, 1920, 1080, "studio mode program (private refs)");
}

/* Studio Mode with "Duplicate Sources" also on. */
static void studio_mode_program_copy_keeps_saved_scale(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	check_duplicate(OBS_SCENE_DUP_PRIVATE_COPY, 1920, 1080, "studio mode program (private copy)");
}

/* "Duplicate" in the scene list. */
static void duplicate_scene_keeps_saved_scale(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	check_duplicate(OBS_SCENE_DUP_REFS, 1920, 1080, "duplicate scene");
}

/* Control: an item saved at the current canvas size was never affected. */
static void duplicate_at_canvas_size_unchanged(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	check_duplicate(OBS_SCENE_DUP_PRIVATE_REFS, CANVAS_CX, CANVAS_CY, "saved at canvas size");
}

/* The scene was shown in the preview before it went to the program, but its
 * media source was stopped and only starts playing on the program. */
static void studio_mode_program_after_preview_keeps_saved_scale(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_data_t *stopped = obs_data_create();
	obs_data_set_bool(stopped, "stopped", true);
	obs_source_t *box = obs_source_create(BOX_ID, "box (media)", stopped, NULL);
	obs_data_release(stopped);
	assert_non_null(box);
	obs_source_t *source = load_scene("scene (media)", "box (media)", 1920, 1080);
	assert_non_null(source);
	obs_scene_t *scene = obs_scene_from_source(source);

	/* Preview, then the program copy, then the media starts. */
	measure(scene);
	obs_scene_t *dup = obs_scene_duplicate(scene, "scene (media)", OBS_SCENE_DUP_PRIVATE_REFS);
	assert_non_null(dup);
	measure(dup);
	obs_data_t *playing = obs_data_create();
	obs_data_set_bool(playing, "stopped", false);
	obs_source_update(box, playing);
	obs_data_release(playing);
	/* Video sources apply updates on the next video tick. */
	for (int i = 0; i < 500 && obs_source_get_width(box) != BOX_CX; i++)
		os_sleep_ms(10);

	struct box_size original = measure(scene);
	struct box_size copy = measure(dup);
	fprintf(stderr, "media started after preview: scene draws the box at %dx%d, its duplicate at %dx%d\n",
		original.cx, original.cy, copy.cx, copy.cy);

	struct box_size expected = expected_size(1080);
	bool ok = same_size(original, expected) && same_size(copy, expected);

	obs_scene_release(dup);
	obs_source_remove(source);
	obs_source_release(source);
	obs_source_remove(box);
	obs_source_release(box);
	settle();

	if (!ok) {
		fprintf(stderr, "media started after preview: expected both at %dx%d\n", expected.cx, expected.cy);
		fail();
	}
}

/* Ungrouping copies the group's items back into the scene the same way. */
static void ungroup_keeps_saved_scale(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_source_t *box = obs_source_create(BOX_ID, "box (ungroup)", NULL, NULL);
	assert_non_null(box);
	obs_source_t *source = load_scene("scene (ungroup)", "box (ungroup)", 1920, 1080);
	assert_non_null(source);
	obs_scene_t *scene = obs_scene_from_source(source);

	struct box_size before = measure(scene);

	obs_sceneitem_t *item = obs_scene_find_source(scene, "box (ungroup)");
	assert_non_null(item);
	obs_sceneitem_t *group = obs_scene_insert_group(scene, "group", &item, 1);
	assert_non_null(group);
	struct box_size grouped = measure(scene);
	obs_sceneitem_group_ungroup(group);
	struct box_size after = measure(scene);

	fprintf(stderr, "ungroup: box at %dx%d, grouped %dx%d, ungrouped %dx%d\n", before.cx, before.cy, grouped.cx,
		grouped.cy, after.cx, after.cy);

	bool ok = before.cx == CANVAS_CX && before.cy == CANVAS_CY && same_size(before, grouped) &&
		  same_size(before, after);

	obs_source_remove(source);
	obs_source_release(source);
	obs_source_remove(box);
	obs_source_release(box);
	settle();

	if (!ok) {
		fprintf(stderr, "ungroup: expected the box to stay at %dx%d\n", CANVAS_CX, CANVAS_CY);
		fail();
	}
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
		.fps_num = 30,
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
		.update = box_update,
		.get_width = box_width,
		.get_height = box_height,
		.video_render = box_render,
	};
	obs_register_source(&info);

	/* Let the graphics thread get through its first frames before the
	 * cases render on its context; under load, renders made right after
	 * obs_reset_video() could read back empty. */
	for (int i = 0; i < 500 && obs_get_total_frames() < 3; i++)
		os_sleep_ms(10);
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
		cmocka_unit_test(studio_mode_program_keeps_saved_scale),
		cmocka_unit_test(studio_mode_program_copy_keeps_saved_scale),
		cmocka_unit_test(studio_mode_program_after_preview_keeps_saved_scale),
		cmocka_unit_test(duplicate_scene_keeps_saved_scale),
		cmocka_unit_test(duplicate_at_canvas_size_unchanged),
		cmocka_unit_test(ungroup_keeps_saved_scale),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
