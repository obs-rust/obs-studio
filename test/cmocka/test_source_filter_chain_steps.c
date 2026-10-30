#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-source-filter-chain.h"

/*
 * Characterization of the filter chain decisions of libobs/obs-source.c,
 * moved to libobs/obs-source-filter-chain.c before their Rust port:
 * - filter_compatible();
 * - the gates of obs_source_filter_add() and the new filter's target;
 * - the filter obs_source_filter_remove_refless() retargets;
 * - the movements of move_filter_dir(), with the walks of find_next_filter()
 *   and find_prev_filter(), and the targets of reorder_filter_targets();
 * - the name match of obs_source_restore_filters() and how it gets a
 *   backed-up filter.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them. Models of the callers below keep a chain
 * of filter ids the way obs-source.c keeps its list (index 0 is the bottom,
 * the filter that renders the source first).
 *
 * Not pinned here, because the C does something the port fixes: a filter
 * that already has a parent on another source (the C takes it), a filter
 * that can't be referenced (the C returns with filter_mutex locked), a
 * movement outside enum obs_order_movement (the C retargets every filter
 * and reports a move) and how a backed-up filter with no current one is
 * loaded (the C loads it public). Only what the C and the port share is
 * asserted for those inputs.
 */

#define ARRAY_LEN(a) (sizeof(a) / sizeof((a)[0]))

#define VIDEO OBS_SOURCE_FILTER_CHAIN_VIDEO
#define AUDIO OBS_SOURCE_FILTER_CHAIN_AUDIO
#define ASYNC OBS_SOURCE_FILTER_CHAIN_ASYNC
#define ASYNC_VIDEO (ASYNC | VIDEO)

/* The source a filter targets in the models. */
#define TARGET_SOURCE -1

#define MAX_FILTERS 8

struct chain {
	size_t num;
	int ids[MAX_FILTERS];
	uint32_t flags[MAX_FILTERS];
	/* the id of the filter each one targets, or TARGET_SOURCE */
	int targets[MAX_FILTERS];
};

static void push(struct chain *c, int id, uint32_t flags)
{
	c->ids[c->num] = id;
	c->flags[c->num] = flags;
	c->num++;
}

/* reorder_filter_targets() */
static void retarget_all(struct chain *c)
{
	for (size_t i = 0; i < c->num; i++) {
		c->targets[i] = obs_source_filter_chain_target_of(i, c->num) == OBS_SOURCE_FILTER_CHAIN_TARGET_SOURCE
					? TARGET_SOURCE
					: c->ids[i + 1];
	}
}

static void assert_chain(const struct chain *c, const int *ids, size_t num)
{
	assert_int_equal(c->num, num);
	for (size_t i = 0; i < num; i++) {
		assert_int_equal(c->ids[i], ids[i]);
		assert_int_equal(c->targets[i], i + 1 < num ? ids[i + 1] : TARGET_SOURCE);
	}
}

/* ------------------------------------------------------------------------ */
/* filter_compatible() */

static void compatibility(void **state)
{
	(void)state;

	/* an audio filter on a video source */
	assert_false(obs_source_filter_chain_compatible(VIDEO, AUDIO));
	/* an async video filter on a sync video source */
	assert_false(obs_source_filter_chain_compatible(VIDEO, ASYNC_VIDEO));
	assert_true(obs_source_filter_chain_compatible(ASYNC_VIDEO, ASYNC_VIDEO));
	/* an audio filter that also sets the async bit: the bit is dropped */
	assert_true(obs_source_filter_chain_compatible(AUDIO, AUDIO | ASYNC));
	assert_true(obs_source_filter_chain_compatible(VIDEO | AUDIO, AUDIO | ASYNC));
	/* but not for a filter that also has video */
	assert_false(obs_source_filter_chain_compatible(VIDEO | AUDIO, ASYNC_VIDEO | AUDIO));
	assert_true(obs_source_filter_chain_compatible(ASYNC_VIDEO | AUDIO, ASYNC_VIDEO | AUDIO));
	/* a video filter needs video, an audio filter audio */
	assert_false(obs_source_filter_chain_compatible(AUDIO, VIDEO));
	assert_false(obs_source_filter_chain_compatible(0, VIDEO));
	assert_true(obs_source_filter_chain_compatible(VIDEO | AUDIO, VIDEO));
	assert_true(obs_source_filter_chain_compatible(VIDEO | AUDIO, AUDIO));
	/* only an async filter needs the async bit */
	assert_false(obs_source_filter_chain_compatible(VIDEO, ASYNC));
	assert_true(obs_source_filter_chain_compatible(ASYNC, ASYNC));
	/* other bits are ignored */
	assert_true(obs_source_filter_chain_compatible(0, 0xfffffff8u));
	assert_true(obs_source_filter_chain_compatible(VIDEO, 0xfffffff8u | VIDEO));
	assert_true(obs_source_filter_chain_compatible(0xfffffff8u, 0));
}

/* ------------------------------------------------------------------------ */
/* obs_source_filter_add() */

static void add_gates(void **state)
{
	(void)state;

	/* a filter already in the list is refused */
	assert_false(obs_source_filter_chain_add_allowed(true, true));
	assert_true(obs_source_filter_chain_add_allowed(false, false));

	/* a source that owns its info id skips the compatibility check */
	assert_false(obs_source_filter_chain_checks_compatible(true));
	assert_true(obs_source_filter_chain_checks_compatible(false));

	assert_int_equal(obs_source_filter_chain_ref_step(true), OBS_SOURCE_FILTER_CHAIN_REF_ADD);
	/* a failed reference never adds */
	assert_int_not_equal(obs_source_filter_chain_ref_step(false), OBS_SOURCE_FILTER_CHAIN_REF_ADD);
}

/* A model of obs_source_filter_add() from the list check on, for a source
 * whose filters all pass. */
static bool add(struct chain *c, int id, uint32_t flags, bool owns_info_id, uint32_t source_flags)
{
	bool listed = false;
	for (size_t i = 0; i < c->num; i++)
		listed = listed || c->ids[i] == id;

	if (!obs_source_filter_chain_add_allowed(listed, listed))
		return false;
	if (obs_source_filter_chain_checks_compatible(owns_info_id) &&
	    !obs_source_filter_chain_compatible(source_flags, flags))
		return false;
	if (obs_source_filter_chain_ref_step(true) != OBS_SOURCE_FILTER_CHAIN_REF_ADD)
		return false;

	int target = obs_source_filter_chain_add_target(c->num) == OBS_SOURCE_FILTER_CHAIN_TARGET_SOURCE ? TARGET_SOURCE
													 : c->ids[0];
	memmove(&c->ids[1], &c->ids[0], c->num * sizeof(c->ids[0]));
	memmove(&c->flags[1], &c->flags[0], c->num * sizeof(c->flags[0]));
	memmove(&c->targets[1], &c->targets[0], c->num * sizeof(c->targets[0]));
	c->ids[0] = id;
	c->flags[0] = flags;
	c->targets[0] = target;
	c->num++;
	return true;
}

static void add_targets(void **state)
{
	(void)state;
	struct chain c = {0};

	assert_int_equal(obs_source_filter_chain_add_target(0), OBS_SOURCE_FILTER_CHAIN_TARGET_SOURCE);
	assert_int_equal(obs_source_filter_chain_add_target(1), OBS_SOURCE_FILTER_CHAIN_TARGET_FILTER);
	assert_int_equal(obs_source_filter_chain_add_target(2), OBS_SOURCE_FILTER_CHAIN_TARGET_FILTER);

	/* onto an empty list: the new filter targets the source */
	assert_true(add(&c, 1, VIDEO, false, VIDEO));
	assert_chain(&c, (const int[]){1}, 1);

	/* onto a list of two: it goes first and targets the old first */
	assert_true(add(&c, 2, VIDEO, false, VIDEO));
	assert_true(add(&c, 3, VIDEO, false, VIDEO));
	assert_chain(&c, (const int[]){3, 2, 1}, 3);

	/* again: refused, nothing changes */
	assert_false(add(&c, 2, VIDEO, false, VIDEO));
	assert_chain(&c, (const int[]){3, 2, 1}, 3);

	/* an audio filter on a video source is refused... */
	assert_false(add(&c, 4, AUDIO, false, VIDEO));
	assert_chain(&c, (const int[]){3, 2, 1}, 3);
	/* ...unless the source owns its info id */
	assert_true(add(&c, 4, AUDIO, true, VIDEO));
	assert_chain(&c, (const int[]){4, 3, 2, 1}, 4);
}

/* ------------------------------------------------------------------------ */
/* obs_source_filter_remove_refless() */

static void remove_at(struct chain *c, size_t idx)
{
	size_t prev = 0;
	if (obs_source_filter_chain_remove_retarget(idx, &prev))
		c->targets[prev] = c->targets[idx];

	memmove(&c->ids[idx], &c->ids[idx + 1], (c->num - idx - 1) * sizeof(c->ids[0]));
	memmove(&c->flags[idx], &c->flags[idx + 1], (c->num - idx - 1) * sizeof(c->flags[0]));
	memmove(&c->targets[idx], &c->targets[idx + 1], (c->num - idx - 1) * sizeof(c->targets[0]));
	c->num--;
}

static struct chain chain_of(const int *ids, size_t num, const uint32_t *flags)
{
	struct chain c = {0};
	for (size_t i = 0; i < num; i++)
		push(&c, ids[i], flags ? flags[i] : VIDEO);
	retarget_all(&c);
	return c;
}

static void remove_retargets(void **state)
{
	(void)state;
	size_t prev = 99;

	/* the first filter: nothing to retarget */
	assert_false(obs_source_filter_chain_remove_retarget(0, &prev));
	assert_int_equal(prev, 99);
	assert_true(obs_source_filter_chain_remove_retarget(1, &prev));
	assert_int_equal(prev, 0);
	assert_true(obs_source_filter_chain_remove_retarget(5, &prev));
	assert_int_equal(prev, 4);

	const int ids[] = {1, 2, 3, 4};

	struct chain c = chain_of(ids, 4, NULL);
	remove_at(&c, 0);
	assert_chain(&c, (const int[]){2, 3, 4}, 3);

	c = chain_of(ids, 4, NULL);
	remove_at(&c, 2);
	assert_chain(&c, (const int[]){1, 2, 4}, 3);

	/* the last: the one before it takes the source */
	c = chain_of(ids, 4, NULL);
	remove_at(&c, 3);
	assert_chain(&c, (const int[]){1, 2, 3}, 3);

	c = chain_of(ids, 1, NULL);
	remove_at(&c, 0);
	assert_int_equal(c.num, 0);
}

/* ------------------------------------------------------------------------ */
/* move_filter_dir() */

#define NOT_FOUND ((size_t)-1)

/* find_next_filter() and find_prev_filter() */
static size_t walk(const struct chain *c, int route, size_t idx)
{
	size_t cur = idx;
	size_t next;

	while (obs_source_filter_chain_walk_step(route, cur, c->num, &next)) {
		if (obs_source_filter_chain_same_kind(c->flags[idx], c->flags[next]))
			return next;
		cur = next;
	}

	return NOT_FOUND;
}

static void move_item(struct chain *c, size_t from, size_t to)
{
	int id = c->ids[from];
	uint32_t flags = c->flags[from];

	if (from < to) {
		memmove(&c->ids[from], &c->ids[from + 1], (to - from) * sizeof(c->ids[0]));
		memmove(&c->flags[from], &c->flags[from + 1], (to - from) * sizeof(c->flags[0]));
	} else if (from > to) {
		memmove(&c->ids[to + 1], &c->ids[to], (from - to) * sizeof(c->ids[0]));
		memmove(&c->flags[to + 1], &c->flags[to], (from - to) * sizeof(c->flags[0]));
	}
	c->ids[to] = id;
	c->flags[to] = flags;
}

/* A model of move_filter_dir() for a listed filter. */
static bool move(struct chain *c, size_t idx, int movement)
{
	int route = obs_source_filter_chain_move_route(movement);
	size_t to;

	if (route == OBS_SOURCE_FILTER_CHAIN_ROUTE_UP || route == OBS_SOURCE_FILTER_CHAIN_ROUTE_DOWN) {
		to = walk(c, route, idx);
		if (to == NOT_FOUND)
			return false;
		move_item(c, idx, to);
	} else if (route == OBS_SOURCE_FILTER_CHAIN_ROUTE_TOP || route == OBS_SOURCE_FILTER_CHAIN_ROUTE_BOTTOM) {
		if (!obs_source_filter_chain_edge_move(route, idx, c->num, &to))
			return false;
		move_item(c, idx, to);
	} else {
		return false;
	}

	retarget_all(c);
	return true;
}

static void move_routes(void **state)
{
	(void)state;

	assert_int_equal(obs_source_filter_chain_move_route(OBS_SOURCE_FILTER_CHAIN_MOVE_UP),
			 OBS_SOURCE_FILTER_CHAIN_ROUTE_UP);
	assert_int_equal(obs_source_filter_chain_move_route(OBS_SOURCE_FILTER_CHAIN_MOVE_DOWN),
			 OBS_SOURCE_FILTER_CHAIN_ROUTE_DOWN);
	assert_int_equal(obs_source_filter_chain_move_route(OBS_SOURCE_FILTER_CHAIN_MOVE_TOP),
			 OBS_SOURCE_FILTER_CHAIN_ROUTE_TOP);
	assert_int_equal(obs_source_filter_chain_move_route(OBS_SOURCE_FILTER_CHAIN_MOVE_BOTTOM),
			 OBS_SOURCE_FILTER_CHAIN_ROUTE_BOTTOM);

	/* a movement outside the enum takes none of the four routes */
	const int unknown[] = {-1, 4, 5, 1000};
	for (size_t i = 0; i < ARRAY_LEN(unknown); i++) {
		int route = obs_source_filter_chain_move_route(unknown[i]);
		assert_true(route != OBS_SOURCE_FILTER_CHAIN_ROUTE_UP && route != OBS_SOURCE_FILTER_CHAIN_ROUTE_DOWN &&
			    route != OBS_SOURCE_FILTER_CHAIN_ROUTE_TOP &&
			    route != OBS_SOURCE_FILTER_CHAIN_ROUTE_BOTTOM);
	}
}

static void walk_steps(void **state)
{
	(void)state;
	size_t next = 99;

	assert_true(obs_source_filter_chain_walk_step(OBS_SOURCE_FILTER_CHAIN_ROUTE_UP, 0, 3, &next));
	assert_int_equal(next, 1);
	assert_true(obs_source_filter_chain_walk_step(OBS_SOURCE_FILTER_CHAIN_ROUTE_UP, 1, 3, &next));
	assert_int_equal(next, 2);
	next = 99;
	assert_false(obs_source_filter_chain_walk_step(OBS_SOURCE_FILTER_CHAIN_ROUTE_UP, 2, 3, &next));
	assert_int_equal(next, 99);

	assert_true(obs_source_filter_chain_walk_step(OBS_SOURCE_FILTER_CHAIN_ROUTE_DOWN, 2, 3, &next));
	assert_int_equal(next, 1);
	next = 99;
	assert_false(obs_source_filter_chain_walk_step(OBS_SOURCE_FILTER_CHAIN_ROUTE_DOWN, 0, 3, &next));
	assert_int_equal(next, 99);

	/* only the async bit decides the kind */
	assert_true(obs_source_filter_chain_same_kind(VIDEO, AUDIO));
	assert_true(obs_source_filter_chain_same_kind(ASYNC_VIDEO, ASYNC));
	assert_true(obs_source_filter_chain_same_kind(AUDIO | ASYNC, ASYNC_VIDEO));
	assert_false(obs_source_filter_chain_same_kind(VIDEO, ASYNC_VIDEO));
	assert_false(obs_source_filter_chain_same_kind(ASYNC_VIDEO, 0));
	assert_true(obs_source_filter_chain_same_kind(0xfffffffbu, 0));
}

static void edge_moves(void **state)
{
	(void)state;
	size_t to = 99;

	assert_true(obs_source_filter_chain_edge_move(OBS_SOURCE_FILTER_CHAIN_ROUTE_TOP, 0, 3, &to));
	assert_int_equal(to, 2);
	to = 99;
	assert_false(obs_source_filter_chain_edge_move(OBS_SOURCE_FILTER_CHAIN_ROUTE_TOP, 2, 3, &to));
	assert_int_equal(to, 99);
	assert_true(obs_source_filter_chain_edge_move(OBS_SOURCE_FILTER_CHAIN_ROUTE_BOTTOM, 2, 3, &to));
	assert_int_equal(to, 0);
	to = 99;
	assert_false(obs_source_filter_chain_edge_move(OBS_SOURCE_FILTER_CHAIN_ROUTE_BOTTOM, 0, 3, &to));
	assert_int_equal(to, 99);
	/* the only filter is both */
	assert_false(obs_source_filter_chain_edge_move(OBS_SOURCE_FILTER_CHAIN_ROUTE_TOP, 0, 1, &to));
	assert_false(obs_source_filter_chain_edge_move(OBS_SOURCE_FILTER_CHAIN_ROUTE_BOTTOM, 0, 1, &to));
}

static void target_chain(void **state)
{
	(void)state;

	assert_int_equal(obs_source_filter_chain_target_of(0, 1), OBS_SOURCE_FILTER_CHAIN_TARGET_SOURCE);
	assert_int_equal(obs_source_filter_chain_target_of(0, 3), OBS_SOURCE_FILTER_CHAIN_TARGET_FILTER);
	assert_int_equal(obs_source_filter_chain_target_of(1, 3), OBS_SOURCE_FILTER_CHAIN_TARGET_FILTER);
	assert_int_equal(obs_source_filter_chain_target_of(2, 3), OBS_SOURCE_FILTER_CHAIN_TARGET_SOURCE);
}

static void moves_skip_the_other_kind(void **state)
{
	(void)state;
	/* sync, async, async, sync, sync */
	const int ids[] = {1, 2, 3, 4, 5};
	const uint32_t flags[] = {VIDEO, ASYNC_VIDEO, ASYNC_VIDEO, VIDEO, AUDIO};

	/* up from 1 passes the two async filters */
	struct chain c = chain_of(ids, 5, flags);
	assert_true(move(&c, 0, OBS_SOURCE_FILTER_CHAIN_MOVE_UP));
	assert_chain(&c, (const int[]){2, 3, 4, 1, 5}, 5);

	/* down from 4 passes them too */
	c = chain_of(ids, 5, flags);
	assert_true(move(&c, 3, OBS_SOURCE_FILTER_CHAIN_MOVE_DOWN));
	assert_chain(&c, (const int[]){4, 1, 2, 3, 5}, 5);

	/* an async filter moves among the async ones */
	c = chain_of(ids, 5, flags);
	assert_true(move(&c, 1, OBS_SOURCE_FILTER_CHAIN_MOVE_UP));
	assert_chain(&c, (const int[]){1, 3, 2, 4, 5}, 5);

	/* and no further: no async filter above 3, none below 2 */
	c = chain_of(ids, 5, flags);
	assert_false(move(&c, 2, OBS_SOURCE_FILTER_CHAIN_MOVE_UP));
	assert_chain(&c, ids, 5);
	assert_false(move(&c, 1, OBS_SOURCE_FILTER_CHAIN_MOVE_DOWN));
	assert_chain(&c, ids, 5);

	/* up at the top and down at the bottom are refused */
	assert_false(move(&c, 4, OBS_SOURCE_FILTER_CHAIN_MOVE_UP));
	assert_false(move(&c, 0, OBS_SOURCE_FILTER_CHAIN_MOVE_DOWN));
	assert_chain(&c, ids, 5);
}

static void top_and_bottom_ignore_the_kind(void **state)
{
	(void)state;
	const int ids[] = {1, 2, 3, 4};
	const uint32_t flags[] = {VIDEO, ASYNC_VIDEO, VIDEO, AUDIO};

	/* characterized, not endorsed: an async filter goes above sync ones */
	struct chain c = chain_of(ids, 4, flags);
	assert_true(move(&c, 1, OBS_SOURCE_FILTER_CHAIN_MOVE_TOP));
	assert_chain(&c, (const int[]){1, 3, 4, 2}, 4);

	c = chain_of(ids, 4, flags);
	assert_true(move(&c, 1, OBS_SOURCE_FILTER_CHAIN_MOVE_BOTTOM));
	assert_chain(&c, (const int[]){2, 1, 3, 4}, 4);

	c = chain_of(ids, 4, flags);
	assert_false(move(&c, 3, OBS_SOURCE_FILTER_CHAIN_MOVE_TOP));
	assert_false(move(&c, 0, OBS_SOURCE_FILTER_CHAIN_MOVE_BOTTOM));
	assert_chain(&c, ids, 4);
}

/* ------------------------------------------------------------------------ */
/* obs_source_restore_filters() */

#define LOADED -1

/* A model of the backup loop: for each backed-up name, the index of the
 * current filter it keeps, or LOADED. */
static void restore(const char *const *backup, size_t backup_num, const char *const *current, size_t current_num,
		    int *plan)
{
	for (size_t i = 0; i < backup_num; i++) {
		bool found = false;
		plan[i] = LOADED;
		for (size_t j = 0; j < current_num; j++) {
			if (obs_source_filter_chain_restore_matches(current[j], backup[i])) {
				plan[i] = (int)j;
				found = true;
				break;
			}
		}

		int step = obs_source_filter_chain_restore_step(found);
		if (found)
			assert_int_equal(step, OBS_SOURCE_FILTER_CHAIN_RESTORE_REUSE);
		else
			assert_int_not_equal(step, OBS_SOURCE_FILTER_CHAIN_RESTORE_REUSE);
	}
}

static void restore_matches(void **state)
{
	(void)state;

	assert_true(obs_source_filter_chain_restore_matches("Color", "Color"));
	assert_false(obs_source_filter_chain_restore_matches("Color", "color"));
	assert_false(obs_source_filter_chain_restore_matches("Color", "Color 2"));
	assert_true(obs_source_filter_chain_restore_matches("", ""));
	/* a filter with no name never matches, not even "" */
	assert_false(obs_source_filter_chain_restore_matches(NULL, ""));
	assert_false(obs_source_filter_chain_restore_matches(NULL, "Color"));

	assert_int_equal(obs_source_filter_chain_restore_step(true), OBS_SOURCE_FILTER_CHAIN_RESTORE_REUSE);
	assert_int_not_equal(obs_source_filter_chain_restore_step(false), OBS_SOURCE_FILTER_CHAIN_RESTORE_REUSE);
}

static void restore_plans(void **state)
{
	(void)state;
	const char *current[] = {"Color", "Crop", "Sharpen"};
	int plan[4];

	/* every name matched, in the backup's order */
	restore((const char *[]){"Sharpen", "Color", "Crop"}, 3, current, 3, plan);
	assert_int_equal(plan[0], 2);
	assert_int_equal(plan[1], 0);
	assert_int_equal(plan[2], 1);

	/* none matched: each is loaded */
	restore((const char *[]){"Mask", "Gain"}, 2, current, 3, plan);
	assert_int_equal(plan[0], LOADED);
	assert_int_equal(plan[1], LOADED);

	/* characterized, not endorsed: a name backed up twice keeps the same
	 * filter twice */
	restore((const char *[]){"Crop", "Mask", "Crop"}, 3, current, 3, plan);
	assert_int_equal(plan[0], 1);
	assert_int_equal(plan[1], LOADED);
	assert_int_equal(plan[2], 1);

	/* a current name that appears twice: the first is kept */
	restore((const char *[]){"Crop"}, 1, (const char *[]){NULL, "Crop", "Crop"}, 3, plan);
	assert_int_equal(plan[0], 1);

	/* nothing current */
	restore((const char *[]){"Crop"}, 1, current, 0, plan);
	assert_int_equal(plan[0], LOADED);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(compatibility),
		cmocka_unit_test(add_gates),
		cmocka_unit_test(add_targets),
		cmocka_unit_test(remove_retargets),
		cmocka_unit_test(move_routes),
		cmocka_unit_test(walk_steps),
		cmocka_unit_test(edge_moves),
		cmocka_unit_test(target_chain),
		cmocka_unit_test(moves_skip_the_other_kind),
		cmocka_unit_test(top_and_bottom_ignore_the_kind),
		cmocka_unit_test(restore_matches),
		cmocka_unit_test(restore_plans),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
