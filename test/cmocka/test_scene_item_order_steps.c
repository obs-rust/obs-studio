#include <limits.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-scene-item-order.h"

/*
 * Characterization of the scene item order, reorder and group membership
 * decisions of libobs/obs-scene.c, moved to libobs/obs-scene-item-order.c
 * before their Rust port:
 * - obs_scene_item_order_anchor(): where obs_sceneitem_set_order() puts the
 *   item back;
 * - obs_scene_item_order_position(): where obs_sceneitem_set_order_position()
 *   puts it;
 * - obs_scene_item_order_item_attached(): the item check of
 *   obs_sceneitem_get_order_position() and obs_sceneitem_group_ungroup2();
 * - obs_scene_item_order_match_item(): one step of the check of
 *   obs_scene_reorder_items();
 * - obs_scene_item_order_reorder_check() and
 *   obs_scene_item_order_in_group_run(): the check and the group runs of
 *   obs_scene_reorder_items2();
 * - obs_scene_item_order_add_item_allowed() and
 *   obs_scene_item_order_remove_item_allowed(): the gates of
 *   obs_sceneitem_group_add_item() and obs_sceneitem_group_remove_item();
 * - obs_scene_item_order_moves_scene() and
 *   obs_scene_item_order_item_active(): the two steps of the gate of
 *   move_active_child().
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them. The items are addresses the functions
 * only compare, never follow.
 *
 * Not pinned here, because the C has no check there and the inputs reach
 * past a list or through NULL in libobs: a movement outside the enum (the
 * item is detached and never attached again), a NULL or detached item for
 * the item check, a reorder2 order that is not the scene's items
 * rearranged, and a remove of an item that is not in the group.
 */

static char storage[16];
#define ITEM(i) ((obs_sceneitem_t *)(void *)&storage[i])
#define A ITEM(1)
#define B ITEM(2)
#define C ITEM(3)
#define D ITEM(4)
#define G ITEM(5)
#define H ITEM(6)

static char scene_storage[8];
#define SCENE(i) ((const obs_scene_t *)(const void *)&scene_storage[i])

#define ARRAY_LEN(a) (sizeof(a) / sizeof((a)[0]))

/* ------------------------------------------------------------------------ */
/* obs_sceneitem_set_order() */

struct anchor_case {
	int movement;
	bool has_prev;
	bool has_next;
	int anchor;
};

static void check_anchors(const struct anchor_case *cases, size_t count)
{
	for (size_t i = 0; i < count; i++) {
		const struct anchor_case *c = &cases[i];
		int got = obs_scene_item_order_anchor(c->movement, c->has_prev, c->has_next);
		if (got != c->anchor) {
			fprintf(stderr, "anchor(%d, %d, %d) = %d, expected %d\n", c->movement, c->has_prev, c->has_next,
				got, c->anchor);
			fail();
		}
	}
}

/* Down: after the item two below (prev->prev, the bottom when that is NULL),
 * or the bottom for the bottom item. */
static void anchor_down(void **state)
{
	(void)state;
	static const struct anchor_case cases[] = {
		{OBS_ORDER_MOVE_DOWN, false, false, OBS_SCENE_ITEM_ORDER_ANCHOR_BOTTOM},
		{OBS_ORDER_MOVE_DOWN, false, true, OBS_SCENE_ITEM_ORDER_ANCHOR_BOTTOM},
		{OBS_ORDER_MOVE_DOWN, true, false, OBS_SCENE_ITEM_ORDER_ANCHOR_PREV_PREV},
		{OBS_ORDER_MOVE_DOWN, true, true, OBS_SCENE_ITEM_ORDER_ANCHOR_PREV_PREV},
	};
	check_anchors(cases, ARRAY_LEN(cases));
}

/* Up: after the item above, or back after prev for the top item (the bottom
 * for a single item, whose prev is NULL). */
static void anchor_up(void **state)
{
	(void)state;
	static const struct anchor_case cases[] = {
		{OBS_ORDER_MOVE_UP, false, false, OBS_SCENE_ITEM_ORDER_ANCHOR_PREV},
		{OBS_ORDER_MOVE_UP, false, true, OBS_SCENE_ITEM_ORDER_ANCHOR_NEXT},
		{OBS_ORDER_MOVE_UP, true, false, OBS_SCENE_ITEM_ORDER_ANCHOR_PREV},
		{OBS_ORDER_MOVE_UP, true, true, OBS_SCENE_ITEM_ORDER_ANCHOR_NEXT},
	};
	check_anchors(cases, ARRAY_LEN(cases));
}

/* Top: after the last item, whatever the neighbours. Bottom: the bottom. */
static void anchor_top_bottom(void **state)
{
	(void)state;
	static const struct anchor_case cases[] = {
		{OBS_ORDER_MOVE_TOP, false, false, OBS_SCENE_ITEM_ORDER_ANCHOR_LAST},
		{OBS_ORDER_MOVE_TOP, false, true, OBS_SCENE_ITEM_ORDER_ANCHOR_LAST},
		{OBS_ORDER_MOVE_TOP, true, false, OBS_SCENE_ITEM_ORDER_ANCHOR_LAST},
		{OBS_ORDER_MOVE_TOP, true, true, OBS_SCENE_ITEM_ORDER_ANCHOR_LAST},
		{OBS_ORDER_MOVE_BOTTOM, false, false, OBS_SCENE_ITEM_ORDER_ANCHOR_BOTTOM},
		{OBS_ORDER_MOVE_BOTTOM, false, true, OBS_SCENE_ITEM_ORDER_ANCHOR_BOTTOM},
		{OBS_ORDER_MOVE_BOTTOM, true, false, OBS_SCENE_ITEM_ORDER_ANCHOR_BOTTOM},
		{OBS_ORDER_MOVE_BOTTOM, true, true, OBS_SCENE_ITEM_ORDER_ANCHOR_BOTTOM},
	};
	check_anchors(cases, ARRAY_LEN(cases));
}

/* A model of obs_sceneitem_set_order() on a list of item numbers from the
 * bottom: the caller's detach and attach around the anchor. */
static size_t model_set_order(int *list, size_t count, size_t index, int movement)
{
	int item = list[index];
	bool has_prev = index > 0;
	bool has_next = index + 1 < count;
	int prev = has_prev ? list[index - 1] : 0;
	int prev_prev = index > 1 ? list[index - 2] : 0;
	int next = has_next ? list[index + 1] : 0;
	int anchor = obs_scene_item_order_anchor(movement, has_prev, has_next);

	for (size_t i = index; i + 1 < count; i++)
		list[i] = list[i + 1];
	count--;

	int after = 0;
	if (anchor == OBS_SCENE_ITEM_ORDER_ANCHOR_PREV_PREV)
		after = prev_prev;
	else if (anchor == OBS_SCENE_ITEM_ORDER_ANCHOR_NEXT)
		after = next;
	else if (anchor == OBS_SCENE_ITEM_ORDER_ANCHOR_PREV)
		after = prev;
	else if (anchor == OBS_SCENE_ITEM_ORDER_ANCHOR_LAST)
		after = count ? list[count - 1] : 0;
	else if (anchor != OBS_SCENE_ITEM_ORDER_ANCHOR_BOTTOM)
		return count;

	size_t at = 0;
	if (after) {
		while (list[at] != after)
			at++;
		at++;
	}
	for (size_t i = count; i > at; i--)
		list[i] = list[i - 1];
	list[at] = item;
	return count + 1;
}

struct order_case {
	size_t index;
	int movement;
	int after[4];
};

/* Each movement at the bottom, in the middle and at the top of 1 2 3 4. */
static void set_order_moves(void **state)
{
	(void)state;
	static const struct order_case cases[] = {
		{0, OBS_ORDER_MOVE_UP, {2, 1, 3, 4}},     {1, OBS_ORDER_MOVE_UP, {1, 3, 2, 4}},
		{3, OBS_ORDER_MOVE_UP, {1, 2, 3, 4}},     {0, OBS_ORDER_MOVE_DOWN, {1, 2, 3, 4}},
		{1, OBS_ORDER_MOVE_DOWN, {2, 1, 3, 4}},   {2, OBS_ORDER_MOVE_DOWN, {1, 3, 2, 4}},
		{3, OBS_ORDER_MOVE_DOWN, {1, 2, 4, 3}},   {0, OBS_ORDER_MOVE_TOP, {2, 3, 4, 1}},
		{1, OBS_ORDER_MOVE_TOP, {1, 3, 4, 2}},    {3, OBS_ORDER_MOVE_TOP, {1, 2, 3, 4}},
		{0, OBS_ORDER_MOVE_BOTTOM, {1, 2, 3, 4}}, {2, OBS_ORDER_MOVE_BOTTOM, {3, 1, 2, 4}},
		{3, OBS_ORDER_MOVE_BOTTOM, {4, 1, 2, 3}},
	};

	for (size_t i = 0; i < ARRAY_LEN(cases); i++) {
		int list[5] = {1, 2, 3, 4};
		size_t count = model_set_order(list, 4, cases[i].index, cases[i].movement);
		assert_int_equal(count, 4);
		assert_memory_equal(list, cases[i].after, sizeof(cases[i].after));
	}
}

/* A single item stays where it is for every movement. */
static void set_order_single(void **state)
{
	(void)state;
	static const int movements[] = {OBS_ORDER_MOVE_UP, OBS_ORDER_MOVE_DOWN, OBS_ORDER_MOVE_TOP,
					OBS_ORDER_MOVE_BOTTOM};

	for (size_t i = 0; i < ARRAY_LEN(movements); i++) {
		int list[2] = {7};
		assert_int_equal(model_set_order(list, 1, 0, movements[i]), 1);
		assert_int_equal(list[0], 7);
	}
}

/* ------------------------------------------------------------------------ */
/* obs_sceneitem_set_order_position() */

/* Position 0 or an empty scene: the bottom, and the steps are not written. */
static void position_bottom(void **state)
{
	(void)state;
	static const int positions[] = {0, 1, 2, 5, -1, INT_MIN, INT_MAX};
	size_t steps = 12345;

	assert_true(obs_scene_item_order_position(0, true, &steps));
	assert_int_equal(steps, 12345);

	for (size_t i = 0; i < ARRAY_LEN(positions); i++) {
		assert_true(obs_scene_item_order_position(positions[i], false, &steps));
		assert_int_equal(steps, 12345);
	}
}

/* Otherwise position - 1 steps from the first item; the caller stops at the
 * last item. A negative position takes no step, so the item goes after the
 * first item, as for position 1: characterized, not endorsed. */
static void position_steps(void **state)
{
	(void)state;
	static const struct {
		int position;
		size_t steps;
	} cases[] = {
		{1, 0},  {2, 1},  {3, 2},       {6, 5}, {11, 10}, {INT_MAX, (size_t)INT_MAX - 1},
		{-1, 0}, {-2, 0}, {INT_MIN, 0},
	};

	for (size_t i = 0; i < ARRAY_LEN(cases); i++) {
		size_t steps = 12345;
		assert_false(obs_scene_item_order_position(cases[i].position, true, &steps));
		assert_int_equal(steps, cases[i].steps);
	}
}

/* A model of the caller on 1 2 3 4 5 with item 3 moved: the list after the
 * detach is 1 2 4 5, and the walk stops at the last item. */
static void position_walk(void **state)
{
	(void)state;
	static const struct {
		int position;
		int after[5];
	} cases[] = {
		{-1, {1, 3, 2, 4, 5}}, {0, {3, 1, 2, 4, 5}}, {1, {1, 3, 2, 4, 5}}, {2, {1, 2, 3, 4, 5}},
		{3, {1, 2, 4, 3, 5}},  {4, {1, 2, 4, 5, 3}}, {5, {1, 2, 4, 5, 3}}, {9, {1, 2, 4, 5, 3}},
	};

	for (size_t i = 0; i < ARRAY_LEN(cases); i++) {
		int list[5] = {1, 2, 4, 5};
		size_t count = 4;
		size_t steps = 0;
		size_t at = 0;

		if (!obs_scene_item_order_position(cases[i].position, count > 0, &steps)) {
			size_t walk = 0;
			for (size_t s = steps; s > 0; --s) {
				if (walk + 1 >= count)
					break;
				walk++;
			}
			at = walk + 1;
		}
		for (size_t k = count; k > at; k--)
			list[k] = list[k - 1];
		list[at] = 3;
		assert_memory_equal(list, cases[i].after, sizeof(cases[i].after));
	}
}

/* ------------------------------------------------------------------------ */
/* obs_sceneitem_get_order_position() and obs_sceneitem_group_ungroup2() */

static void item_attached(void **state)
{
	(void)state;
	assert_true(obs_scene_item_order_item_attached(true, true));
}

/* ------------------------------------------------------------------------ */
/* obs_scene_reorder_items() */

static void match_item_steps(void **state)
{
	(void)state;
	obs_sceneitem_t *const order[] = {A, B, C};
	bool matches = true;

	assert_true(obs_scene_item_order_match_item(order, 3, A, 0, &matches));
	assert_true(matches);
	assert_true(obs_scene_item_order_match_item(order, 3, C, 2, &matches));
	assert_true(matches);

	/* Found elsewhere: the order differs. */
	assert_true(obs_scene_item_order_match_item(order, 3, B, 0, &matches));
	assert_false(matches);

	/* Once cleared, it stays cleared. */
	assert_true(obs_scene_item_order_match_item(order, 3, A, 0, &matches));
	assert_false(matches);

	/* Not in the order: not found, the flag untouched. */
	matches = true;
	assert_false(obs_scene_item_order_match_item(order, 3, D, 0, &matches));
	assert_true(matches);
	assert_false(obs_scene_item_order_match_item(order, 0, A, 0, &matches));
	assert_true(matches);
	assert_false(obs_scene_item_order_match_item(NULL, 0, A, 0, &matches));
	assert_true(matches);

	/* A repeated item counts at its first entry. */
	obs_sceneitem_t *const repeated[] = {A, A, B};
	assert_true(obs_scene_item_order_match_item(repeated, 3, A, 1, &matches));
	assert_false(matches);
	matches = true;
	assert_true(obs_scene_item_order_match_item(repeated, 3, A, 0, &matches));
	assert_true(matches);
}

/* A model of sceneitems_match() and the refusal of obs_scene_reorder_items():
 * true when the order is applied. */
static bool model_reorder(obs_sceneitem_t *const *scene, size_t scene_count, obs_sceneitem_t *const *order, size_t size)
{
	bool matches = true;
	size_t count = 0;

	for (; count < scene_count; count++) {
		if (!obs_scene_item_order_match_item(order, size, scene[count], count, &matches))
			return false;
	}

	return count == size && !matches;
}

static void reorder_flat(void **state)
{
	(void)state;
	obs_sceneitem_t *const scene[] = {A, B, C};

	obs_sceneitem_t *const same[] = {A, B, C};
	assert_false(model_reorder(scene, 3, same, 3));

	obs_sceneitem_t *const permutation[] = {C, A, B};
	assert_true(model_reorder(scene, 3, permutation, 3));

	obs_sceneitem_t *const missing[] = {B, A};
	assert_false(model_reorder(scene, 3, missing, 2));

	obs_sceneitem_t *const extra[] = {B, A, C, D};
	assert_false(model_reorder(scene, 3, extra, 4));

	obs_sceneitem_t *const repeated[] = {B, A, A};
	assert_false(model_reorder(scene, 3, repeated, 3));

	/* A repeat that still holds every item is longer than the scene. */
	obs_sceneitem_t *const repeated_all[] = {B, A, C, A};
	assert_false(model_reorder(scene, 3, repeated_all, 4));
}

/* ------------------------------------------------------------------------ */
/* obs_scene_reorder_items2() */

static void reorder2_check(void **state)
{
	(void)state;
	/* A, then group G holding B and C, then D. */
	static const struct obs_sceneitem_order_info current[] = {
		{NULL, A}, {NULL, G}, {G, B}, {G, C}, {NULL, D},
	};

	assert_int_equal(obs_scene_item_order_reorder_check(current, 5, current, 5), OBS_SCENE_ITEM_ORDER_SAME);
	assert_int_equal(obs_scene_item_order_reorder_check(NULL, 0, NULL, 0), OBS_SCENE_ITEM_ORDER_SAME);

	/* The top level rearranged. */
	static const struct obs_sceneitem_order_info top[] = {
		{NULL, D}, {NULL, G}, {G, B}, {G, C}, {NULL, A},
	};
	assert_int_equal(obs_scene_item_order_reorder_check(current, 5, top, 5), OBS_SCENE_ITEM_ORDER_APPLY);

	/* Inside the group. */
	static const struct obs_sceneitem_order_info inside[] = {
		{NULL, A}, {NULL, G}, {G, C}, {G, B}, {NULL, D},
	};
	assert_int_equal(obs_scene_item_order_reorder_check(current, 5, inside, 5), OBS_SCENE_ITEM_ORDER_APPLY);

	/* D moved into the group, B out of it: only the group field changes. */
	static const struct obs_sceneitem_order_info moved[] = {
		{NULL, A}, {NULL, G}, {NULL, B}, {G, C}, {G, D},
	};
	assert_int_equal(obs_scene_item_order_reorder_check(current, 5, moved, 5), OBS_SCENE_ITEM_ORDER_APPLY);
}

/* The caller's run: entries after head, one step at a time, up to the first
 * that is not in the run. */
static size_t run_end(const struct obs_sceneitem_order_info *order, size_t size, size_t head)
{
	size_t i = head + 1;

	while (obs_scene_item_order_in_group_run(order, size, head, i))
		i++;

	return i;
}

static void group_runs(void **state)
{
	(void)state;
	static const struct obs_sceneitem_order_info order[] = {
		{NULL, A}, {NULL, G}, {G, B}, {G, C}, {NULL, D}, {NULL, H}, {G, A}, {H, B},
	};

	/* G holds B and C, up to D at the top level. */
	assert_int_equal(run_end(order, 8, 1), 4);
	/* H's run stops at the entry for G: that sub-item does not follow its
	 * group, so it goes to the top level (characterized, not endorsed), and
	 * so does everything after it. */
	assert_int_equal(run_end(order, 8, 5), 6);
	/* A run that ends the order. */
	assert_int_equal(run_end(order, 4, 1), 4);
	/* An empty group, and a group last. */
	static const struct obs_sceneitem_order_info empty[] = {
		{NULL, G},
		{NULL, A},
		{NULL, H},
	};
	assert_int_equal(run_end(empty, 3, 0), 1);
	assert_int_equal(run_end(empty, 3, 2), 3);

	/* Each step on its own: past the order, the head itself, another
	 * group's sub-item, a top-level entry. */
	assert_false(obs_scene_item_order_in_group_run(order, 8, 1, 8));
	assert_false(obs_scene_item_order_in_group_run(order, 4, 1, 4));
	assert_false(obs_scene_item_order_in_group_run(order, 8, 1, 1));
	assert_true(obs_scene_item_order_in_group_run(order, 8, 1, 6));
	assert_false(obs_scene_item_order_in_group_run(order, 8, 1, 7));
	assert_false(obs_scene_item_order_in_group_run(order, 8, 1, 0));
}

/* ------------------------------------------------------------------------ */
/* obs_sceneitem_group_add_item() and obs_sceneitem_group_remove_item() */

static void add_item_gate(void **state)
{
	(void)state;
	const obs_scene_t *scene = SCENE(1);
	const obs_scene_t *group_scene = SCENE(2);
	const obs_scene_t *other = SCENE(3);

	assert_true(obs_scene_item_order_add_item_allowed(scene, scene, group_scene));
	assert_false(obs_scene_item_order_add_item_allowed(group_scene, scene, group_scene));
	assert_false(obs_scene_item_order_add_item_allowed(other, scene, group_scene));
	assert_false(obs_scene_item_order_add_item_allowed(NULL, scene, group_scene));
	/* An item has one parent, so it is in the group's parent scene and in
	 * the group's own scene at once only when those are the same scene. The
	 * second check never decides in libobs: characterized, not endorsed. */
	assert_false(obs_scene_item_order_add_item_allowed(scene, scene, scene));
}

/* ------------------------------------------------------------------------ */
/* move_active_child() */

static void active_child_gates(void **state)
{
	(void)state;
	assert_true(obs_scene_item_order_moves_scene(SCENE(1), SCENE(2)));
	assert_true(obs_scene_item_order_moves_scene(SCENE(1), NULL));
	assert_true(obs_scene_item_order_moves_scene(NULL, SCENE(2)));
	assert_false(obs_scene_item_order_moves_scene(SCENE(1), SCENE(1)));
	assert_false(obs_scene_item_order_moves_scene(NULL, NULL));

	assert_true(obs_scene_item_order_item_active(1));
	assert_true(obs_scene_item_order_item_active(LONG_MAX));
	/* A count gone negative counts as inactive, as 0 does. */
	assert_false(obs_scene_item_order_item_active(0));
	assert_false(obs_scene_item_order_item_active(-1));
	assert_false(obs_scene_item_order_item_active(LONG_MIN));
}

static void remove_item_gate(void **state)
{
	(void)state;
	assert_true(obs_scene_item_order_remove_item_allowed(true));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(anchor_down),       cmocka_unit_test(anchor_up),
		cmocka_unit_test(anchor_top_bottom), cmocka_unit_test(set_order_moves),
		cmocka_unit_test(set_order_single),  cmocka_unit_test(position_bottom),
		cmocka_unit_test(position_steps),    cmocka_unit_test(position_walk),
		cmocka_unit_test(item_attached),     cmocka_unit_test(match_item_steps),
		cmocka_unit_test(reorder_flat),      cmocka_unit_test(reorder2_check),
		cmocka_unit_test(group_runs),        cmocka_unit_test(add_item_gate),
		cmocka_unit_test(remove_item_gate),  cmocka_unit_test(active_child_gates),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
