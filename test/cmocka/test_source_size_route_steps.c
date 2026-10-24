#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <limits.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-source-size.h"

/*
 * Characterization of the size query routing of obs_source_get_width() and
 * obs_source_get_height() in libobs/obs-source.c, moved to
 * libobs/obs-source-size.c before its Rust port:
 * - obs_source_size_route(): the decision of get_base_width() and
 *   get_base_height(). A transition reports its actual size while enabled,
 *   else 0; a source with data and a size callback asks it (a filter only
 *   while enabled); a filter without one asks its target; an active async
 *   source reports its frame's size, the axes swapped unless the rotation
 *   is a multiple of 180 (get_async_width() and get_async_height()); anything
 *   else reports 0. A filter is a source with a filter parent.
 * - obs_source_size_entry(): where obs_source_get_width() starts. A filter
 *   starts at itself; any other source at its first filter if it has
 *   filters (get_recurse_width()), else at itself.
 * Cases marked "characterized, not endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them. test_slideshow_size, test_filter_color_space,
 * test_scene_item_crop, test_scene_rotated_scale_filter and
 * test_async_rotation_normalize cover the caller (data_valid(), filter_mutex,
 * the recursion and the callbacks) through the real libobs.
 */

/* enum obs_source_type (obs-source.h, which needs the whole of obs.h);
 * obs-source.c checks the header's against it */
#define TYPE_INPUT 0
#define TYPE_FILTER OBS_SOURCE_SIZE_TYPE_FILTER
#define TYPE_TRANSITION OBS_SOURCE_SIZE_TYPE_TRANSITION
#define TYPE_SCENE 3

#define TRANSITION OBS_SOURCE_SIZE_TRANSITION
#define CALLBACK OBS_SOURCE_SIZE_CALLBACK
#define TARGET OBS_SOURCE_SIZE_ASK_TARGET
#define ASYNC OBS_SOURCE_SIZE_ASYNC
#define SWAPPED OBS_SOURCE_SIZE_ASYNC_SWAPPED
#define ZERO OBS_SOURCE_SIZE_ZERO

#define SELF OBS_SOURCE_SIZE_ENTRY_SELF
#define FIRST_FILTER OBS_SOURCE_SIZE_ENTRY_FIRST_FILTER

static const char *route_name(enum obs_source_size_ask route)
{
	switch (route) {
	case TRANSITION:
		return "transition";
	case CALLBACK:
		return "callback";
	case TARGET:
		return "ask target";
	case ASYNC:
		return "async";
	case SWAPPED:
		return "async swapped";
	case ZERO:
		return "zero";
	}
	return "(out of range)";
}

static const char *entry_name(enum obs_source_size_start entry)
{
	switch (entry) {
	case SELF:
		return "self";
	case FIRST_FILTER:
		return "first filter";
	}
	return "(out of range)";
}

static struct obs_source_size_node node_of(int type, bool data, bool callback, bool parent, bool enabled,
					   bool async_active, long rotation)
{
	struct obs_source_size_node node = {
		.type = type,
		.has_data = data,
		.has_callback = callback,
		.has_filter_parent = parent,
		.enabled = enabled,
		.async_active = async_active,
		.async_rotation = rotation,
	};
	return node;
}

static void check_route_line(struct obs_source_size_node node, enum obs_source_size_ask expected, int line)
{
	const enum obs_source_size_ask actual = obs_source_size_route(node);
	if (actual != expected) {
		fprintf(stderr,
			"line %d: type %d, data %d, callback %d, parent %d, enabled %d, async %d, rotation %ld: "
			"%s (%d), expected %s\n",
			line, node.type, node.has_data, node.has_callback, node.has_filter_parent, node.enabled,
			node.async_active, node.async_rotation, route_name(actual), (int)actual, route_name(expected));
		fail();
	}
}

#define check_route(node, expected) check_route_line(node, expected, __LINE__)

static void check_entry_line(int type, bool has_filters, enum obs_source_size_start expected, int line)
{
	const enum obs_source_size_start actual = obs_source_size_entry(type, has_filters);
	if (actual != expected) {
		fprintf(stderr, "line %d: type %d, filters %d: %s (%d), expected %s\n", line, type, has_filters,
			entry_name(actual), (int)actual, entry_name(expected));
		fail();
	}
}

#define check_entry(type, has_filters, expected) check_entry_line(type, has_filters, expected, __LINE__)

/* An active async input with no callback: the route is the swap alone. */
static void check_swap_line(long rotation, bool swapped, int line)
{
	check_route_line(node_of(TYPE_INPUT, true, false, false, true, true, rotation), swapped ? SWAPPED : ASYNC,
			 line);
}

#define check_swap(rotation, swapped) check_swap_line(rotation, swapped, __LINE__)

/* Every combination of the inputs: the type (an input, a filter, a
 * transition, a scene), data, a size callback, a filter parent, enabled
 * and async active, at rotation 0. */
struct route_row {
	int type;
	bool data, callback, parent, enabled, async_active;
	enum obs_source_size_ask route;
};

/* type, data, callback, parent, enabled, async, route */
static const struct route_row route_rows[128] = {
	{TYPE_INPUT, 0, 0, 0, 0, 0, ZERO},
	{TYPE_INPUT, 0, 0, 0, 0, 1, ASYNC},
	{TYPE_INPUT, 0, 0, 0, 1, 0, ZERO},
	{TYPE_INPUT, 0, 0, 0, 1, 1, ASYNC},
	{TYPE_INPUT, 0, 0, 1, 0, 0, TARGET},
	{TYPE_INPUT, 0, 0, 1, 0, 1, TARGET},
	{TYPE_INPUT, 0, 0, 1, 1, 0, TARGET},
	{TYPE_INPUT, 0, 0, 1, 1, 1, TARGET},
	{TYPE_INPUT, 0, 1, 0, 0, 0, ZERO},
	{TYPE_INPUT, 0, 1, 0, 0, 1, ASYNC},
	{TYPE_INPUT, 0, 1, 0, 1, 0, ZERO},
	{TYPE_INPUT, 0, 1, 0, 1, 1, ASYNC},
	{TYPE_INPUT, 0, 1, 1, 0, 0, TARGET},
	{TYPE_INPUT, 0, 1, 1, 0, 1, TARGET},
	{TYPE_INPUT, 0, 1, 1, 1, 0, TARGET},
	{TYPE_INPUT, 0, 1, 1, 1, 1, TARGET},
	{TYPE_INPUT, 1, 0, 0, 0, 0, ZERO},
	{TYPE_INPUT, 1, 0, 0, 0, 1, ASYNC},
	{TYPE_INPUT, 1, 0, 0, 1, 0, ZERO},
	{TYPE_INPUT, 1, 0, 0, 1, 1, ASYNC},
	{TYPE_INPUT, 1, 0, 1, 0, 0, TARGET},
	{TYPE_INPUT, 1, 0, 1, 0, 1, TARGET},
	{TYPE_INPUT, 1, 0, 1, 1, 0, TARGET},
	{TYPE_INPUT, 1, 0, 1, 1, 1, TARGET},
	{TYPE_INPUT, 1, 1, 0, 0, 0, CALLBACK},
	{TYPE_INPUT, 1, 1, 0, 0, 1, CALLBACK},
	{TYPE_INPUT, 1, 1, 0, 1, 0, CALLBACK},
	{TYPE_INPUT, 1, 1, 0, 1, 1, CALLBACK},
	{TYPE_INPUT, 1, 1, 1, 0, 0, TARGET},
	{TYPE_INPUT, 1, 1, 1, 0, 1, TARGET},
	{TYPE_INPUT, 1, 1, 1, 1, 0, CALLBACK},
	{TYPE_INPUT, 1, 1, 1, 1, 1, CALLBACK},
	{TYPE_FILTER, 0, 0, 0, 0, 0, ZERO},
	{TYPE_FILTER, 0, 0, 0, 0, 1, ASYNC},
	{TYPE_FILTER, 0, 0, 0, 1, 0, ZERO},
	{TYPE_FILTER, 0, 0, 0, 1, 1, ASYNC},
	{TYPE_FILTER, 0, 0, 1, 0, 0, TARGET},
	{TYPE_FILTER, 0, 0, 1, 0, 1, TARGET},
	{TYPE_FILTER, 0, 0, 1, 1, 0, TARGET},
	{TYPE_FILTER, 0, 0, 1, 1, 1, TARGET},
	{TYPE_FILTER, 0, 1, 0, 0, 0, ZERO},
	{TYPE_FILTER, 0, 1, 0, 0, 1, ASYNC},
	{TYPE_FILTER, 0, 1, 0, 1, 0, ZERO},
	{TYPE_FILTER, 0, 1, 0, 1, 1, ASYNC},
	{TYPE_FILTER, 0, 1, 1, 0, 0, TARGET},
	{TYPE_FILTER, 0, 1, 1, 0, 1, TARGET},
	{TYPE_FILTER, 0, 1, 1, 1, 0, TARGET},
	{TYPE_FILTER, 0, 1, 1, 1, 1, TARGET},
	{TYPE_FILTER, 1, 0, 0, 0, 0, ZERO},
	{TYPE_FILTER, 1, 0, 0, 0, 1, ASYNC},
	{TYPE_FILTER, 1, 0, 0, 1, 0, ZERO},
	{TYPE_FILTER, 1, 0, 0, 1, 1, ASYNC},
	{TYPE_FILTER, 1, 0, 1, 0, 0, TARGET},
	{TYPE_FILTER, 1, 0, 1, 0, 1, TARGET},
	{TYPE_FILTER, 1, 0, 1, 1, 0, TARGET},
	{TYPE_FILTER, 1, 0, 1, 1, 1, TARGET},
	{TYPE_FILTER, 1, 1, 0, 0, 0, CALLBACK},
	{TYPE_FILTER, 1, 1, 0, 0, 1, CALLBACK},
	{TYPE_FILTER, 1, 1, 0, 1, 0, CALLBACK},
	{TYPE_FILTER, 1, 1, 0, 1, 1, CALLBACK},
	{TYPE_FILTER, 1, 1, 1, 0, 0, TARGET},
	{TYPE_FILTER, 1, 1, 1, 0, 1, TARGET},
	{TYPE_FILTER, 1, 1, 1, 1, 0, CALLBACK},
	{TYPE_FILTER, 1, 1, 1, 1, 1, CALLBACK},
	{TYPE_TRANSITION, 0, 0, 0, 0, 0, ZERO},
	{TYPE_TRANSITION, 0, 0, 0, 0, 1, ZERO},
	{TYPE_TRANSITION, 0, 0, 0, 1, 0, TRANSITION},
	{TYPE_TRANSITION, 0, 0, 0, 1, 1, TRANSITION},
	{TYPE_TRANSITION, 0, 0, 1, 0, 0, ZERO},
	{TYPE_TRANSITION, 0, 0, 1, 0, 1, ZERO},
	{TYPE_TRANSITION, 0, 0, 1, 1, 0, TRANSITION},
	{TYPE_TRANSITION, 0, 0, 1, 1, 1, TRANSITION},
	{TYPE_TRANSITION, 0, 1, 0, 0, 0, ZERO},
	{TYPE_TRANSITION, 0, 1, 0, 0, 1, ZERO},
	{TYPE_TRANSITION, 0, 1, 0, 1, 0, TRANSITION},
	{TYPE_TRANSITION, 0, 1, 0, 1, 1, TRANSITION},
	{TYPE_TRANSITION, 0, 1, 1, 0, 0, ZERO},
	{TYPE_TRANSITION, 0, 1, 1, 0, 1, ZERO},
	{TYPE_TRANSITION, 0, 1, 1, 1, 0, TRANSITION},
	{TYPE_TRANSITION, 0, 1, 1, 1, 1, TRANSITION},
	{TYPE_TRANSITION, 1, 0, 0, 0, 0, ZERO},
	{TYPE_TRANSITION, 1, 0, 0, 0, 1, ZERO},
	{TYPE_TRANSITION, 1, 0, 0, 1, 0, TRANSITION},
	{TYPE_TRANSITION, 1, 0, 0, 1, 1, TRANSITION},
	{TYPE_TRANSITION, 1, 0, 1, 0, 0, ZERO},
	{TYPE_TRANSITION, 1, 0, 1, 0, 1, ZERO},
	{TYPE_TRANSITION, 1, 0, 1, 1, 0, TRANSITION},
	{TYPE_TRANSITION, 1, 0, 1, 1, 1, TRANSITION},
	{TYPE_TRANSITION, 1, 1, 0, 0, 0, ZERO},
	{TYPE_TRANSITION, 1, 1, 0, 0, 1, ZERO},
	{TYPE_TRANSITION, 1, 1, 0, 1, 0, TRANSITION},
	{TYPE_TRANSITION, 1, 1, 0, 1, 1, TRANSITION},
	{TYPE_TRANSITION, 1, 1, 1, 0, 0, ZERO},
	{TYPE_TRANSITION, 1, 1, 1, 0, 1, ZERO},
	{TYPE_TRANSITION, 1, 1, 1, 1, 0, TRANSITION},
	{TYPE_TRANSITION, 1, 1, 1, 1, 1, TRANSITION},
	{TYPE_SCENE, 0, 0, 0, 0, 0, ZERO},
	{TYPE_SCENE, 0, 0, 0, 0, 1, ASYNC},
	{TYPE_SCENE, 0, 0, 0, 1, 0, ZERO},
	{TYPE_SCENE, 0, 0, 0, 1, 1, ASYNC},
	{TYPE_SCENE, 0, 0, 1, 0, 0, TARGET},
	{TYPE_SCENE, 0, 0, 1, 0, 1, TARGET},
	{TYPE_SCENE, 0, 0, 1, 1, 0, TARGET},
	{TYPE_SCENE, 0, 0, 1, 1, 1, TARGET},
	{TYPE_SCENE, 0, 1, 0, 0, 0, ZERO},
	{TYPE_SCENE, 0, 1, 0, 0, 1, ASYNC},
	{TYPE_SCENE, 0, 1, 0, 1, 0, ZERO},
	{TYPE_SCENE, 0, 1, 0, 1, 1, ASYNC},
	{TYPE_SCENE, 0, 1, 1, 0, 0, TARGET},
	{TYPE_SCENE, 0, 1, 1, 0, 1, TARGET},
	{TYPE_SCENE, 0, 1, 1, 1, 0, TARGET},
	{TYPE_SCENE, 0, 1, 1, 1, 1, TARGET},
	{TYPE_SCENE, 1, 0, 0, 0, 0, ZERO},
	{TYPE_SCENE, 1, 0, 0, 0, 1, ASYNC},
	{TYPE_SCENE, 1, 0, 0, 1, 0, ZERO},
	{TYPE_SCENE, 1, 0, 0, 1, 1, ASYNC},
	{TYPE_SCENE, 1, 0, 1, 0, 0, TARGET},
	{TYPE_SCENE, 1, 0, 1, 0, 1, TARGET},
	{TYPE_SCENE, 1, 0, 1, 1, 0, TARGET},
	{TYPE_SCENE, 1, 0, 1, 1, 1, TARGET},
	{TYPE_SCENE, 1, 1, 0, 0, 0, CALLBACK},
	{TYPE_SCENE, 1, 1, 0, 0, 1, CALLBACK},
	{TYPE_SCENE, 1, 1, 0, 1, 0, CALLBACK},
	{TYPE_SCENE, 1, 1, 0, 1, 1, CALLBACK},
	{TYPE_SCENE, 1, 1, 1, 0, 0, TARGET},
	{TYPE_SCENE, 1, 1, 1, 0, 1, TARGET},
	{TYPE_SCENE, 1, 1, 1, 1, 0, CALLBACK},
	{TYPE_SCENE, 1, 1, 1, 1, 1, CALLBACK},
};

static const int types[4] = {TYPE_INPUT, TYPE_FILTER, TYPE_TRANSITION, TYPE_SCENE};

static void route_every_combination(void **state)
{
	(void)state;

	for (size_t i = 0; i < sizeof(route_rows) / sizeof(route_rows[0]); ++i) {
		const struct route_row *row = &route_rows[i];
		/* the rows count up: the type, then a binary number with data first */
		const size_t bits = (size_t)row->data << 4 | (size_t)row->callback << 3 | (size_t)row->parent << 2 |
				    (size_t)row->enabled << 1 | (size_t)row->async_active;
		if (row->type != types[i / 32] || bits != i % 32) {
			fprintf(stderr, "row %zu is out of order\n", i);
			fail();
		}
		check_route_line(node_of(row->type, row->data, row->callback, row->parent, row->enabled,
					 row->async_active, 0),
				 row->route, (int)i);
	}
}

/* The same rows at a rotation of 90: only the async route changes, to the
 * swapped one. */
static void route_every_combination_rotated(void **state)
{
	(void)state;

	for (size_t i = 0; i < sizeof(route_rows) / sizeof(route_rows[0]); ++i) {
		const struct route_row *row = &route_rows[i];
		check_route_line(node_of(row->type, row->data, row->callback, row->parent, row->enabled,
					 row->async_active, 90),
				 row->route == ASYNC ? SWAPPED : row->route, (int)i);
	}
}

/* Only OBS_SOURCE_TYPE_TRANSITION has a route of its own: the filter type,
 * scenes and values outside the enum route as inputs (a filter is a source
 * with a filter parent since 8a1486d). */
static void route_other_types_as_inputs(void **state)
{
	(void)state;
	const int others[] = {TYPE_FILTER, TYPE_SCENE, -1, 4, INT_MIN, INT_MAX};

	for (size_t t = 0; t < sizeof(others) / sizeof(others[0]); ++t) {
		for (size_t i = 0; i < 32; ++i) {
			const struct route_row *row = &route_rows[i];
			check_route_line(node_of(others[t], row->data, row->callback, row->parent, row->enabled,
						 row->async_active, 0),
					 row->route, (int)i);
		}
	}
}

/* A transition reports transition_actual_cx/cy while enabled and 0 while
 * not, whatever else it has: its callback, its data, a parent, async. */
static void route_transition(void **state)
{
	(void)state;

	for (unsigned bits = 0; bits < 16; ++bits) {
		const bool data = bits & 1, callback = bits & 2, parent = bits & 4, async_active = bits & 8;
		check_route(node_of(TYPE_TRANSITION, data, callback, parent, true, async_active, 90), TRANSITION);
		check_route(node_of(TYPE_TRANSITION, data, callback, parent, false, async_active, 90), ZERO);
	}
}

/* The chains of test_filter_color_space and test_scene_rotated_scale_filter:
 * filters with and without a size callback above an input. */
static void route_filter_chains(void **state)
{
	(void)state;

	/* a filter with a size callback (Scaling/Aspect Ratio, Crop/Pad) asks
	 * it while enabled */
	check_route(node_of(TYPE_FILTER, true, true, true, true, false, 0), CALLBACK);
	/* disabled, it asks its target: a disabled filter does not resize */
	check_route(node_of(TYPE_FILTER, true, true, true, false, false, 0), TARGET);
	/* a filter without one (Color Correction) asks its target */
	check_route(node_of(TYPE_FILTER, true, false, true, true, false, 0), TARGET);
	/* a filter whose data is gone asks its target (f1aa71b) */
	check_route(node_of(TYPE_FILTER, false, true, true, true, false, 0), TARGET);
	/* a filter's own async frames do not count: the target first */
	check_route(node_of(TYPE_FILTER, true, false, true, true, true, 90), TARGET);
	/* the input at the bottom asks its callback, enabled or not */
	check_route(node_of(TYPE_INPUT, true, true, false, true, false, 0), CALLBACK);
	check_route(node_of(TYPE_INPUT, true, true, false, false, false, 0), CALLBACK);
	/* an async input (a camera) reports its frame, rotated or not */
	check_route(node_of(TYPE_INPUT, true, false, false, true, true, 0), ASYNC);
	check_route(node_of(TYPE_INPUT, true, false, false, true, true, 90), SWAPPED);
	/* before its first frame it reports 0 (506e30d) */
	check_route(node_of(TYPE_INPUT, true, false, false, true, false, 90), ZERO);
	/* an async input with a size callback asks the callback */
	check_route(node_of(TYPE_INPUT, true, true, false, true, true, 90), CALLBACK);
	/* a scene asks its callback */
	check_route(node_of(TYPE_SCENE, true, true, false, true, false, 0), CALLBACK);
}

/* Characterized, not endorsed: a filter is a source with a filter parent,
 * not one of OBS_SOURCE_TYPE_FILTER. A filter that is not in a chain (no
 * parent: created, or removed from its source) asks its callback even while
 * disabled, and without one reports its async frame or 0 instead of asking
 * a target. */
static void route_detached_filter(void **state)
{
	(void)state;

	check_route(node_of(TYPE_FILTER, true, true, false, false, false, 0), CALLBACK);
	check_route(node_of(TYPE_FILTER, true, false, false, true, false, 0), ZERO);
	check_route(node_of(TYPE_FILTER, true, false, false, true, true, 90), SWAPPED);
	/* and an input with a filter parent routes as a filter */
	check_route(node_of(TYPE_INPUT, true, true, true, false, false, 0), TARGET);
}

/* Characterized, not endorsed: a source with a callback but no data does
 * not ask it, and with nowhere else to go reports its async frame or 0.
 * obs_source_get_width() checks data_valid() first, so libobs reaches this
 * only through a filter target or a first filter whose data is gone. */
static void route_no_data(void **state)
{
	(void)state;

	check_route(node_of(TYPE_INPUT, false, true, false, true, false, 0), ZERO);
	check_route(node_of(TYPE_INPUT, false, true, false, true, true, 0), ASYNC);
	check_route(node_of(TYPE_SCENE, false, true, false, true, false, 0), ZERO);
}

/* The swap: the axes swap unless the rotation is a multiple of 180, with
 * C's truncating %. obs_source_set_async_rotation() keeps the stored
 * rotation in 0-359 since #2136, but the route takes any long, as it gets
 * whatever the field holds. */
static void swap_rotations(void **state)
{
	(void)state;

	check_swap(0, false);
	check_swap(90, true);
	check_swap(-90, true);
	check_swap(180, false);
	check_swap(-180, false);
	check_swap(270, true);
	check_swap(-270, true);
	check_swap(45, true);
	check_swap(360, false);
	/* the rest of the normalized range */
	check_swap(1, true);
	check_swap(179, true);
	check_swap(181, true);
	check_swap(359, true);
	/* raw values the setter no longer stores */
	check_swap(-360, false);
	check_swap(540, false);
	check_swap(-1, true);
}

/* LONG_MIN % 180 and LONG_MAX % 180 are not 0 for a 32-bit long (-128,
 * 127) or a 64-bit one (-8, 7): both swap. The multiples of 180 next to them
 * do not. */
static void swap_long_extremes(void **state)
{
	(void)state;

	check_swap(LONG_MIN, true);
	check_swap(LONG_MAX, true);
#if LONG_MAX == 2147483647L
	check_swap(LONG_MIN + 128, false);
	check_swap(LONG_MAX - 127, false);
#else
	check_swap(LONG_MIN + 8, false);
	check_swap(LONG_MAX - 7, false);
	/* the ends of a 32-bit long, as a 64-bit one */
	check_swap((long)INT_MIN, true);
	check_swap((long)INT_MAX, true);
	check_swap((long)INT_MIN + 128, false);
#endif
}

/* The rotation matters only on the async route. */
static void swap_only_when_async(void **state)
{
	(void)state;
	const long rotations[] = {0, 90, 45, LONG_MIN, LONG_MAX};

	for (size_t r = 0; r < sizeof(rotations) / sizeof(rotations[0]); ++r) {
		const long rotation = rotations[r];
		check_route(node_of(TYPE_INPUT, true, true, false, true, true, rotation), CALLBACK);
		check_route(node_of(TYPE_FILTER, true, false, true, true, true, rotation), TARGET);
		check_route(node_of(TYPE_TRANSITION, true, false, false, true, true, rotation), TRANSITION);
		check_route(node_of(TYPE_INPUT, true, false, false, true, false, rotation), ZERO);
	}
}

/* Where obs_source_get_width() starts. */
static void entry_points(void **state)
{
	(void)state;

	/* a filter starts at itself, whatever filters it has (ed8998e) */
	check_entry(TYPE_FILTER, false, SELF);
	check_entry(TYPE_FILTER, true, SELF);
	/* an input starts at its first filter, so filters that resize count
	 * (3510e69) */
	check_entry(TYPE_INPUT, true, FIRST_FILTER);
	check_entry(TYPE_INPUT, false, SELF);
	/* a transition and a scene start the same way */
	check_entry(TYPE_TRANSITION, true, FIRST_FILTER);
	check_entry(TYPE_TRANSITION, false, SELF);
	check_entry(TYPE_SCENE, true, FIRST_FILTER);
	check_entry(TYPE_SCENE, false, SELF);
	/* values outside the enum start as an input */
	check_entry(-1, true, FIRST_FILTER);
	check_entry(4, true, FIRST_FILTER);
	check_entry(INT_MIN, true, FIRST_FILTER);
	check_entry(INT_MAX, false, SELF);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(route_every_combination),
		cmocka_unit_test(route_every_combination_rotated),
		cmocka_unit_test(route_other_types_as_inputs),
		cmocka_unit_test(route_transition),
		cmocka_unit_test(route_filter_chains),
		cmocka_unit_test(route_detached_filter),
		cmocka_unit_test(route_no_data),
		cmocka_unit_test(swap_rotations),
		cmocka_unit_test(swap_long_extremes),
		cmocka_unit_test(swap_only_when_async),
		cmocka_unit_test(entry_points),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
