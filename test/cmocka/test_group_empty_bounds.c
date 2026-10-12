/* Regression test for obsproject/obs-studio#4245 (reported by lindenkron,
 * reproduced by farhanjiwani and flaeri): moving a source into an empty
 * group that has a bounding box breaks the group, and the transform dialog
 * shows 90001 for the item and the group.
 *
 * An empty group has no size. With a bounding box, the scene item transform
 * (calculate_bounds_data() in obs-scene.c) divides the box by the group's
 * size to find its scale, which gives an infinite scale. Adding an item maps
 * it into the group with the inverse of that transform, so the item's
 * position and scale become NaN, and the group's size is computed from them.
 * The transform dialog clamps NaN to its spin boxes' maximum: 90001.
 *
 * Each case builds a real libobs scene with a group at (240, 60) whose
 * bounding box is 800x600 (centred), adds a 200x100 source the way scripts
 * do (obs_sceneitem_group_add_item) or the way the Sources dock does when a
 * source is dragged into the group (obs_scene_reorder_items2), lets the
 * video thread tick, and checks that the transforms of the item and the
 * group are finite and that the item is drawn where the bounding box puts
 * it (canvas read back). */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <math.h>
#include <stdio.h>
#include <stdlib.h>

#include <obs.h>
#include <graphics/matrix4.h>
#include <graphics/vec4.h>
#include <util/platform.h>
#include <util/threading.h>

#define BOX_ID "group_empty_bounds_box"
#define CANVAS_CX 1280
#define CANVAS_CY 720

#define RED 0xFF0000FF   /* ABGR */
#define GREEN 0xFF00FF00 /* ABGR */

static bool obs_up;

/* A solid colour source whose size can change while it is shown. */
struct box {
	volatile long cx;
	volatile long cy;
	uint32_t color;
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
	box->color = (uint32_t)obs_data_get_int(settings, "color");
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
	struct box *box = data;
	(void)effect;

	uint32_t cx = box_width(box);
	uint32_t cy = box_height(box);
	if (!cx || !cy)
		return;

	gs_effect_t *solid = obs_get_base_effect(OBS_EFFECT_SOLID);
	struct vec4 color;
	vec4_from_rgba(&color, box->color);
	gs_effect_set_vec4(gs_effect_get_param_by_name(solid, "color"), &color);
	while (gs_effect_loop(solid, "Solid"))
		gs_draw_sprite(NULL, 0, cx, cy);
}

static obs_source_t *create_box(const char *name, uint32_t cx, uint32_t cy, uint32_t color)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_int(settings, "cx", cx);
	obs_data_set_int(settings, "cy", cy);
	obs_data_set_int(settings, "color", color);
	obs_source_t *source = obs_source_create(BOX_ID, name, settings, NULL);
	obs_data_release(settings);
	assert_non_null(source);
	return source;
}

static void resize_box(obs_source_t *source, uint32_t cx, uint32_t cy)
{
	struct box *box = obs_obj_get_data(source);
	os_atomic_set_long(&box->cx, (long)cx);
	os_atomic_set_long(&box->cy, (long)cy);
}

/* Waits until the video thread has ticked the scene (which applies pending
 * transforms and resizes groups) a few times. */
static void wait_for_ticks(void)
{
	uint32_t start = obs_get_total_frames();
	for (int i = 0; i < 500 && obs_get_total_frames() - start < 3; i++)
		os_sleep_ms(10);
}

enum add_path {
	/* obs_sceneitem_group_add_item(), as scripts do */
	ADD_ITEM,
	/* obs_scene_reorder_items2() with the group set, as the Sources dock
	 * does when a source is dragged into a group */
	DRAG_INTO,
};

static const char *path_name(enum add_path path)
{
	return path == ADD_ITEM ? "obs_sceneitem_group_add_item" : "dragged in (obs_scene_reorder_items2)";
}

static const char *bounds_name(enum obs_bounds_type type)
{
	switch (type) {
	case OBS_BOUNDS_NONE:
		return "no bounds";
	case OBS_BOUNDS_STRETCH:
		return "stretch";
	case OBS_BOUNDS_SCALE_INNER:
		return "scale inner";
	case OBS_BOUNDS_SCALE_OUTER:
		return "scale outer";
	case OBS_BOUNDS_SCALE_TO_WIDTH:
		return "scale to width";
	case OBS_BOUNDS_SCALE_TO_HEIGHT:
		return "scale to height";
	case OBS_BOUNDS_MAX_ONLY:
		return "max only";
	}
	return "?";
}

struct order_ctx {
	struct obs_sceneitem_order_info order[8];
	size_t count;
	obs_sceneitem_t *group;
	obs_sceneitem_t *item;
};

static bool push_child(obs_scene_t *scene, obs_sceneitem_t *child, void *param)
{
	struct order_ctx *ctx = param;
	(void)scene;
	ctx->order[ctx->count].group = ctx->group;
	ctx->order[ctx->count].item = child;
	ctx->count++;
	return true;
}

/* Moves `item` (a top-level item of `scene`) into `group` the way
 * SourceTree::dropEvent does: one obs_scene_reorder_items2() call with the
 * group's existing children and the item listed under the group. */
static void drag_into_group(obs_scene_t *scene, obs_sceneitem_t *group, obs_sceneitem_t *item)
{
	struct order_ctx ctx = {.group = group, .item = item};
	ctx.order[ctx.count].group = NULL;
	ctx.order[ctx.count].item = group;
	ctx.count++;
	obs_sceneitem_group_enum_items(group, push_child, &ctx);
	ctx.order[ctx.count].group = group;
	ctx.order[ctx.count].item = item;
	ctx.count++;
	assert_true(obs_scene_reorder_items2(scene, ctx.order, ctx.count));
}

static void add_to_group(obs_scene_t *scene, obs_sceneitem_t *group, obs_sceneitem_t *item, enum add_path path)
{
	if (path == ADD_ITEM)
		obs_sceneitem_group_add_item(group, item);
	else
		drag_into_group(scene, group, item);
	assert_ptr_equal(obs_sceneitem_get_group(scene, item), group);
}

struct rect {
	float x0, y0, x1, y1;
};

/* Where a group's child is drawn on the canvas. */
static struct rect child_on_canvas(obs_sceneitem_t *group, obs_sceneitem_t *child)
{
	struct matrix4 box, draw;
	struct vec3 a, b;

	obs_sceneitem_get_box_transform(child, &box);
	obs_sceneitem_get_draw_transform(group, &draw);
	vec3_set(&a, 0.0f, 0.0f, 0.0f);
	vec3_set(&b, 1.0f, 1.0f, 0.0f);
	vec3_transform(&a, &a, &box);
	vec3_transform(&a, &a, &draw);
	vec3_transform(&b, &b, &box);
	vec3_transform(&b, &b, &draw);
	struct rect r = {fminf(a.x, b.x), fminf(a.y, b.y), fmaxf(a.x, b.x), fmaxf(a.y, b.y)};
	return r;
}

static bool finite2(const struct vec2 *v)
{
	return isfinite(v->x) && isfinite(v->y);
}

/* The values the transform dialog shows must be finite: position, scale,
 * rotation and bounding box of the item and of its group, and the group's
 * size. Returns the number of problems found. */
static int check_finite(const char *what, obs_sceneitem_t *group, obs_sceneitem_t *item)
{
	int problems = 0;
	obs_sceneitem_t *items[] = {item, group};
	const char *names[] = {"item", "group"};

	for (int i = 0; i < 2; i++) {
		struct obs_transform_info info;
		obs_sceneitem_get_info2(items[i], &info);
		if (!finite2(&info.pos) || !finite2(&info.scale) || !isfinite(info.rot) || !finite2(&info.bounds)) {
			fprintf(stderr,
				"%s: the %s's transform is not finite: pos (%g, %g), scale (%g, %g), rot %g, "
				"bounds (%g, %g)\n",
				what, names[i], info.pos.x, info.pos.y, info.scale.x, info.scale.y, info.rot,
				info.bounds.x, info.bounds.y);
			problems++;
		}

		struct matrix4 draw;
		obs_sceneitem_get_draw_transform(items[i], &draw);
		if (!isfinite(draw.x.x) || !isfinite(draw.y.y) || !isfinite(draw.t.x) || !isfinite(draw.t.y)) {
			fprintf(stderr, "%s: the %s's draw transform is not finite\n", what, names[i]);
			problems++;
		}
	}
	return problems;
}

/* Renders the canvas and returns the number of sample points around `r` that
 * do not show the expected colour: points 3 px inside each edge (and the
 * centre) must be `color`, points 3 px outside each edge (where they are on
 * the canvas) must not be. */
static int check_drawn_at(const char *what, obs_scene_t *scene, const struct rect *r, uint32_t color)
{
	int wrong = 0;
	uint8_t *data;
	uint32_t linesize;

	float cx = (r->x0 + r->x1) / 2.0f;
	float cy = (r->y0 + r->y1) / 2.0f;
	struct {
		float x, y;
		bool inside;
	} pts[] = {
		{cx, cy, true},
		{r->x0 + 3.0f, cy, true},
		{r->x1 - 3.0f, cy, true},
		{cx, r->y0 + 3.0f, true},
		{cx, r->y1 - 3.0f, true},
		{r->x0 - 3.0f, cy, false},
		{r->x1 + 3.0f, cy, false},
		{cx, r->y0 - 3.0f, false},
		{cx, r->y1 + 3.0f, false},
	};

	uint8_t want_r = color & 0xFF;
	uint8_t want_g = (color >> 8) & 0xFF;
	uint8_t want_b = (color >> 16) & 0xFF;

	obs_enter_graphics();
	gs_texrender_t *tr = gs_texrender_create(GS_RGBA, GS_ZS_NONE);
	gs_stagesurf_t *stage = gs_stagesurface_create(CANVAS_CX, CANVAS_CY, GS_RGBA);
	bool mapped = false;

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
		mapped = gs_stagesurface_map(stage, &data, &linesize);
		if (mapped) {
			for (size_t i = 0; i < sizeof(pts) / sizeof(pts[0]); i++) {
				int x = (int)floorf(pts[i].x);
				int y = (int)floorf(pts[i].y);
				if (x < 0 || y < 0 || x >= CANVAS_CX || y >= CANVAS_CY)
					continue;
				const uint8_t *p = data + (size_t)y * linesize + (size_t)x * 4;
				bool match = abs(p[0] - want_r) < 40 && abs(p[1] - want_g) < 40 &&
					     abs(p[2] - want_b) < 40 && p[3] > 200;
				if (match != pts[i].inside) {
					fprintf(stderr,
						"%s: pixel (%d, %d) is %02x%02x%02x%02x, expected %s the item\n", what,
						x, y, p[0], p[1], p[2], p[3], pts[i].inside ? "inside" : "outside");
					wrong++;
				}
			}
			gs_stagesurface_unmap(stage);
		}
	}

	gs_stagesurface_destroy(stage);
	gs_texrender_destroy(tr);
	obs_leave_graphics();

	if (!mapped) {
		fprintf(stderr, "%s: could not read the canvas back\n", what);
		wrong++;
	}
	return wrong;
}

/* Within a pixel: bounding box sizes are stored relative to the canvas, and
 * the alignment offset is truncated to whole pixels, so 600 can come back as
 * 599.99994 and move the item by one pixel. */
static int check_rect(const char *what, const struct rect *got, const struct rect *want)
{
	if (fabsf(got->x0 - want->x0) > 1.01f || fabsf(got->y0 - want->y0) > 1.01f ||
	    fabsf(got->x1 - want->x1) > 1.01f || fabsf(got->y1 - want->y1) > 1.01f || !isfinite(got->x0) ||
	    !isfinite(got->x1) || !isfinite(got->y0) || !isfinite(got->y1)) {
		fprintf(stderr, "%s: the item is at (%g, %g)-(%g, %g) on the canvas, expected (%g, %g)-(%g, %g)\n",
			what, got->x0, got->y0, got->x1, got->y1, want->x0, want->y0, want->x1, want->y1);
		return 1;
	}
	return 0;
}

struct fixture {
	obs_scene_t *scene;
	obs_sceneitem_t *group;
};

static bool first_child_cb(obs_scene_t *scene, obs_sceneitem_t *item, void *param)
{
	(void)scene;
	*(obs_sceneitem_t **)param = item;
	return false;
}

/* A scene with a group at (240, 60) and an 800x600 bounding box, centred.
 * With `first`, the group is created around an item of that source (as
 * "Group Selected Items" does), so it is not empty when the bounding box is
 * set. */
static struct fixture make_group(enum obs_bounds_type type, obs_source_t *first)
{
	struct fixture f;
	struct vec2 v;
	f.scene = obs_scene_create("group empty bounds");
	assert_non_null(f.scene);
	obs_set_output_source(0, obs_scene_get_source(f.scene));

	if (first) {
		obs_sceneitem_t *item = obs_scene_add(f.scene, first);
		assert_non_null(item);
		vec2_set(&v, 240.0f, 60.0f);
		obs_sceneitem_set_pos(item, &v);
		wait_for_ticks();
		f.group = obs_scene_insert_group(f.scene, "group", &item, 1);
	} else {
		f.group = obs_scene_add_group(f.scene, "group");
	}
	assert_non_null(f.group);
	wait_for_ticks();

	vec2_set(&v, 240.0f, 60.0f);
	obs_sceneitem_set_pos(f.group, &v);
	obs_sceneitem_set_bounds_type(f.group, type);
	vec2_set(&v, 800.0f, 600.0f);
	obs_sceneitem_set_bounds(f.group, &v);
	obs_sceneitem_set_bounds_alignment(f.group, OBS_ALIGN_CENTER);
	wait_for_ticks();
	return f;
}

static obs_sceneitem_t *first_child(obs_sceneitem_t *group)
{
	obs_scene_t *group_scene = obs_sceneitem_group_get_scene(group);
	obs_sceneitem_t *found = NULL;
	obs_scene_enum_items(group_scene, first_child_cb, &found);
	return found;
}

static void free_fixture(struct fixture *f)
{
	obs_set_output_source(0, NULL);
	obs_source_remove(obs_scene_get_source(f->scene));
	obs_scene_release(f->scene);
	wait_for_ticks();
}

static obs_sceneitem_t *add_box(obs_scene_t *scene, obs_source_t *source, float x, float y)
{
	obs_sceneitem_t *item = obs_scene_add(scene, source);
	assert_non_null(item);
	struct vec2 pos;
	vec2_set(&pos, x, y);
	obs_sceneitem_set_pos(item, &pos);
	wait_for_ticks();
	return item;
}

/* Where the bounding box puts a 200x100 source: the group's 800x600 box at
 * (240, 60), centred. */
static struct rect expected_rect(enum obs_bounds_type type)
{
	switch (type) {
	case OBS_BOUNDS_STRETCH: /* scale 4 x 6 */
		return (struct rect){240.0f, 60.0f, 1040.0f, 660.0f};
	case OBS_BOUNDS_SCALE_INNER:    /* scale 4: 800x400 */
	case OBS_BOUNDS_SCALE_TO_WIDTH: /* scale 4: 800x400 */
		return (struct rect){240.0f, 160.0f, 1040.0f, 560.0f};
	case OBS_BOUNDS_SCALE_OUTER:     /* scale 6: 1200x600 */
	case OBS_BOUNDS_SCALE_TO_HEIGHT: /* scale 6: 1200x600 */
		return (struct rect){40.0f, 60.0f, 1240.0f, 660.0f};
	case OBS_BOUNDS_MAX_ONLY: /* fits already: 200x100 in the middle */
		return (struct rect){540.0f, 310.0f, 740.0f, 410.0f};
	case OBS_BOUNDS_NONE:
		break;
	}
	return (struct rect){0};
}

/* Adds a 200x100 source to an empty group with bounds of `type`. */
static void check_empty_group(enum obs_bounds_type type, enum add_path path)
{
	char what[128];
	snprintf(what, sizeof(what), "%s, %s", bounds_name(type), path_name(path));

	struct fixture f = make_group(type, NULL);
	obs_source_t *source = create_box("box", 200, 100, RED);
	obs_sceneitem_t *item = add_box(f.scene, source, 600.0f, 300.0f);

	add_to_group(f.scene, f.group, item, path);
	wait_for_ticks();

	int problems = check_finite(what, f.group, item);

	uint32_t group_cx = obs_source_get_width(obs_sceneitem_get_source(f.group));
	uint32_t group_cy = obs_source_get_height(obs_sceneitem_get_source(f.group));
	if (group_cx != 200 || group_cy != 100) {
		fprintf(stderr, "%s: the group's content is %ux%u, expected 200x100\n", what, group_cx, group_cy);
		problems++;
	}

	struct rect want = expected_rect(type);
	struct rect got = child_on_canvas(f.group, item);
	problems += check_rect(what, &got, &want);
	problems += check_drawn_at(what, f.scene, &want, RED);

	obs_source_release(source);
	free_fixture(&f);
	if (problems) {
		fprintf(stderr, "%s: %d problem(s)\n", what, problems);
		fail();
	}
}

#define EMPTY_GROUP_CASE(name, type, path)    \
	static void name(void **state)        \
	{                                     \
		(void)state;                  \
		if (!obs_up)                  \
			skip();               \
		check_empty_group(type, path); \
	}

EMPTY_GROUP_CASE(add_item_stretch, OBS_BOUNDS_STRETCH, ADD_ITEM)
EMPTY_GROUP_CASE(add_item_scale_inner, OBS_BOUNDS_SCALE_INNER, ADD_ITEM)
EMPTY_GROUP_CASE(add_item_scale_outer, OBS_BOUNDS_SCALE_OUTER, ADD_ITEM)
EMPTY_GROUP_CASE(add_item_scale_to_width, OBS_BOUNDS_SCALE_TO_WIDTH, ADD_ITEM)
EMPTY_GROUP_CASE(add_item_scale_to_height, OBS_BOUNDS_SCALE_TO_HEIGHT, ADD_ITEM)
EMPTY_GROUP_CASE(add_item_max_only, OBS_BOUNDS_MAX_ONLY, ADD_ITEM)
EMPTY_GROUP_CASE(drag_into_stretch, OBS_BOUNDS_STRETCH, DRAG_INTO)
EMPTY_GROUP_CASE(drag_into_scale_inner, OBS_BOUNDS_SCALE_INNER, DRAG_INTO)
EMPTY_GROUP_CASE(drag_into_scale_outer, OBS_BOUNDS_SCALE_OUTER, DRAG_INTO)
EMPTY_GROUP_CASE(drag_into_scale_to_width, OBS_BOUNDS_SCALE_TO_WIDTH, DRAG_INTO)
EMPTY_GROUP_CASE(drag_into_scale_to_height, OBS_BOUNDS_SCALE_TO_HEIGHT, DRAG_INTO)
EMPTY_GROUP_CASE(drag_into_max_only, OBS_BOUNDS_MAX_ONLY, DRAG_INTO)

/* A source with no size yet (a browser or image source that has not loaded)
 * added to an empty group with a bounding box: the transforms must stay
 * finite, and once the source has a size it is drawn in the box. */
static void check_zero_size_source(enum add_path path)
{
	char what[128];
	snprintf(what, sizeof(what), "0x0 source, scale inner, %s", path_name(path));

	struct fixture f = make_group(OBS_BOUNDS_SCALE_INNER, NULL);
	obs_source_t *source = create_box("empty box", 0, 0, RED);
	obs_sceneitem_t *item = add_box(f.scene, source, 600.0f, 300.0f);

	add_to_group(f.scene, f.group, item, path);
	wait_for_ticks();
	int problems = check_finite(what, f.group, item);

	resize_box(source, 200, 100);
	wait_for_ticks();
	snprintf(what, sizeof(what), "0x0 source grown to 200x100, scale inner, %s", path_name(path));
	problems += check_finite(what, f.group, item);
	struct rect want = expected_rect(OBS_BOUNDS_SCALE_INNER);
	struct rect got = child_on_canvas(f.group, item);
	problems += check_rect(what, &got, &want);
	problems += check_drawn_at(what, f.scene, &want, RED);

	obs_source_release(source);
	free_fixture(&f);
	if (problems)
		fail();
}

static void zero_size_source_add_item(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	check_zero_size_source(ADD_ITEM);
}

static void zero_size_source_drag_into(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	check_zero_size_source(DRAG_INTO);
}

/* Control: a 0x0 source dragged into a group with a bounding box that already
 * holds a source, inside that source. The other source stays where it is. */
static void zero_size_source_into_filled_group(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	const char *what = "0x0 source into a filled group, scale inner";

	obs_source_t *first_src = create_box("first", 200, 100, GREEN);
	struct fixture f = make_group(OBS_BOUNDS_SCALE_INNER, first_src);
	obs_sceneitem_t *first = first_child(f.group);
	assert_non_null(first);

	/* Inside the first source, (240, 160)-(1040, 560), so the group's
	 * content does not grow. */
	obs_source_t *source = create_box("empty box", 0, 0, RED);
	obs_sceneitem_t *item = add_box(f.scene, source, 640.0f, 360.0f);
	add_to_group(f.scene, f.group, item, DRAG_INTO);
	wait_for_ticks();

	int problems = check_finite(what, f.group, item);
	struct rect want = expected_rect(OBS_BOUNDS_SCALE_INNER);
	struct rect got = child_on_canvas(f.group, first);
	problems += check_rect(what, &got, &want);
	problems += check_drawn_at(what, f.scene, &want, GREEN);

	obs_source_release(source);
	obs_source_release(first_src);
	free_fixture(&f);
	if (problems)
		fail();
}

/* Control: a source dragged into a group with a bounding box that already
 * holds a source, inside the content's area. Nothing moves on the canvas. */
static void drag_into_filled_group(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	const char *what = "source dragged into a filled group, scale inner";

	obs_source_t *first_src = create_box("first", 200, 100, GREEN);
	struct fixture f = make_group(OBS_BOUNDS_SCALE_INNER, first_src);
	obs_sceneitem_t *first = first_child(f.group);
	assert_non_null(first);

	/* The first source fills (240, 160)-(1040, 560); put the second one at
	 * (640, 360), 200x100, inside it. */
	obs_source_t *source = create_box("second", 200, 100, RED);
	obs_sceneitem_t *item = add_box(f.scene, source, 640.0f, 360.0f);
	add_to_group(f.scene, f.group, item, DRAG_INTO);
	wait_for_ticks();

	int problems = check_finite(what, f.group, item);
	struct rect want_first = expected_rect(OBS_BOUNDS_SCALE_INNER);
	struct rect got = child_on_canvas(f.group, first);
	problems += check_rect(what, &got, &want_first);
	struct rect want = {640.0f, 360.0f, 840.0f, 460.0f};
	got = child_on_canvas(f.group, item);
	problems += check_rect(what, &got, &want);
	problems += check_drawn_at(what, f.scene, &want, RED);

	obs_source_release(source);
	obs_source_release(first_src);
	free_fixture(&f);
	if (problems)
		fail();
}

/* Control: obs_sceneitem_group_add_item() into a group with a bounding box
 * that already holds a source keeps every transform finite. (Where the added
 * source lands is not checked: the function maps it through the group's
 * transform before it is in the group, a separate problem.) */
static void add_item_to_filled_group(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	const char *what = "source added to a filled group, scale inner";

	obs_source_t *first_src = create_box("first", 200, 100, GREEN);
	struct fixture f = make_group(OBS_BOUNDS_SCALE_INNER, first_src);
	obs_sceneitem_t *first = first_child(f.group);
	assert_non_null(first);

	obs_source_t *source = create_box("second", 200, 100, RED);
	obs_sceneitem_t *item = add_box(f.scene, source, 640.0f, 360.0f);
	add_to_group(f.scene, f.group, item, ADD_ITEM);
	wait_for_ticks();

	int problems = check_finite(what, f.group, item) + check_finite(what, f.group, first);

	obs_source_release(source);
	obs_source_release(first_src);
	free_fixture(&f);
	if (problems)
		fail();
}

/* Control: an empty group without a bounding box. The source stays where it
 * was on the canvas. */
static void drag_into_empty_group_without_bounds(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	const char *what = "no bounds, dragged in (obs_scene_reorder_items2)";

	struct fixture f = make_group(OBS_BOUNDS_NONE, NULL);
	obs_source_t *source = create_box("box", 200, 100, RED);
	obs_sceneitem_t *item = add_box(f.scene, source, 600.0f, 300.0f);
	add_to_group(f.scene, f.group, item, DRAG_INTO);
	wait_for_ticks();

	int problems = check_finite(what, f.group, item);
	struct rect want = {600.0f, 300.0f, 800.0f, 400.0f};
	struct rect got = child_on_canvas(f.group, item);
	problems += check_rect(what, &got, &want);
	problems += check_drawn_at(what, f.scene, &want, RED);

	obs_source_release(source);
	free_fixture(&f);
	if (problems)
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
		cmocka_unit_test(add_item_stretch),
		cmocka_unit_test(add_item_scale_inner),
		cmocka_unit_test(add_item_scale_outer),
		cmocka_unit_test(add_item_scale_to_width),
		cmocka_unit_test(add_item_scale_to_height),
		cmocka_unit_test(add_item_max_only),
		cmocka_unit_test(drag_into_stretch),
		cmocka_unit_test(drag_into_scale_inner),
		cmocka_unit_test(drag_into_scale_outer),
		cmocka_unit_test(drag_into_scale_to_width),
		cmocka_unit_test(drag_into_scale_to_height),
		cmocka_unit_test(drag_into_max_only),
		cmocka_unit_test(zero_size_source_add_item),
		cmocka_unit_test(zero_size_source_drag_into),
		cmocka_unit_test(zero_size_source_into_filled_group),
		cmocka_unit_test(drag_into_filled_group),
		cmocka_unit_test(add_item_to_filled_group),
		cmocka_unit_test(drag_into_empty_group_without_bounds),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
