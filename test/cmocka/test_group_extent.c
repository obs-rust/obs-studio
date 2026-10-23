/* Pins the current C behavior of the scene content extent.
 *
 * resize_scene_base in obs-scene.c.h folds the four corners of every
 * item's box transform into a min and a max (obs_scene_extent_add), then
 * sizes the scene from max - min (obs_scene_extent_size). The scene still
 * walks its items and still shifts the children, so the test passes the
 * matrices in and needs no display and no scene.
 *
 * None of these symbols are exported from libobs. This executable compiles
 * libobs/obs-scene-extent.c.h itself and characterizes that C. It cannot
 * see a later Rust swap until the symbols are reachable from the test.
 *
 * Cases marked characterized, not endorsed, are what the C does today,
 * including the odd parts. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <float.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include <graphics/math-defs.h>
#include <obs-scene-extent.h>

static uint32_t bits(float f)
{
	uint32_t u;
	memcpy(&u, &f, sizeof(u));
	return u;
}

/* Bit for bit: -0 is not +0. */
static void check_float(float got, float want, const char *what)
{
	if (bits(got) == bits(want))
		return;

	fprintf(stderr, "%s: %g (0x%08x), expected %g (0x%08x)\n", what, got, bits(got), want, bits(want));
	fail();
}

static void check_vec2(const struct vec2 *got, float x, float y, const char *what)
{
	if (bits(got->x) == bits(x) && bits(got->y) == bits(y))
		return;

	fprintf(stderr, "%s: (%g, %g), expected (%g, %g)\n", what, got->x, got->y, x, y);
	fail();
}

/* A box transform: (1, 0) lands on (xx, xy) and (0, 1) on (yx, yy) before
 * the translation (tx, ty). Corner (u, v) is (u * xx + v * yx + tx,
 * u * xy + v * yy + ty). */
static void box(struct matrix4 *m, float xx, float xy, float yx, float yy, float tx, float ty)
{
	matrix4_identity(m);
	vec4_set(&m->x, xx, xy, 0.0f, 0.0f);
	vec4_set(&m->y, yx, yy, 0.0f, 0.0f);
	vec4_set(&m->t, tx, ty, 0.0f, 1.0f);
}

struct extent {
	struct vec2 minv;
	struct vec2 maxv;
	struct vec2 scale;
	uint32_t cx;
	uint32_t cy;
};

/* What resize_scene_base computes for these items, in this order. */
static void measure(struct extent *e, const struct matrix4 *items, size_t count)
{
	obs_scene_extent_init(&e->minv, &e->maxv);
	for (size_t i = 0; i < count; i++)
		obs_scene_extent_add(&e->minv, &e->maxv, &items[i]);
	obs_scene_extent_size(&e->scale, &e->cx, &e->cy, &e->minv, &e->maxv);
}

static void one_item(void **state)
{
	struct matrix4 items[1];
	struct extent e;

	(void)state;

	box(&items[0], 100.0f, 0.0f, 0.0f, 50.0f, 10.0f, 20.0f);
	measure(&e, items, 1);

	check_vec2(&e.minv, 10.0f, 20.0f, "min");
	check_vec2(&e.maxv, 110.0f, 70.0f, "max");
	check_vec2(&e.scale, 100.0f, 50.0f, "scale");
	assert_int_equal(e.cx, 100);
	assert_int_equal(e.cy, 50);
}

/* The extent covers every item, whichever item each edge comes from. */
static void several_items(void **state)
{
	struct matrix4 items[3];
	struct extent e;

	(void)state;

	box(&items[0], 100.0f, 0.0f, 0.0f, 50.0f, 10.0f, 20.0f);
	box(&items[1], 30.0f, 0.0f, 0.0f, 30.0f, -5.0f, 60.0f);
	box(&items[2], 10.0f, 0.0f, 0.0f, 10.0f, 200.0f, -40.0f);
	measure(&e, items, 3);

	check_vec2(&e.minv, -5.0f, -40.0f, "min");
	check_vec2(&e.maxv, 210.0f, 90.0f, "max");
	check_vec2(&e.scale, 215.0f, 130.0f, "scale");
	assert_int_equal(e.cx, 215);
	assert_int_equal(e.cy, 130);
}

/* A box turned 45 degrees: the corners are (50, 50), (60, 60), (40, 60)
 * and (50, 70). The extent is that of the corners, not the box's own
 * translation and size. */
static void rotated_item_uses_the_corners(void **state)
{
	struct matrix4 items[1];
	struct extent e;

	(void)state;

	box(&items[0], 10.0f, 10.0f, -10.0f, 10.0f, 50.0f, 50.0f);
	measure(&e, items, 1);

	check_vec2(&e.minv, 40.0f, 50.0f, "min");
	check_vec2(&e.maxv, 60.0f, 70.0f, "max");
	check_vec2(&e.scale, 20.0f, 20.0f, "scale");
	assert_int_equal(e.cx, 20);
	assert_int_equal(e.cy, 20);
}

/* The size is the extent rounded up; a whole one stays. */
static void fractional_extent_rounds_up(void **state)
{
	struct matrix4 items[1];
	struct extent e;

	(void)state;

	box(&items[0], 99.5f, 0.0f, 0.0f, 0.25f, 0.5f, -0.75f);
	measure(&e, items, 1);

	check_vec2(&e.minv, 0.5f, -0.75f, "min");
	check_vec2(&e.maxv, 100.0f, -0.5f, "max");
	check_vec2(&e.scale, 99.5f, 0.25f, "scale");
	assert_int_equal(e.cx, 100);
	assert_int_equal(e.cy, 1);

	box(&items[0], 1280.0f, 0.0f, 0.0f, 720.0f, 0.25f, 0.25f);
	measure(&e, items, 1);

	check_vec2(&e.scale, 1280.0f, 720.0f, "whole scale");
	assert_int_equal(e.cx, 1280);
	assert_int_equal(e.cy, 720);
}

/* The fold starts at (M_INFINITE, M_INFINITE) and (-M_INFINITE,
 * -M_INFINITE). M_INFINITE is 3.4e38, not infinity: it is below FLT_MAX.
 * resize_scene_base sets an empty scene to 0 x 0 itself and never sizes
 * it; sized anyway, the empty fold is -M_INFINITE - M_INFINITE, which
 * overflows to -inf, so 0 x 0 too. */
static void empty_scene(void **state)
{
	struct extent e;

	(void)state;

	measure(&e, NULL, 0);

	check_vec2(&e.minv, M_INFINITE, M_INFINITE, "min");
	check_vec2(&e.maxv, -M_INFINITE, -M_INFINITE, "max");
	check_vec2(&e.scale, -INFINITY, -INFINITY, "scale");
	assert_int_equal(e.cx, 0);
	assert_int_equal(e.cy, 0);
}

/* A NaN anywhere in the x column makes every corner's x NaN (the corners
 * have z = 0 and w = 1, and 0 * NaN is NaN). The < and > compares are false
 * for NaN, so the fold skips it: on its own the item leaves x at the start
 * values (0 wide), and next to another item it changes nothing.
 * Characterized, not endorsed. */
static void nan_is_skipped(void **state)
{
	struct matrix4 items[2];
	struct extent e;

	(void)state;

	box(&items[0], 100.0f, 0.0f, 0.0f, 50.0f, NAN, 20.0f);
	measure(&e, items, 1);

	check_vec2(&e.minv, M_INFINITE, 20.0f, "NaN alone min");
	check_vec2(&e.maxv, -M_INFINITE, 70.0f, "NaN alone max");
	check_vec2(&e.scale, -INFINITY, 50.0f, "NaN alone scale");
	assert_int_equal(e.cx, 0);
	assert_int_equal(e.cy, 50);

	box(&items[1], 30.0f, 0.0f, 0.0f, 10.0f, 5.0f, 0.0f);
	measure(&e, items, 2);

	check_vec2(&e.minv, 5.0f, 0.0f, "NaN first min");
	check_vec2(&e.maxv, 35.0f, 70.0f, "NaN first max");
	assert_int_equal(e.cx, 30);
	assert_int_equal(e.cy, 70);

	box(&items[0], 30.0f, 0.0f, 0.0f, 10.0f, 5.0f, 0.0f);
	box(&items[1], NAN, 0.0f, 0.0f, 50.0f, 0.0f, 20.0f);
	measure(&e, items, 2);

	check_vec2(&e.minv, 5.0f, 0.0f, "NaN second min");
	check_vec2(&e.maxv, 35.0f, 70.0f, "NaN second max");
	assert_int_equal(e.cx, 30);
	assert_int_equal(e.cy, 70);
}

/* Infinities. Characterized, not endorsed:
 * - an item at x = +inf alone: min keeps its start (+inf < M_INFINITE is
 *   false) and max is +inf, so the extent is +inf: UINT32_MAX wide;
 * - next to a finite item, the extent reaches +inf too: UINT32_MAX;
 * - at y = -inf alone: max keeps its start, min is -inf: UINT32_MAX high;
 * - an infinitely wide item at 0: its corners' x are NaN (0 * inf), which
 *   the fold skips, and +inf, so UINT32_MAX wide as well. */
static void infinity(void **state)
{
	struct matrix4 items[2];
	struct extent e;

	(void)state;

	box(&items[0], 100.0f, 0.0f, 0.0f, 50.0f, INFINITY, 20.0f);
	measure(&e, items, 1);

	check_vec2(&e.minv, M_INFINITE, 20.0f, "+inf min");
	check_vec2(&e.maxv, INFINITY, 70.0f, "+inf max");
	check_float(e.scale.x, INFINITY, "+inf scale x");
	assert_int_equal(e.cx, UINT32_MAX);
	assert_int_equal(e.cy, 50);

	box(&items[1], 30.0f, 0.0f, 0.0f, 10.0f, 5.0f, 0.0f);
	measure(&e, items, 2);

	check_vec2(&e.minv, 5.0f, 0.0f, "+inf and finite min");
	check_vec2(&e.maxv, INFINITY, 70.0f, "+inf and finite max");
	check_float(e.scale.x, INFINITY, "+inf and finite scale x");
	assert_int_equal(e.cx, UINT32_MAX);
	assert_int_equal(e.cy, 70);

	box(&items[0], 100.0f, 0.0f, 0.0f, 50.0f, 10.0f, -INFINITY);
	measure(&e, items, 1);

	check_vec2(&e.minv, 10.0f, -INFINITY, "-inf min");
	check_vec2(&e.maxv, 110.0f, -M_INFINITE, "-inf max");
	check_float(e.scale.y, INFINITY, "-inf scale y");
	assert_int_equal(e.cx, 100);
	assert_int_equal(e.cy, UINT32_MAX);

	box(&items[0], INFINITY, 0.0f, 0.0f, 50.0f, 0.0f, 0.0f);
	measure(&e, items, 1);

	check_vec2(&e.minv, M_INFINITE, 0.0f, "infinite width min");
	check_vec2(&e.maxv, INFINITY, 50.0f, "infinite width max");
	check_float(e.scale.x, INFINITY, "infinite width scale x");
	assert_int_equal(e.cx, UINT32_MAX);
	assert_int_equal(e.cy, 50);
}

/* The start values are finite, so an item wholly past them leaves them
 * in place: at x = FLT_MAX the min stays M_INFINITE, and at y = -FLT_MAX
 * the max stays -M_INFINITE. Characterized, not endorsed. */
static void past_the_start_values(void **state)
{
	struct matrix4 items[1];
	struct extent e;

	(void)state;

	box(&items[0], 0.0f, 0.0f, 0.0f, 0.0f, FLT_MAX, -FLT_MAX);
	measure(&e, items, 1);

	check_vec2(&e.minv, M_INFINITE, -FLT_MAX, "min");
	check_vec2(&e.maxv, FLT_MAX, -M_INFINITE, "max");
	assert_int_equal(e.cx, UINT32_MAX);
	assert_int_equal(e.cy, UINT32_MAX);
}

/* Sizes past the uint32_t range saturate (#1840): the largest float below
 * 2^32 stays, 2^32 and more are UINT32_MAX, and so is an extent whose
 * max - min overflows to +inf. */
static void out_of_range_sizes(void **state)
{
	struct matrix4 items[2];
	struct extent e;

	(void)state;

	box(&items[0], 5e9f, 0.0f, 0.0f, 4294967040.0f, 0.0f, 0.0f);
	measure(&e, items, 1);

	check_vec2(&e.scale, 5e9f, 4294967040.0f, "scale");
	assert_int_equal(e.cx, UINT32_MAX);
	assert_int_equal(e.cy, 4294967040u);

	box(&items[0], 4294967296.0f, 0.0f, 0.0f, 1e30f, 0.0f, 0.0f);
	measure(&e, items, 1);

	assert_int_equal(e.cx, UINT32_MAX);
	assert_int_equal(e.cy, UINT32_MAX);

	box(&items[0], 1.0f, 0.0f, 0.0f, 1.0f, -3e38f, -3e38f);
	box(&items[1], 1.0f, 0.0f, 0.0f, 1.0f, 3e38f, 3e38f);
	measure(&e, items, 2);

	check_vec2(&e.scale, INFINITY, INFINITY, "overflowed scale");
	assert_int_equal(e.cx, UINT32_MAX);
	assert_int_equal(e.cy, UINT32_MAX);
}

/* A corner equal to the current min or max does not replace it, so with
 * -0 and +0 the first one stays. Characterized, not endorsed. The first
 * matrix's (0, 0) corner has x = -0 (every product is -0); the second's
 * has x = +0. */
static void tie_keeps_the_first(void **state)
{
	struct matrix4 items[2];
	struct extent e;

	(void)state;

	matrix4_identity(&items[0]);
	vec4_set(&items[0].x, -1.0f, 0.0f, 0.0f, 0.0f);
	vec4_set(&items[0].y, -1.0f, 0.0f, 0.0f, 0.0f);
	vec4_set(&items[0].z, -1.0f, 0.0f, 1.0f, 0.0f);
	vec4_set(&items[0].t, -0.0f, 0.0f, 0.0f, 1.0f);
	box(&items[1], -1.0f, 0.0f, -1.0f, 0.0f, 0.0f, 0.0f);

	measure(&e, items, 2);
	check_float(e.maxv.x, -0.0f, "-0 first max x");
	check_float(e.minv.x, -2.0f, "-0 first min x");

	struct matrix4 swapped[2] = {items[1], items[0]};
	measure(&e, swapped, 2);
	check_float(e.maxv.x, 0.0f, "+0 first max x");
	check_float(e.minv.x, -2.0f, "+0 first min x");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(one_item),
		cmocka_unit_test(several_items),
		cmocka_unit_test(rotated_item_uses_the_corners),
		cmocka_unit_test(fractional_extent_rounds_up),
		cmocka_unit_test(empty_scene),
		cmocka_unit_test(nan_is_skipped),
		cmocka_unit_test(infinity),
		cmocka_unit_test(past_the_start_values),
		cmocka_unit_test(out_of_range_sizes),
		cmocka_unit_test(tie_keeps_the_first),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
