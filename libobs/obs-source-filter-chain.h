/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 2 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.
******************************************************************************/

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/* The filter chain decisions of obs-source.c, on plain values
 * (obs-source-filter-chain.c):
 * - obs_source_filter_chain_compatible(): filter_compatible(), which
 *   obs_source_filter_add() and duplicate_filter() ask;
 * - obs_source_filter_chain_add_allowed(),
 *   obs_source_filter_chain_checks_compatible(),
 *   obs_source_filter_chain_ref_step() and
 *   obs_source_filter_chain_add_target(): the gates of
 *   obs_source_filter_add() and the new filter's target;
 * - obs_source_filter_chain_remove_retarget(): the filter that
 *   obs_source_filter_remove_refless() retargets;
 * - obs_source_filter_chain_move_route(), obs_source_filter_chain_walk_step(),
 *   obs_source_filter_chain_same_kind() and
 *   obs_source_filter_chain_edge_move(): the movements of move_filter_dir(),
 *   with the walks of find_next_filter() and find_prev_filter();
 * - obs_source_filter_chain_target_of(): each filter's target in
 *   reorder_filter_targets();
 * - obs_source_filter_chain_restore_matches() and
 *   obs_source_filter_chain_restore_step(): which current filter
 *   obs_source_restore_filters() keeps for a backed-up one, and how it
 *   loads one that has none.
 *
 * obs-source.c keeps every filter_mutex lock, da_find(), da_insert(),
 * da_erase(), da_move_item() and da_move(), every filter_parent and
 * filter_target write, every obs_source_get_ref() and release,
 * obs_load_source(), every obs_data call, the filter_add and filter_remove
 * callbacks, every signal and every log, each where it was. Nothing here
 * takes a lock, calls into the core or keeps state.
 *
 * Hidden in libobs: not EXPORT. */

/* obs-source.c checks that these are OBS_SOURCE_VIDEO, OBS_SOURCE_AUDIO,
 * OBS_SOURCE_ASYNC and the four enum obs_order_movement values. */
#define OBS_SOURCE_FILTER_CHAIN_VIDEO (1 << 0)
#define OBS_SOURCE_FILTER_CHAIN_AUDIO (1 << 1)
#define OBS_SOURCE_FILTER_CHAIN_ASYNC (1 << 2)
#define OBS_SOURCE_FILTER_CHAIN_MOVE_UP 0
#define OBS_SOURCE_FILTER_CHAIN_MOVE_DOWN 1
#define OBS_SOURCE_FILTER_CHAIN_MOVE_TOP 2
#define OBS_SOURCE_FILTER_CHAIN_MOVE_BOTTOM 3

/* What obs_source_filter_add() does after referencing the filter. */
enum obs_source_filter_chain_ref {
	/* Set the parent and target and insert the filter. */
	OBS_SOURCE_FILTER_CHAIN_REF_ADD = 0,
	/* Return, with filter_mutex still locked. Only the C gives it. */
	OBS_SOURCE_FILTER_CHAIN_REF_RETURN = 1,
	/* Unlock filter_mutex, then return. Only the Rust port gives it
	 * (#2981). */
	OBS_SOURCE_FILTER_CHAIN_REF_UNLOCK_RETURN = 2,
};

/* What a filter targets. */
enum obs_source_filter_chain_target {
	/* The source the filter is on. */
	OBS_SOURCE_FILTER_CHAIN_TARGET_SOURCE = 0,
	/* The filter after it in the list (for a new filter, the first one). */
	OBS_SOURCE_FILTER_CHAIN_TARGET_FILTER = 1,
};

/* What move_filter_dir() does for a movement. */
enum obs_source_filter_chain_route {
	/* Walk up the list to the nearest filter of the same kind
	 * (find_next_filter()). */
	OBS_SOURCE_FILTER_CHAIN_ROUTE_UP = 0,
	/* Walk down the list to the nearest filter of the same kind
	 * (find_prev_filter()). */
	OBS_SOURCE_FILTER_CHAIN_ROUTE_DOWN = 1,
	/* Move to the end of the list (the top), whatever the kind. */
	OBS_SOURCE_FILTER_CHAIN_ROUTE_TOP = 2,
	/* Move to the start of the list (the bottom), whatever the kind. */
	OBS_SOURCE_FILTER_CHAIN_ROUTE_BOTTOM = 3,
	/* Move nothing, but retarget every filter and report a move. Only the C
	 * gives it, for a movement outside the enum. */
	OBS_SOURCE_FILTER_CHAIN_ROUTE_RETARGET = 4,
	/* Move nothing and report no move. Only the Rust port gives it, for a
	 * movement outside the enum (#2981). */
	OBS_SOURCE_FILTER_CHAIN_ROUTE_STAY = 5,
};

/* How obs_source_restore_filters() gets a backed-up filter. */
enum obs_source_filter_chain_restore {
	/* Keep the current filter it found (already referenced). */
	OBS_SOURCE_FILTER_CHAIN_RESTORE_REUSE = 0,
	/* Load it from its data with obs_load_source(), public unless its type
	 * is a registered filter. Only the C gives it. */
	OBS_SOURCE_FILTER_CHAIN_RESTORE_LOAD = 1,
	/* Load it from its data with obs_load_private_source(). Only the Rust
	 * port gives it (#2981). */
	OBS_SOURCE_FILTER_CHAIN_RESTORE_LOAD_PRIVATE = 2,
};

/* source_flags and filter_flags: the source's and the filter's
 * info.output_flags.
 *
 * Returns whether the filter fits the source: its video, audio and async
 * bits are a subset of the source's, without the async bit for a filter
 * that has audio and no video. */
bool obs_source_filter_chain_compatible(uint32_t source_flags, uint32_t filter_flags);

/* listed: the filter is in the source's list (da_find()). has_parent: its
 * filter_parent is set. Read under the source's filter_mutex.
 *
 * Returns whether obs_source_filter_add() goes on; otherwise it warns that
 * the filter is already present and returns. The C refuses only a listed
 * filter; the Rust port also one that has a parent (#2981). */
bool obs_source_filter_chain_add_allowed(bool listed, bool has_parent);

/* owns_info_id: the source's owns_info_id.
 *
 * Returns whether obs_source_filter_add() checks filter_compatible(): not
 * for a source that owns its info id. */
bool obs_source_filter_chain_checks_compatible(bool owns_info_id);

/* referenced: obs_source_get_ref() of the filter gave a filter.
 *
 * Returns an enum obs_source_filter_chain_ref value: ADD when it did, else
 * RETURN in the C and UNLOCK_RETURN in the Rust port. */
int obs_source_filter_chain_ref_step(bool referenced);

/* count: the number of filters on the source before the new one goes in at
 * the start.
 *
 * Returns an enum obs_source_filter_chain_target value: SOURCE for an empty
 * list, else FILTER (the first filter). */
int obs_source_filter_chain_add_target(size_t count);

/* idx: the removed filter's index in the list.
 *
 * Returns whether a filter takes the removed filter's target, and stores
 * its index (the one before, idx - 1) in *prev. */
bool obs_source_filter_chain_remove_retarget(size_t idx, size_t *prev);

/* movement: the enum obs_order_movement value, as an int.
 *
 * Returns an enum obs_source_filter_chain_route value: UP, DOWN, TOP or
 * BOTTOM for the four movements; for any other, RETARGET in the C and STAY
 * in the Rust port. */
int obs_source_filter_chain_move_route(int movement);

/* route: ROUTE_UP or ROUTE_DOWN. cur: the index the walk is at. count: the
 * number of filters.
 *
 * Returns false at the end of the list (the last index going up, 0 going
 * down); otherwise stores the next index of the walk in *next (cur + 1 up,
 * cur - 1 down) and returns true. */
bool obs_source_filter_chain_walk_step(int route, size_t cur, size_t count, size_t *next);

/* filter_flags and other_flags: the moved filter's and another filter's
 * info.output_flags.
 *
 * Returns whether both are async or both are not: the walk stops at that
 * filter. */
bool obs_source_filter_chain_same_kind(uint32_t filter_flags, uint32_t other_flags);

/* route: ROUTE_TOP or ROUTE_BOTTOM. idx: the moved filter's index. count:
 * the number of filters.
 *
 * Returns false when the filter is already there; otherwise stores where it
 * goes in *to (count - 1 for the top, 0 for the bottom) and returns true. */
bool obs_source_filter_chain_edge_move(int route, size_t idx, size_t count, size_t *to);

/* i: a filter's index. count: the number of filters.
 *
 * Returns an enum obs_source_filter_chain_target value: SOURCE for the last
 * filter, else FILTER (the one at i + 1). */
int obs_source_filter_chain_target_of(size_t i, size_t count);

/* cur_name: a current filter's context.name (may be NULL). name: the
 * backed-up filter's "name" (obs_data_get_string(), never NULL).
 *
 * Returns whether the current filter is the backed-up one: the same name.
 * A NULL cur_name never matches. */
bool obs_source_filter_chain_restore_matches(const char *cur_name, const char *name);

/* found: a current filter matched and was referenced.
 *
 * Returns an enum obs_source_filter_chain_restore value: REUSE when one
 * was, else LOAD in the C and LOAD_PRIVATE in the Rust port. */
int obs_source_filter_chain_restore_step(bool found);
