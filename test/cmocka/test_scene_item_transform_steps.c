#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <limits.h>
#include <math.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs.h"
#include "graphics/math-defs.h"
#include "graphics/matrix4.h"
#include "graphics/vec2.h"
#include "obs-scene-bounds.h"
#include "obs-scene-coords.h"
#include "obs-scene-crop.h"
#include "obs-scene-item-transform.h"

/*
 * Characterization of the scene item transform composition of
 * compute_item_transform() in libobs/obs-scene.c, moved to
 * libobs/obs-scene-item-transform.c before its Rust port:
 * - obs_scene_item_transform_prepare(): the bounds crop reset, a nested
 *   scene's crop rescaled by its size change, the cropped size and the last
 *   size;
 * - obs_scene_item_transform_bounded(): whether the item has a bounding box;
 * - obs_scene_item_transform_matrix(): draw_transform and box_transform;
 * - obs_scene_item_transform_box_scale(): the box scale without bounds.
 * Cases marked "characterized, not endorsed" are odd but real.
 *
 * compose() below runs the steps in compute_item_transform()'s order, with
 * what obs-scene.c keeps between them: the canvas size get_scene_dimensions()
 * reads (given here as values), item_canvas_scale(), pos_to_absolute() and
 * size_to_absolute() (the hidden obs_scene_* coordinate functions),
 * calculate_bounds_data() (obs_bounds_place()), add_alignment() and the
 * casts. The two (uint32_t)((float)cx * scale.x) of an item without bounds
 * are undefined in C for a flipped item (a negative product): x86-64
 * converts through a 64-bit register and wraps, arm64 saturates to 0.
 * compose() pins x86-64's result on every platform by doing that conversion
 * in defined C (x86_cast_u32()), so the pins do not depend on the
 * architecture the test runs on; obs-scene.c keeps the cast itself.
 *
 * Every pinned float is the same on every platform: the composition is
 * float multiplies, adds and conversions, and the only library calls are
 * sinf() and cosf() of the rotation. Their results are exact for a rotation
 * of 0 (sin 0 and cos 0), which every pin outside the rotation cases uses.
 * The rotation cases pin RAD(rot) (one float multiply), check the matrix
 * against the same libobs matrix4 calls in the documented order, and pin
 * the matrix bits only with glibc on x86-64, the C library the bits were
 * taken from.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself (with obs-scene-crop.c, obs-scene-coords.c, obs-scene-bounds.c
 * and obs-scene-bounds-place.c, also hidden) and calls only them; matrix4_*
 * come from libobs. test_scene_group_transform, test_scene_bounds_place,
 * test_scene_bounds_fit, test_scene_coords, test_transform_clipboard,
 * test_scene_rotated_scale_filter, test_scene_item_crop,
 * test_scene_item_crop_load, test_scene_duplicate_scale_ref,
 * test_group_child_drift, test_group_extent, test_group_empty_bounds and
 * test_scene_bounds_type_range cover the callers through the real libobs;
 * the last one pins that a bounds type outside the enum is OBS_BOUNDS_NONE
 * before any of this runs (#2281).
 */

/* obs-internal.h, which this test does not include. */
void add_alignment(struct vec2 *v, uint32_t align, int cx, int cy);

static int failures;

static uint32_t float_bits(float f)
{
	uint32_t u;
	memcpy(&u, &f, sizeof(u));
	return u;
}

static float bits_float(uint32_t u)
{
	float f;
	memcpy(&f, &u, sizeof(f));
	return f;
}

static void check_u64_line(uint64_t actual, uint64_t expected, const char *what, int line)
{
	if (actual != expected) {
		fprintf(stderr, "PIN line=%d got=0x%016llxull: %s: expected 0x%016llx\n", line,
			(unsigned long long)actual, what, (unsigned long long)expected);
		failures++;
	}
}

static void check_bits_line(float actual, uint32_t expected, const char *what, int line)
{
	if (float_bits(actual) != expected) {
		fprintf(stderr, "PIN line=%d got=0x%08xu: %s: %g, expected %g (0x%08x)\n", line, float_bits(actual),
			what, (double)actual, (double)bits_float(expected), expected);
		failures++;
	}
}

static void check_int_line(long long actual, long long expected, const char *what, int line)
{
	if (actual != expected) {
		fprintf(stderr, "PIN line=%d got=%lld: %s: expected %lld\n", line, actual, what, expected);
		failures++;
	}
}

#define check_u64(actual, expected, what) check_u64_line(actual, expected, what, __LINE__)
#define check_bits(actual, expected, what) check_bits_line(actual, expected, what, __LINE__)
#define check_int(actual, expected, what) check_int_line(actual, expected, what, __LINE__)

static void finish(void)
{
	int n = failures;
	failures = 0;
	if (n) {
		fprintf(stderr, "%d pinned value(s) differ\n", n);
		fail();
	}
}

/* ------------------------------------------------------------------------- */

/* The item fields compute_item_transform() reads and writes. */
struct item {
	struct vec2 pos;
	struct vec2 scale;
	struct vec2 scale_ref;
	float rot;
	uint32_t align;
	struct obs_sceneitem_crop crop;
	struct obs_sceneitem_crop bounds_crop;
	bool absolute;
	bool is_scene;
	bool is_group;
	bool parent_is_group;
	uint32_t last_width;
	uint32_t last_height;
	int bounds_type;
	uint32_t bounds_align;
	struct vec2 bounds;
	bool crop_to_bounds;

	struct matrix4 draw;
	struct matrix4 box;
	struct vec2 output_scale;
	struct vec2 box_scale;
	/* the size after the bounds or the cast, which add_alignment() gets */
	uint32_t cx;
	uint32_t cy;
};

/* x86-64's (uint32_t) of a float: through a 64-bit signed conversion, then
 * the low 32 bits. Defined here for the finite values below 2^63 the test
 * uses; for those it is the C cast on x86-64 (GCC, Clang, MSVC). */
static uint32_t x86_cast_u32(float f)
{
	return (uint32_t)(uint64_t)(int64_t)f;
}

static void compose(struct item *it, uint32_t width, uint32_t height, bool update_tex, float canvas_x, float canvas_y)
{
	struct obs_scene_item_transform_size size;
	uint32_t cx;
	uint32_t cy;
	struct vec2 base_origin;
	struct vec2 origin;
	struct vec2 scale;
	struct vec2 position;
	bool nudge = obs_scene_coords_nudge(it->is_group, it->parent_is_group);

	size = obs_scene_item_transform_prepare(&it->crop, &it->bounds_crop, &it->last_width, &it->last_height, width,
						height, update_tex && it->is_scene);
	cx = size.cx;
	cy = size.cy;
	width = cx;
	height = cy;

	vec2_zero(&base_origin);
	vec2_zero(&origin);

	if (!it->absolute) {
		if (obs_scene_coords_keep_scale(it->is_group, it->is_scene))
			scale = it->scale;
		else
			obs_scene_item_canvas_scale(&scale, &it->scale, canvas_y, it->scale_ref.y);
		obs_scene_pos_to_absolute(&position, &it->pos, canvas_x, canvas_y, nudge);
	} else {
		scale = it->scale;
		position = it->pos;
	}

	if (obs_scene_item_transform_bounded(it->bounds_type)) {
		struct vec2 bounds;
		if (it->absolute)
			bounds = it->bounds;
		else
			obs_scene_size_to_absolute(&bounds, &it->bounds, canvas_y, nudge);
		obs_bounds_place((enum obs_bounds_type)it->bounds_type, &bounds, &origin, &scale, &cx, &cy,
				 it->bounds_align, it->crop_to_bounds, &it->bounds_crop);
	} else {
		cx = x86_cast_u32((float)cx * scale.x);
		cy = x86_cast_u32((float)cy * scale.y);
	}
	it->cx = cx;
	it->cy = cy;

	add_alignment(&origin, it->align, (int)cx, (int)cy);
	obs_scene_item_transform_matrix(&it->draw, &scale, &origin, it->rot, &position);
	it->output_scale = scale;

	if (obs_scene_item_transform_bounded(it->bounds_type)) {
		if (!it->absolute)
			obs_scene_size_to_absolute(&scale, &it->bounds, canvas_y, nudge);
		else
			scale = it->bounds;
	} else {
		obs_scene_item_transform_box_scale(&scale, width, height);
	}
	it->box_scale = scale;

	add_alignment(&base_origin, it->align, (int)scale.x, (int)scale.y);
	obs_scene_item_transform_matrix(&it->box, &scale, &base_origin, it->rot, &position);
}

static uint64_t fnv_u32(uint64_t h, uint32_t v)
{
	for (int i = 0; i < 4; i++) {
		h ^= (v >> (8 * i)) & 0xffu;
		h *= 0x100000001b3ull;
	}
	return h;
}

static uint64_t fnv_matrix(uint64_t h, const struct matrix4 *m)
{
	const struct vec4 *rows[4] = {&m->x, &m->y, &m->z, &m->t};
	for (int r = 0; r < 4; r++) {
		h = fnv_u32(h, float_bits(rows[r]->x));
		h = fnv_u32(h, float_bits(rows[r]->y));
		h = fnv_u32(h, float_bits(rows[r]->z));
		h = fnv_u32(h, float_bits(rows[r]->w));
	}
	return h;
}

/* Every output of one composition, in a fixed order. */
static uint64_t fnv_item(uint64_t h, const struct item *it)
{
	h = fnv_matrix(h, &it->draw);
	h = fnv_matrix(h, &it->box);
	h = fnv_u32(h, float_bits(it->output_scale.x));
	h = fnv_u32(h, float_bits(it->output_scale.y));
	h = fnv_u32(h, float_bits(it->box_scale.x));
	h = fnv_u32(h, float_bits(it->box_scale.y));
	h = fnv_u32(h, (uint32_t)it->crop.left);
	h = fnv_u32(h, (uint32_t)it->crop.top);
	h = fnv_u32(h, (uint32_t)it->crop.right);
	h = fnv_u32(h, (uint32_t)it->crop.bottom);
	h = fnv_u32(h, (uint32_t)it->bounds_crop.left);
	h = fnv_u32(h, (uint32_t)it->bounds_crop.top);
	h = fnv_u32(h, (uint32_t)it->bounds_crop.right);
	h = fnv_u32(h, (uint32_t)it->bounds_crop.bottom);
	h = fnv_u32(h, it->last_width);
	h = fnv_u32(h, it->last_height);
	h = fnv_u32(h, it->cx);
	h = fnv_u32(h, it->cy);
	return h;
}

#define FNV_BASIS 0xcbf29ce484222325ull

/* A 200 x 100 source at (100, 50) on a 1920 x 1080 canvas, scale 1.5,
 * top-left, no bounds, no crop, absolute coordinates. */
static struct item base_item(void)
{
	struct item it;
	memset(&it, 0, sizeof(it));
	it.pos.x = 100.0f;
	it.pos.y = 50.0f;
	it.scale.x = 1.5f;
	it.scale.y = 1.5f;
	it.scale_ref.x = 1920.0f;
	it.scale_ref.y = 1080.0f;
	it.align = OBS_ALIGN_TOP | OBS_ALIGN_LEFT;
	it.absolute = true;
	it.bounds_type = OBS_BOUNDS_NONE;
	it.bounds_align = OBS_ALIGN_CENTER;
	it.bounds.x = 800.0f;
	it.bounds.y = 600.0f;
	return it;
}

#define SRC_CX 200
#define SRC_CY 100

static const uint32_t alignments[] = {
	OBS_ALIGN_CENTER,
	OBS_ALIGN_LEFT,
	OBS_ALIGN_RIGHT,
	OBS_ALIGN_TOP,
	OBS_ALIGN_TOP | OBS_ALIGN_LEFT,
	OBS_ALIGN_TOP | OBS_ALIGN_RIGHT,
	OBS_ALIGN_BOTTOM,
	OBS_ALIGN_BOTTOM | OBS_ALIGN_LEFT,
	OBS_ALIGN_BOTTOM | OBS_ALIGN_RIGHT,
	/* extra bits: both sides of each axis (right and bottom win), and a
	 * bit above the four */
	OBS_ALIGN_LEFT | OBS_ALIGN_RIGHT | OBS_ALIGN_TOP | OBS_ALIGN_BOTTOM,
	0x10u | OBS_ALIGN_TOP | OBS_ALIGN_LEFT,
};
#define N_ALIGN (sizeof(alignments) / sizeof(alignments[0]))

static const float sign_x[] = {1.0f, -1.0f, 1.0f, -1.0f};
static const float sign_y[] = {1.0f, 1.0f, -1.0f, -1.0f};

/* ------------------------------------------------------------------------- */

static void steps_layout(void **state)
{
	(void)state;
	check_int((long long)sizeof(struct obs_scene_item_transform_size), 8, "size struct size");
	check_int((long long)offsetof(struct obs_scene_item_transform_size, cx), 0, "cx offset");
	check_int((long long)offsetof(struct obs_scene_item_transform_size, cy), 4, "cy offset");
	check_int((long long)sizeof(struct obs_sceneitem_crop), 16, "crop size");
	check_int((long long)sizeof(struct matrix4), 64, "matrix4 size");
	check_int((long long)sizeof(struct vec2), 8, "vec2 size");
	check_int(OBS_BOUNDS_NONE, 0, "OBS_BOUNDS_NONE");
	check_int(OBS_BOUNDS_STRETCH, 1, "OBS_BOUNDS_STRETCH");
	check_int(OBS_BOUNDS_SCALE_INNER, 2, "OBS_BOUNDS_SCALE_INNER");
	check_int(OBS_BOUNDS_SCALE_OUTER, 3, "OBS_BOUNDS_SCALE_OUTER");
	check_int(OBS_BOUNDS_SCALE_TO_WIDTH, 4, "OBS_BOUNDS_SCALE_TO_WIDTH");
	check_int(OBS_BOUNDS_SCALE_TO_HEIGHT, 5, "OBS_BOUNDS_SCALE_TO_HEIGHT");
	check_int(OBS_BOUNDS_MAX_ONLY, 6, "OBS_BOUNDS_MAX_ONLY");
	check_int(OBS_ALIGN_CENTER, 0, "OBS_ALIGN_CENTER");
	check_int(OBS_ALIGN_LEFT, 1, "OBS_ALIGN_LEFT");
	check_int(OBS_ALIGN_RIGHT, 2, "OBS_ALIGN_RIGHT");
	check_int(OBS_ALIGN_TOP, 4, "OBS_ALIGN_TOP");
	check_int(OBS_ALIGN_BOTTOM, 8, "OBS_ALIGN_BOTTOM");
	/* RAD() is one float multiply by this constant */
	check_bits(RAD(1.0f), 0x3c8efa35u, "RAD(1)");
	finish();
}

static struct obs_sceneitem_crop crop_of(int left, int top, int right, int bottom)
{
	struct obs_sceneitem_crop c = {left, top, right, bottom};
	return c;
}

static struct obs_scene_item_transform_size prepare(struct obs_sceneitem_crop *crop,
						    struct obs_sceneitem_crop *bounds_crop, uint32_t *lw, uint32_t *lh,
						    uint32_t w, uint32_t h, bool nested)
{
	return obs_scene_item_transform_prepare(crop, bounds_crop, lw, lh, w, h, nested);
}

static void check_crop_line(struct obs_sceneitem_crop c, int left, int top, int right, int bottom, const char *what,
			    int line)
{
	check_int_line(c.left, left, what, line);
	check_int_line(c.top, top, what, line);
	check_int_line(c.right, right, what, line);
	check_int_line(c.bottom, bottom, what, line);
}
#define check_crop(c, l, t, r, b, what) check_crop_line(c, l, t, r, b, what, __LINE__)

static void steps_prepare_crop(void **state)
{
	(void)state;
	struct obs_sceneitem_crop crop, bounds_crop;
	struct obs_scene_item_transform_size size;
	uint32_t lw, lh;

	/* no crop: the whole source; the bounds crop is zeroed whatever it was;
	 * the last size becomes the source size */
	crop = crop_of(0, 0, 0, 0);
	bounds_crop = crop_of(7, -8, INT_MAX, INT_MIN);
	lw = 11;
	lh = 22;
	size = prepare(&crop, &bounds_crop, &lw, &lh, 1920, 1080, false);
	check_int(size.cx, 1920, "no crop cx");
	check_int(size.cy, 1080, "no crop cy");
	check_crop(bounds_crop, 0, 0, 0, 0, "bounds crop reset");
	check_int(lw, 1920, "last width");
	check_int(lh, 1080, "last height");

	/* a crop on each side, not nested: the crop is kept, even with a size
	 * change */
	crop = crop_of(10, 20, 30, 40);
	lw = 960;
	lh = 540;
	size = prepare(&crop, &bounds_crop, &lw, &lh, 1920, 1080, false);
	check_int(size.cx, 1880, "cropped cx");
	check_int(size.cy, 1020, "cropped cy");
	check_crop(crop, 10, 20, 30, 40, "crop kept");

	/* cropping all of it: 0; more: 2; a negative sum: 2 (characterized, not
	 * endorsed; obs_sceneitem_set_crop() clamps each crop to 0 and up); two
	 * INT_MAX crops sum in 64 bits */
	crop = crop_of(100, 0, 100, 0);
	size = prepare(&crop, &bounds_crop, &lw, &lh, 200, 100, false);
	check_int(size.cx, 0, "cropped to nothing");
	crop = crop_of(100, 0, 101, 0);
	size = prepare(&crop, &bounds_crop, &lw, &lh, 200, 100, false);
	check_int(size.cx, 2, "cropped past the size");
	crop = crop_of(-5, 0, 0, 0);
	size = prepare(&crop, &bounds_crop, &lw, &lh, 200, 100, false);
	check_int(size.cx, 2, "negative crop");
	crop = crop_of(INT_MAX, INT_MAX, INT_MAX, INT_MAX);
	size = prepare(&crop, &bounds_crop, &lw, &lh, UINT32_MAX, UINT32_MAX, false);
	check_int(size.cx, 1, "two INT_MAX crops of UINT32_MAX");
	check_int(size.cy, 1, "two INT_MAX crops of UINT32_MAX, y");
	crop = crop_of(INT_MAX, 0, 0, 0);
	size = prepare(&crop, &bounds_crop, &lw, &lh, UINT32_MAX, 0, false);
	check_int(size.cx, (long long)UINT32_MAX - INT_MAX, "INT_MAX crop of UINT32_MAX");
	check_int(size.cy, 0, "zero height");
	check_int(lw, UINT32_MAX, "last width UINT32_MAX");
	check_int(lh, 0, "last height 0");
	finish();
}

static void steps_prepare_nested(void **state)
{
	(void)state;
	struct obs_sceneitem_crop crop, bounds_crop;
	struct obs_scene_item_transform_size size;
	uint32_t lw, lh;

	/* last_width 0 (never sized): the crop is kept */
	crop = crop_of(10, 20, 30, 40);
	lw = 0;
	lh = 540;
	size = prepare(&crop, &bounds_crop, &lw, &lh, 1920, 1080, true);
	check_crop(crop, 10, 20, 30, 40, "last width 0");
	check_int(size.cx, 1880, "last width 0 cx");
	check_int(lw, 1920, "last width set");

	/* last_height 0 */
	crop = crop_of(10, 20, 30, 40);
	lw = 960;
	lh = 0;
	prepare(&crop, &bounds_crop, &lw, &lh, 1920, 1080, true);
	check_crop(crop, 10, 20, 30, 40, "last height 0");

	/* the nested scene doubled: so does its crop */
	crop = crop_of(10, 20, 30, 40);
	lw = 960;
	lh = 540;
	size = prepare(&crop, &bounds_crop, &lw, &lh, 1920, 1080, true);
	check_crop(crop, 20, 40, 60, 80, "doubled");
	check_int(size.cx, 1840, "doubled cx");
	check_int(size.cy, 960, "doubled cy");

	/* the same size: unchanged */
	size = prepare(&crop, &bounds_crop, &lw, &lh, 1920, 1080, true);
	check_crop(crop, 20, 40, 60, 80, "same size");

	/* shrunk to two thirds: truncated toward 0 */
	crop = crop_of(100, 100, 1, 2);
	lw = 1920;
	lh = 1080;
	prepare(&crop, &bounds_crop, &lw, &lh, 1280, 720, true);
	check_crop(crop, 66, 66, 0, 1, "two thirds");

	/* a different factor per axis */
	crop = crop_of(10, 10, 10, 10);
	lw = 100;
	lh = 100;
	prepare(&crop, &bounds_crop, &lw, &lh, 300, 50, true);
	check_crop(crop, 30, 5, 30, 5, "per axis");

	/* to 0 x 0: every crop 0 */
	crop = crop_of(10, 20, 30, 40);
	lw = 1920;
	lh = 1080;
	size = prepare(&crop, &bounds_crop, &lw, &lh, 0, 0, true);
	check_crop(crop, 0, 0, 0, 0, "to zero");
	check_int(size.cx, 0, "to zero cx");
	check_int(lw, 0, "last width 0 after");

	/* a huge factor saturates; a negative crop scales negative
	 * (characterized, not endorsed) */
	crop = crop_of(1000, -3, 0, 7);
	lw = 1;
	lh = 1;
	size = prepare(&crop, &bounds_crop, &lw, &lh, UINT32_MAX, 2, true);
	check_crop(crop, INT_MAX, -6, 0, 14, "saturated and negative");
	check_int(size.cx, 2147483648, "saturated cx");
	check_int(size.cy, 2, "negative crop cy");

	/* sizes past 2^24 convert to float rounded: 16777217 / 16777216 is 1 */
	crop = crop_of(1000, 0, 0, 0);
	lw = 16777216;
	lh = 1;
	prepare(&crop, &bounds_crop, &lw, &lh, 16777217, 1, true);
	check_crop(crop, 1000, 0, 0, 0, "past 2^24");
	finish();
}

static void steps_bounded(void **state)
{
	(void)state;
	check_int(obs_scene_item_transform_bounded(OBS_BOUNDS_NONE), 0, "NONE");
	for (int t = OBS_BOUNDS_STRETCH; t <= OBS_BOUNDS_MAX_ONLY; t++)
		check_int(obs_scene_item_transform_bounded(t), 1, "a bounds type");
	/* Characterized, not endorsed: anything but 0 is bounded. A value
	 * outside the enum is OBS_BOUNDS_NONE before compute_item_transform()
	 * runs (#2281, test_scene_bounds_type_range). */
	check_int(obs_scene_item_transform_bounded(7), 1, "7");
	check_int(obs_scene_item_transform_bounded(-1), 1, "-1");
	check_int(obs_scene_item_transform_bounded(INT_MAX), 1, "INT_MAX");
	check_int(obs_scene_item_transform_bounded(INT_MIN), 1, "INT_MIN");
	finish();
}

static void steps_box_scale(void **state)
{
	(void)state;
	struct vec2 s;

	vec2_set(&s, 1.5f, -2.0f);
	obs_scene_item_transform_box_scale(&s, 1920, 1080);
	check_bits(s.x, 0x45340000u, "1.5 * 1920");
	check_bits(s.y, 0xc5070000u, "-2 * 1080");

	/* the size converts to float first, rounded: 16777217 is 16777216 */
	vec2_set(&s, 1.0f, 3.0f);
	obs_scene_item_transform_box_scale(&s, 16777217, UINT32_MAX);
	check_bits(s.x, 0x4b800000u, "16777217 as float");
	check_bits(s.y, 0x50400000u, "3 * UINT32_MAX as float");

	/* one rounding of the float product */
	vec2_set(&s, 0.1f, 1.0f / 3.0f);
	obs_scene_item_transform_box_scale(&s, 3, 7);
	check_bits(s.x, 0x3e99999au, "0.1 * 3");
	check_bits(s.y, 0x40155556u, "1/3 * 7");

	/* zero size and signed zeros */
	vec2_set(&s, -1.0f, 1.0f);
	obs_scene_item_transform_box_scale(&s, 0, 0);
	check_bits(s.x, 0x80000000u, "-1 * 0");
	check_bits(s.y, 0x00000000u, "1 * 0");

	/* infinities and NaN pass through the multiply */
	vec2_set(&s, INFINITY, bits_float(0x7fc01234u));
	obs_scene_item_transform_box_scale(&s, 0, 5);
	check_int(isnan(s.x) != 0, 1, "inf * 0");
	check_bits(s.y, 0x7fc01234u, "NaN payload");
	finish();
}

static void check_matrix_line(const struct matrix4 *m, const uint32_t want[16], const char *what, int line)
{
	const float *f[16] = {&m->x.x, &m->x.y, &m->x.z, &m->x.w, &m->y.x, &m->y.y, &m->y.z, &m->y.w,
			      &m->z.x, &m->z.y, &m->z.z, &m->z.w, &m->t.x, &m->t.y, &m->t.z, &m->t.w};
	for (int i = 0; i < 16; i++) {
		if (float_bits(*f[i]) != want[i]) {
			fprintf(stderr, "%s, line %d, lane %d: 0x%08x, expected 0x%08x\n", what, line, i,
				float_bits(*f[i]), want[i]);
			failures++;
		}
	}
}
#define check_matrix(m, want, what) check_matrix_line(m, want, what, __LINE__)

static void matrix_of(struct matrix4 *m, float sx, float sy, float ox, float oy, float rot, float px, float py)
{
	struct vec2 scale, origin, position;
	vec2_set(&scale, sx, sy);
	vec2_set(&origin, ox, oy);
	vec2_set(&position, px, py);
	memset(m, 0x5a, sizeof(*m));
	obs_scene_item_transform_matrix(m, &scale, &origin, rot, &position);
}

/* The matrix calls compute_item_transform() made, in its order. */
static void reference_matrix(struct matrix4 *m, float sx, float sy, float ox, float oy, float rot, float px, float py)
{
	matrix4_identity(m);
	matrix4_scale3f(m, m, sx, sy, 1.0f);
	matrix4_translate3f(m, m, -ox, -oy, 0.0f);
	matrix4_rotate_aa4f(m, m, 0.0f, 0.0f, 1.0f, RAD(rot));
	matrix4_translate3f(m, m, px, py, 0.0f);
}

static bool matrix_equal(const struct matrix4 *a, const struct matrix4 *b)
{
	return memcmp(a, b, sizeof(*a)) == 0;
}

static void steps_matrix_rot0(void **state)
{
	(void)state;
	struct matrix4 m;

	/* the identity */
	matrix_of(&m, 1.0f, 1.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f);
	static const uint32_t identity[16] = {0x3f800000u, 0, 0,           0, 0, 0x3f800000u, 0, 0,
					      0,           0, 0x3f800000u, 0, 0, 0,           0, 0x3f800000u};
	check_matrix(&m, identity, "identity");

	/* scale (2, 3), origin (10, 20), position (100, 200): the origin is in
	 * scaled units, so it is subtracted as it is, then the position added */
	matrix_of(&m, 2.0f, 3.0f, 10.0f, 20.0f, 0.0f, 100.0f, 200.0f);
	static const uint32_t scaled[16] = {0x40000000u, 0, 0,           0, 0,           0x40400000u, 0, 0,
					    0,           0, 0x3f800000u, 0, 0x42b40000u, 0x43340000u, 0, 0x3f800000u};
	check_matrix(&m, scaled, "scaled");

	/* flipped: the scale's sign, and the signed zeros it makes */
	matrix_of(&m, -1.5f, -1.5f, 300.0f, 150.0f, 0.0f, 100.0f, 50.0f);
	static const uint32_t flipped[16] = {
		0xbfc00000u, 0x00000000u, 0x00000000u, 0x00000000u, 0x00000000u, 0xbfc00000u, 0x00000000u, 0x00000000u,
		0x00000000u, 0x00000000u, 0x3f800000u, 0x00000000u, 0xc3480000u, 0xc2c80000u, 0x00000000u, 0x3f800000u,
	};
	check_matrix(&m, flipped, "flipped");

	/* -0 rotation is exact too */
	matrix_of(&m, 2.0f, 3.0f, 10.0f, 20.0f, -0.0f, 100.0f, 200.0f);
	check_matrix(&m, scaled, "-0 rotation");
	finish();
}

static void steps_matrix_rotations(void **state)
{
	(void)state;
	static const float rots[] = {0.0f, 90.0f, 45.0f, -30.0f, 720.0f, 180.0f, 360.0f, -90.0f};
	static const uint32_t rad_bits[] = {0x00000000u, 0x3fc90fdbu, 0x3f490fdbu, 0xbf060a92u,
					    0x41490fdbu, 0x40490fdbu, 0x40c90fdbu, 0xbfc90fdbu};
	struct matrix4 m, ref;

	for (size_t i = 0; i < sizeof(rots) / sizeof(rots[0]); i++) {
		check_bits(RAD(rots[i]), rad_bits[i], "RAD(rot)");
		/* each order of the same calls gives the matrix the C makes */
		matrix_of(&m, 1.5f, -2.0f, 150.0f, 50.0f, rots[i], 100.0f, 50.0f);
		reference_matrix(&ref, 1.5f, -2.0f, 150.0f, 50.0f, rots[i], 100.0f, 50.0f);
		check_int(matrix_equal(&m, &ref), 1, "the matrix4 calls in order");
	}

	/* rotating before the origin translation is a different matrix */
	matrix_of(&m, 1.5f, -2.0f, 150.0f, 50.0f, 45.0f, 100.0f, 50.0f);
	matrix4_identity(&ref);
	matrix4_scale3f(&ref, &ref, 1.5f, -2.0f, 1.0f);
	matrix4_rotate_aa4f(&ref, &ref, 0.0f, 0.0f, 1.0f, RAD(45.0f));
	matrix4_translate3f(&ref, &ref, -150.0f, -50.0f, 0.0f);
	matrix4_translate3f(&ref, &ref, 100.0f, 50.0f, 0.0f);
	check_int(matrix_equal(&m, &ref), 0, "rotation before the origin");

#if defined(__GLIBC__) && defined(__x86_64__)
	/* the bits, with glibc's sinf and cosf */
	static const uint32_t rot90[16] = {
		0x00000000u, 0x3fc00000u, 0x00000000u, 0x00000000u, 0x40000000u, 0x00000000u, 0x00000000u, 0x00000000u,
		0x00000000u, 0x00000000u, 0x3f800000u, 0x00000000u, 0x43160000u, 0xc2c80000u, 0x00000000u, 0x3f800000u,
	};
	static const uint32_t rot45[16] = {
		0x3f87c3b6u, 0x3f87c3b7u, 0x00000000u, 0x00000000u, 0x3fb504f4u, 0xbfb504f2u, 0x00000000u, 0x00000000u,
		0x00000000u, 0x00000000u, 0x3f800000u, 0x00000000u, 0x41ea5090u, 0xc2b6d7bcu, 0x00000000u, 0x3f800000u,
	};
	static const uint32_t rotm30[16] = {
		0x3fa646e1u, 0xbf400002u, 0x00000000u, 0x00000000u, 0xbf800001u, 0xbfddb3d7u, 0x00000000u, 0x00000000u,
		0x00000000u, 0x00000000u, 0x3f800000u, 0x00000000u, 0xc25b9d80u, 0x42a365c1u, 0x00000000u, 0x3f800000u,
	};
	static const uint32_t rot720[16] = {
		0x3fc00000u, 0x350ccde2u, 0x00000000u, 0x00000000u, 0x353bbd2eu, 0xc0000000u, 0x00000000u, 0x00000000u,
		0x00000000u, 0x00000000u, 0x3f800000u, 0x00000000u, 0xc247fffcu, 0xb8600000u, 0x00000000u, 0x3f800000u,
	};
	matrix_of(&m, 1.5f, -2.0f, 150.0f, 50.0f, 90.0f, 100.0f, 50.0f);
	check_matrix(&m, rot90, "90");
	matrix_of(&m, 1.5f, -2.0f, 150.0f, 50.0f, 45.0f, 100.0f, 50.0f);
	check_matrix(&m, rot45, "45");
	matrix_of(&m, 1.5f, -2.0f, 150.0f, 50.0f, -30.0f, 100.0f, 50.0f);
	check_matrix(&m, rotm30, "-30");
	matrix_of(&m, 1.5f, -2.0f, 150.0f, 50.0f, 720.0f, 100.0f, 50.0f);
	check_matrix(&m, rot720, "720");
#endif
	finish();
}

/* draw and box translations and scales of one composition */
static void check_placed_line(const struct item *it, uint32_t draw_tx, uint32_t draw_ty, uint32_t box_tx,
			      uint32_t box_ty, const char *what, int line)
{
	check_bits_line(it->draw.t.x, draw_tx, what, line);
	check_bits_line(it->draw.t.y, draw_ty, what, line);
	check_bits_line(it->box.t.x, box_tx, what, line);
	check_bits_line(it->box.t.y, box_ty, what, line);
}
#define check_placed(it, a, b, c, d, what) check_placed_line(it, a, b, c, d, what, __LINE__)

static void steps_compose_alignments(void **state)
{
	(void)state;
	/* An unbounded 200 x 100 source at (100, 50), scale 1.5: the alignment
	 * moves the origin by none, half or all of the scaled size (300 x 150)
	 * on each axis, and the box by the box scale. */
	static const uint32_t want[N_ALIGN][4] = {
		{0xc2480000u, 0xc1c80000u, 0xc2480000u, 0xc1c80000u}, /* -50 -25 -50 -25 */
		{0x42c80000u, 0xc1c80000u, 0x42c80000u, 0xc1c80000u}, /* 100 -25 100 -25 */
		{0xc3480000u, 0xc1c80000u, 0xc3480000u, 0xc1c80000u}, /* -200 -25 -200 -25 */
		{0xc2480000u, 0x42480000u, 0xc2480000u, 0x42480000u}, /* -50 50 -50 50 */
		{0x42c80000u, 0x42480000u, 0x42c80000u, 0x42480000u}, /* 100 50 100 50 */
		{0xc3480000u, 0x42480000u, 0xc3480000u, 0x42480000u}, /* -200 50 -200 50 */
		{0xc2480000u, 0xc2c80000u, 0xc2480000u, 0xc2c80000u}, /* -50 -100 -50 -100 */
		{0x42c80000u, 0xc2c80000u, 0x42c80000u, 0xc2c80000u}, /* 100 -100 100 -100 */
		{0xc3480000u, 0xc2c80000u, 0xc3480000u, 0xc2c80000u}, /* -200 -100 -200 -100 */
		{0xc3480000u, 0xc2c80000u, 0xc3480000u, 0xc2c80000u}, /* -200 -100 -200 -100 */
		{0x42c80000u, 0x42480000u, 0x42c80000u, 0x42480000u}, /* 100 50 100 50 */
	};
	for (size_t a = 0; a < N_ALIGN; a++) {
		struct item it = base_item();
		it.align = alignments[a];
		compose(&it, SRC_CX, SRC_CY, false, 1920.0f, 1080.0f);
		check_placed(&it, want[a][0], want[a][1], want[a][2], want[a][3], "alignment");
		check_int(it.cx, 300, "scaled cx");
		check_int(it.cy, 150, "scaled cy");
		check_bits(it.output_scale.x, 0x3fc00000u, "output scale");
		check_bits(it.box_scale.x, 0x43960000u, "box scale x 300");
		check_bits(it.box_scale.y, 0x43160000u, "box scale y 150");
	}
	finish();
}

static void steps_compose_flipped(void **state)
{
	(void)state;
	/* Flipped items without bounds: (float)cx * scale.x is negative, and
	 * the cast to uint32_t is x86-64's (the width wraps, and add_alignment()
	 * gets the negative width back as an int). arm64 gives 0 there: such an
	 * item sits half or all of its size away on arm64 (R7-3), which
	 * obs-scene.c keeps; these pins are x86-64's on every platform. */
	static const uint32_t want[4][N_ALIGN][4] = {
		{
			{0xc2480000u, 0xc1c80000u, 0xc2480000u, 0xc1c80000u}, /* -50 -25 -50 -25 */
			{0x42c80000u, 0xc1c80000u, 0x42c80000u, 0xc1c80000u}, /* 100 -25 100 -25 */
			{0xc3480000u, 0xc1c80000u, 0xc3480000u, 0xc1c80000u}, /* -200 -25 -200 -25 */
			{0xc2480000u, 0x42480000u, 0xc2480000u, 0x42480000u}, /* -50 50 -50 50 */
			{0x42c80000u, 0x42480000u, 0x42c80000u, 0x42480000u}, /* 100 50 100 50 */
			{0xc3480000u, 0x42480000u, 0xc3480000u, 0x42480000u}, /* -200 50 -200 50 */
			{0xc2480000u, 0xc2c80000u, 0xc2480000u, 0xc2c80000u}, /* -50 -100 -50 -100 */
			{0x42c80000u, 0xc2c80000u, 0x42c80000u, 0xc2c80000u}, /* 100 -100 100 -100 */
			{0xc3480000u, 0xc2c80000u, 0xc3480000u, 0xc2c80000u}, /* -200 -100 -200 -100 */
			{0xc3480000u, 0xc2c80000u, 0xc3480000u, 0xc2c80000u}, /* -200 -100 -200 -100 */
			{0x42c80000u, 0x42480000u, 0x42c80000u, 0x42480000u}, /* 100 50 100 50 */
		},
		{
			{0x437a0000u, 0xc1c80000u, 0x437a0000u, 0xc1c80000u}, /* 250 -25 250 -25 */
			{0x42c80000u, 0xc1c80000u, 0x42c80000u, 0xc1c80000u}, /* 100 -25 100 -25 */
			{0x43c80000u, 0xc1c80000u, 0x43c80000u, 0xc1c80000u}, /* 400 -25 400 -25 */
			{0x437a0000u, 0x42480000u, 0x437a0000u, 0x42480000u}, /* 250 50 250 50 */
			{0x42c80000u, 0x42480000u, 0x42c80000u, 0x42480000u}, /* 100 50 100 50 */
			{0x43c80000u, 0x42480000u, 0x43c80000u, 0x42480000u}, /* 400 50 400 50 */
			{0x437a0000u, 0xc2c80000u, 0x437a0000u, 0xc2c80000u}, /* 250 -100 250 -100 */
			{0x42c80000u, 0xc2c80000u, 0x42c80000u, 0xc2c80000u}, /* 100 -100 100 -100 */
			{0x43c80000u, 0xc2c80000u, 0x43c80000u, 0xc2c80000u}, /* 400 -100 400 -100 */
			{0x43c80000u, 0xc2c80000u, 0x43c80000u, 0xc2c80000u}, /* 400 -100 400 -100 */
			{0x42c80000u, 0x42480000u, 0x42c80000u, 0x42480000u}, /* 100 50 100 50 */
		},
		{
			{0xc2480000u, 0x42fa0000u, 0xc2480000u, 0x42fa0000u}, /* -50 125 -50 125 */
			{0x42c80000u, 0x42fa0000u, 0x42c80000u, 0x42fa0000u}, /* 100 125 100 125 */
			{0xc3480000u, 0x42fa0000u, 0xc3480000u, 0x42fa0000u}, /* -200 125 -200 125 */
			{0xc2480000u, 0x42480000u, 0xc2480000u, 0x42480000u}, /* -50 50 -50 50 */
			{0x42c80000u, 0x42480000u, 0x42c80000u, 0x42480000u}, /* 100 50 100 50 */
			{0xc3480000u, 0x42480000u, 0xc3480000u, 0x42480000u}, /* -200 50 -200 50 */
			{0xc2480000u, 0x43480000u, 0xc2480000u, 0x43480000u}, /* -50 200 -50 200 */
			{0x42c80000u, 0x43480000u, 0x42c80000u, 0x43480000u}, /* 100 200 100 200 */
			{0xc3480000u, 0x43480000u, 0xc3480000u, 0x43480000u}, /* -200 200 -200 200 */
			{0xc3480000u, 0x43480000u, 0xc3480000u, 0x43480000u}, /* -200 200 -200 200 */
			{0x42c80000u, 0x42480000u, 0x42c80000u, 0x42480000u}, /* 100 50 100 50 */
		},
		{
			{0x437a0000u, 0x42fa0000u, 0x437a0000u, 0x42fa0000u}, /* 250 125 250 125 */
			{0x42c80000u, 0x42fa0000u, 0x42c80000u, 0x42fa0000u}, /* 100 125 100 125 */
			{0x43c80000u, 0x42fa0000u, 0x43c80000u, 0x42fa0000u}, /* 400 125 400 125 */
			{0x437a0000u, 0x42480000u, 0x437a0000u, 0x42480000u}, /* 250 50 250 50 */
			{0x42c80000u, 0x42480000u, 0x42c80000u, 0x42480000u}, /* 100 50 100 50 */
			{0x43c80000u, 0x42480000u, 0x43c80000u, 0x42480000u}, /* 400 50 400 50 */
			{0x437a0000u, 0x43480000u, 0x437a0000u, 0x43480000u}, /* 250 200 250 200 */
			{0x42c80000u, 0x43480000u, 0x42c80000u, 0x43480000u}, /* 100 200 100 200 */
			{0x43c80000u, 0x43480000u, 0x43c80000u, 0x43480000u}, /* 400 200 400 200 */
			{0x43c80000u, 0x43480000u, 0x43c80000u, 0x43480000u}, /* 400 200 400 200 */
			{0x42c80000u, 0x42480000u, 0x42c80000u, 0x42480000u}, /* 100 50 100 50 */
		},
	};
	for (int s = 0; s < 4; s++) {
		for (size_t a = 0; a < N_ALIGN; a++) {
			struct item it = base_item();
			it.align = alignments[a];
			it.scale.x *= sign_x[s];
			it.scale.y *= sign_y[s];
			compose(&it, SRC_CX, SRC_CY, false, 1920.0f, 1080.0f);
			check_placed(&it, want[s][a][0], want[s][a][1], want[s][a][2], want[s][a][3], "flipped");
			check_int(it.cx, sign_x[s] < 0 ? (long long)(uint32_t)-300 : 300, "x86 cast cx");
			check_int(it.cy, sign_y[s] < 0 ? (long long)(uint32_t)-150 : 150, "x86 cast cy");
		}
	}
	finish();
}

static void steps_compose_bounds(void **state)
{
	(void)state;
	/* Each bounds type, an 800 x 600 box, the source centred in it, crop to
	 * bounds off and on, upright and flipped on both axes. */
	static const uint32_t want[7][2][2][8] = {
		{
			{
				{300u, 150u, 0x3fc00000u, 0x3fc00000u, 0x43960000u, 0x43160000u, 0x42c80000u,
				 0x42480000u}, /* 1.5 1.5 300 150 100 50 */
				{4294966996u, 4294967146u, 0xbfc00000u, 0xbfc00000u, 0xc3960000u, 0xc3160000u,
				 0x42c80000u, 0x42480000u}, /* -1.5 -1.5 -300 -150 100 50 */
			},
			{
				{300u, 150u, 0x3fc00000u, 0x3fc00000u, 0x43960000u, 0x43160000u, 0x42c80000u,
				 0x42480000u}, /* 1.5 1.5 300 150 100 50 */
				{4294966996u, 4294967146u, 0xbfc00000u, 0xbfc00000u, 0xc3960000u, 0xc3160000u,
				 0x42c80000u, 0x42480000u}, /* -1.5 -1.5 -300 -150 100 50 */
			},
		},
		{
			{
				{400u, 400u, 0x40000000u, 0x40800000u, 0x43c80000u, 0x43c80000u, 0x42c80000u,
				 0x42480000u}, /* 2 4 400 400 100 50 */
				{400u, 400u, 0xc0000000u, 0xc0800000u, 0x43c80000u, 0x43c80000u, 0x43fa0000u,
				 0x43e10000u}, /* -2 -4 400 400 500 450 */
			},
			{
				{400u, 400u, 0x40000000u, 0x40800000u, 0x43c80000u, 0x43c80000u, 0x42c80000u,
				 0x42480000u}, /* 2 4 400 400 100 50 */
				{400u, 400u, 0xc0000000u, 0xc0800000u, 0x43c80000u, 0x43c80000u, 0x43fa0000u,
				 0x43e10000u}, /* -2 -4 400 400 500 450 */
			},
		},
		{
			{
				{400u, 400u, 0x40000000u, 0x40000000u, 0x43c80000u, 0x43c80000u, 0x42c80000u,
				 0x43160000u}, /* 2 2 400 400 100 150 */
				{400u, 400u, 0xc0000000u, 0xc0000000u, 0x43c80000u, 0x43c80000u, 0x43fa0000u,
				 0x43af0000u}, /* -2 -2 400 400 500 350 */
			},
			{
				{400u, 400u, 0x40000000u, 0x40000000u, 0x43c80000u, 0x43c80000u, 0x42c80000u,
				 0x43160000u}, /* 2 2 400 400 100 150 */
				{400u, 400u, 0xc0000000u, 0xc0000000u, 0x43c80000u, 0x43c80000u, 0x43fa0000u,
				 0x43af0000u}, /* -2 -2 400 400 500 350 */
			},
		},
		{
			{
				{400u, 400u, 0x40800000u, 0x40800000u, 0x43c80000u, 0x43c80000u, 0xc2c80000u,
				 0x42480000u}, /* 4 4 400 400 -100 50 */
				{400u, 400u, 0xc0800000u, 0xc0800000u, 0x43c80000u, 0x43c80000u, 0x442f0000u,
				 0x43e10000u}, /* -4 -4 400 400 700 450 */
			},
			{
				{400u, 400u, 0x40800000u, 0x40800000u, 0x43c80000u, 0x43c80000u, 0x42c80000u,
				 0x42480000u}, /* 4 4 400 400 100 50 */
				{400u, 400u, 0xc0800000u, 0xc0800000u, 0x43c80000u, 0x43c80000u, 0x43fa0000u,
				 0x43e10000u}, /* -4 -4 400 400 500 450 */
			},
		},
		{
			{
				{400u, 400u, 0x40000000u, 0x40000000u, 0x43c80000u, 0x43c80000u, 0x42c80000u,
				 0x43160000u}, /* 2 2 400 400 100 150 */
				{400u, 400u, 0xc0000000u, 0xc0000000u, 0x43c80000u, 0x43c80000u, 0x43fa0000u,
				 0x43af0000u}, /* -2 -2 400 400 500 350 */
			},
			{
				{400u, 400u, 0x40000000u, 0x40000000u, 0x43c80000u, 0x43c80000u, 0x42c80000u,
				 0x43160000u}, /* 2 2 400 400 100 150 */
				{400u, 400u, 0xc0000000u, 0xc0000000u, 0x43c80000u, 0x43c80000u, 0x43fa0000u,
				 0x43af0000u}, /* -2 -2 400 400 500 350 */
			},
		},
		{
			{
				{400u, 400u, 0x40800000u, 0x40800000u, 0x43c80000u, 0x43c80000u, 0xc2c80000u,
				 0x42480000u}, /* 4 4 400 400 -100 50 */
				{400u, 400u, 0xc0800000u, 0xc0800000u, 0x43c80000u, 0x43c80000u, 0x442f0000u,
				 0x43e10000u}, /* -4 -4 400 400 700 450 */
			},
			{
				{400u, 400u, 0x40800000u, 0x40800000u, 0x43c80000u, 0x43c80000u, 0x42c80000u,
				 0x42480000u}, /* 4 4 400 400 100 50 */
				{400u, 400u, 0xc0800000u, 0xc0800000u, 0x43c80000u, 0x43c80000u, 0x43fa0000u,
				 0x43e10000u}, /* -4 -4 400 400 500 450 */
			},
		},
		{
			{
				{400u, 400u, 0x3fc00000u, 0x3fc00000u, 0x43c80000u, 0x43c80000u, 0x43160000u,
				 0x432f0000u}, /* 1.5 1.5 400 400 150 175 */
				{400u, 400u, 0xbfc00000u, 0xbfc00000u, 0x43c80000u, 0x43c80000u, 0x43e10000u,
				 0x43a28000u}, /* -1.5 -1.5 400 400 450 325 */
			},
			{
				{400u, 400u, 0x3fc00000u, 0x3fc00000u, 0x43c80000u, 0x43c80000u, 0x43160000u,
				 0x432f0000u}, /* 1.5 1.5 400 400 150 175 */
				{400u, 400u, 0xbfc00000u, 0xbfc00000u, 0x43c80000u, 0x43c80000u, 0x43e10000u,
				 0x43a28000u}, /* -1.5 -1.5 400 400 450 325 */
			},
		},
	};
	static const int want_crop[7][2][2][4] = {
		{
			{
				{0, 0, 0, 0},
				{0, 0, 0, 0},
			},
			{
				{0, 0, 0, 0},
				{0, 0, 0, 0},
			},
		},
		{
			{
				{0, 0, 0, 0},
				{0, 0, 0, 0},
			},
			{
				{0, 0, 0, 0},
				{0, 0, 0, 0},
			},
		},
		{
			{
				{0, 0, 0, 0},
				{0, 0, 0, 0},
			},
			{
				{0, 0, 0, 0},
				{0, 0, 0, 0},
			},
		},
		{
			{
				{0, 0, 0, 0},
				{0, 0, 0, 0},
			},
			{
				{50, 0, 50, 0},
				{50, 0, 50, 0},
			},
		},
		{
			{
				{0, 0, 0, 0},
				{0, 0, 0, 0},
			},
			{
				{0, 0, 0, 0},
				{0, 0, 0, 0},
			},
		},
		{
			{
				{0, 0, 0, 0},
				{0, 0, 0, 0},
			},
			{
				{50, 0, 50, 0},
				{50, 0, 50, 0},
			},
		},
		{
			{
				{0, 0, 0, 0},
				{0, 0, 0, 0},
			},
			{
				{0, 0, 0, 0},
				{0, 0, 0, 0},
			},
		},
	};
	for (int t = 0; t <= OBS_BOUNDS_MAX_ONLY; t++) {
		for (int c = 0; c < 2; c++) {
			for (int f = 0; f < 2; f++) {
				struct item it = base_item();
				it.bounds_type = t;
				it.crop_to_bounds = c;
				it.scale.x = f ? -1.5f : 1.5f;
				it.scale.y = f ? -1.5f : 1.5f;
				it.bounds.x = 400.0f;
				it.bounds.y = 400.0f;
				compose(&it, SRC_CX, SRC_CY, false, 1920.0f, 1080.0f);
				const uint32_t *w = want[t][c][f];
				check_int(it.cx, w[0], "bounds cx");
				check_int(it.cy, w[1], "bounds cy");
				check_bits(it.output_scale.x, w[2], "output scale x");
				check_bits(it.output_scale.y, w[3], "output scale y");
				check_bits(it.box_scale.x, w[4], "box scale x");
				check_bits(it.box_scale.y, w[5], "box scale y");
				check_bits(it.draw.t.x, w[6], "draw tx");
				check_bits(it.draw.t.y, w[7], "draw ty");
				const int *wc = want_crop[t][c][f];
				check_crop(it.bounds_crop, wc[0], wc[1], wc[2], wc[3], "bounds crop");
			}
		}
	}
	finish();
}

static void steps_compose_relative(void **state)
{
	(void)state;
	/* Relative coordinates: the position, the scale and the bounds are read
	 * against the canvas (1920 x 1080 here, 1280 x 720 for the second), and
	 * a group's items are not nudged. */
	static const uint32_t want[6][8] = {
		{0x3f400000u, 0x3f400000u, 0x43160000u, 0x42960000u, 0x447f0000u, 0x44414000u, 0x447f0000u,
		 0x44414000u}, /* 0.75 0.75 150 75 1020 773 1020 773 */
		{0x3facccccu, 0x3facccccu, 0x43870000u, 0x43070000u, 0x44700000u, 0x4439c000u, 0x44700000u,
		 0x4439c000u}, /* 1.35 1.35 270 135 960 743 960 743 */
		{0x3f400000u, 0x3f400000u, 0x43160000u, 0x42960000u, 0x447f0000u, 0x44414000u, 0x447f0000u,
		 0x44414000u}, /* 0.75 0.75 150 75 1020 773 1020 773 */
		{0x3f000000u, 0x3f000000u, 0x42c80000u, 0x42480000u, 0x442a0000u, 0x4400c000u, 0x442a0000u,
		 0x4400c000u}, /* 0.5 0.5 100 50 680 515 680 515 */
		{0x3f666666u, 0x3f666666u, 0x43340000u, 0x42b40000u, 0x44200000u, 0x43f78000u, 0x44200000u,
		 0x43f78000u}, /* 0.9 0.9 180 90 640 495 640 495 */
		{0x3f400000u, 0x3f400000u, 0x43160000u, 0x42960000u, 0x4423c000u, 0x43fb8000u, 0x4423c000u,
		 0x43fb8000u}, /* 0.75 0.75 150 75 655 503 655 503 */
	};
	struct item it;
	int i = 0;
	for (int canvas = 0; canvas < 2; canvas++) {
		float cw = canvas ? 1280.0f : 1920.0f;
		float ch = canvas ? 720.0f : 1080.0f;
		for (int k = 0; k < 3; k++, i++) {
			it = base_item();
			it.absolute = false;
			it.pos.x = 0.25f;
			it.pos.y = 0.5f;
			it.scale.x = 0.75f;
			it.scale.y = 0.75f;
			it.bounds.x = 0.5f;
			it.bounds.y = 0.25f;
			it.align = OBS_ALIGN_CENTER;
			if (k == 1)
				it.bounds_type = OBS_BOUNDS_SCALE_INNER;
			if (k == 2) {
				it.parent_is_group = true;
				it.is_scene = true;
			}
			compose(&it, SRC_CX, SRC_CY, false, cw, ch);
			check_bits(it.output_scale.x, want[i][0], "relative output scale x");
			check_bits(it.output_scale.y, want[i][1], "relative output scale y");
			check_bits(it.box_scale.x, want[i][2], "relative box scale x");
			check_bits(it.box_scale.y, want[i][3], "relative box scale y");
			check_bits(it.draw.t.x, want[i][4], "relative draw tx");
			check_bits(it.draw.t.y, want[i][5], "relative draw ty");
			check_bits(it.box.t.x, want[i][6], "relative box tx");
			check_bits(it.box.t.y, want[i][7], "relative box ty");
		}
	}
	finish();
}

static void steps_compose_nested_crop(void **state)
{
	(void)state;
	struct item it;

	/* a nested scene seen for the first time (last_width 0): its crop is
	 * kept and the last size set */
	it = base_item();
	it.is_scene = true;
	it.crop = crop_of(10, 20, 30, 40);
	compose(&it, 1920, 1080, true, 1920.0f, 1080.0f);
	check_crop(it.crop, 10, 20, 30, 40, "first size");
	check_int(it.last_width, 1920, "first last width");
	check_int(it.last_height, 1080, "first last height");
	check_int(it.cx, 2820, "first cx: (1920 - 40) * 1.5");
	check_int(it.cy, 1530, "first cy: (1080 - 60) * 1.5");

	/* the scene shrinks to half: so does the crop */
	compose(&it, 960, 540, true, 1920.0f, 1080.0f);
	check_crop(it.crop, 5, 10, 15, 20, "half");
	check_int(it.cx, 1410, "half cx: (960 - 20) * 1.5");
	check_int(it.cy, 765, "half cy: (540 - 30) * 1.5");
	check_bits(it.box_scale.x, 0x44b04000u, "half box scale x 1410");
	check_bits(it.box_scale.y, 0x443f4000u, "half box scale y 765");

	/* without update_tex (a setter's update): the crop is kept through a
	 * size change, but the last size still follows */
	compose(&it, 1920, 1080, false, 1920.0f, 1080.0f);
	check_crop(it.crop, 5, 10, 15, 20, "no update_tex");
	check_int(it.last_width, 1920, "no update_tex last width");

	/* not a scene: kept */
	it = base_item();
	it.crop = crop_of(10, 20, 30, 40);
	it.last_width = 960;
	it.last_height = 540;
	compose(&it, 1920, 1080, true, 1920.0f, 1080.0f);
	check_crop(it.crop, 10, 20, 30, 40, "not a scene");

	/* a nested scene with bounds: the bounds crop of the last composition
	 * is reset before the cropped size is taken */
	it = base_item();
	it.is_scene = true;
	it.bounds_type = OBS_BOUNDS_SCALE_OUTER;
	it.crop_to_bounds = true;
	it.bounds.x = 100.0f;
	it.bounds.y = 100.0f;
	compose(&it, 200, 100, true, 1920.0f, 1080.0f);
	check_crop(it.bounds_crop, 50, 0, 50, 0, "outer crop");
	check_int(it.cx, 100, "outer cx");
	compose(&it, 200, 100, true, 1920.0f, 1080.0f);
	check_crop(it.bounds_crop, 50, 0, 50, 0, "outer crop again, not doubled");
	finish();
}

/* Every alignment x bounds type x crop x absolute/relative x scale sign at
 * one rotation, folded into one hash. */
static uint64_t sweep(float rot)
{
	uint64_t h = FNV_BASIS;
	for (size_t a = 0; a < N_ALIGN; a++) {
		for (int t = 0; t <= OBS_BOUNDS_MAX_ONLY; t++) {
			for (int crop = 0; crop < 2; crop++) {
				for (int rel = 0; rel < 2; rel++) {
					for (int s = 0; s < 4; s++) {
						struct item it = base_item();
						it.align = alignments[a];
						it.bounds_type = t;
						it.bounds_align = alignments[(a + 3) % N_ALIGN];
						it.crop_to_bounds = crop;
						it.rot = rot;
						if (crop)
							it.crop = crop_of(10, 5, 30, 15);
						if (rel) {
							it.absolute = false;
							it.pos.x = 0.25f;
							it.pos.y = 0.5f;
							it.scale.x = 0.75f;
							it.scale.y = 0.75f;
							it.bounds.x = 0.5f;
							it.bounds.y = 0.25f;
						}
						it.scale.x *= sign_x[s];
						it.scale.y *= sign_y[s];
						compose(&it, SRC_CX, SRC_CY, false, 1920.0f, 1080.0f);
						h = fnv_item(h, &it);
					}
				}
			}
		}
	}
	return h;
}

static void steps_compose_sweep(void **state)
{
	(void)state;
	check_u64(sweep(0.0f), 0x2a4dd4556ee70059ull, "sweep at 0");
	check_u64(sweep(-0.0f), 0x2a4dd4556ee70059ull, "sweep at -0");
#if defined(__GLIBC__) && defined(__x86_64__)
	check_u64(sweep(90.0f), 0xfab5343e4ab17d6dull, "sweep at 90");
	check_u64(sweep(45.0f), 0xbc167d4b772bf747ull, "sweep at 45");
	check_u64(sweep(-30.0f), 0x7134363196439e8dull, "sweep at -30");
	check_u64(sweep(720.0f), 0x7cb0d4f8e2c7cb3dull, "sweep at 720");
#endif
	finish();
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(steps_layout),           cmocka_unit_test(steps_prepare_crop),
		cmocka_unit_test(steps_prepare_nested),   cmocka_unit_test(steps_bounded),
		cmocka_unit_test(steps_box_scale),        cmocka_unit_test(steps_matrix_rot0),
		cmocka_unit_test(steps_matrix_rotations), cmocka_unit_test(steps_compose_alignments),
		cmocka_unit_test(steps_compose_flipped),  cmocka_unit_test(steps_compose_bounds),
		cmocka_unit_test(steps_compose_relative), cmocka_unit_test(steps_compose_nested_crop),
		cmocka_unit_test(steps_compose_sweep),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
