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

#include "libobs/build/libobs-ported-source-filter-chain.pch.h"

/* Ported to rust/obs-source (source_filter_chain): the hybrid libobs drops
 * this file. */

/* The filter chain decisions of obs-source.c, moved: the decisions, on
 * plain values. obs-source.c keeps the locks, the list edits, the parent and
 * target writes and every other call. Declared in obs-source-filter-chain.h
 * without EXPORT. */

#define OBS_SOURCE_FILTER_CHAIN_AV                                             \
	(OBS_SOURCE_FILTER_CHAIN_ASYNC | OBS_SOURCE_FILTER_CHAIN_VIDEO | OBS_SOURCE_FILTER_CHAIN_AUDIO)

/* filter_compatible(). */
bool obs_source_filter_chain_compatible(uint32_t source_flags, uint32_t filter_flags)
{
	uint32_t s_caps = source_flags & OBS_SOURCE_FILTER_CHAIN_AV;
	uint32_t f_caps = filter_flags & OBS_SOURCE_FILTER_CHAIN_AV;

	if ((f_caps & OBS_SOURCE_FILTER_CHAIN_AUDIO) != 0 && (f_caps & OBS_SOURCE_FILTER_CHAIN_VIDEO) == 0)
		f_caps &= ~(uint32_t)OBS_SOURCE_FILTER_CHAIN_ASYNC;

	return (s_caps & f_caps) == f_caps;
}

/* The list check of obs_source_filter_add(): a filter on another source is
 * taken. */
bool obs_source_filter_chain_add_allowed(bool listed, bool has_parent)
{
	UNUSED_PARAMETER(has_parent);
	return !listed;
}

bool obs_source_filter_chain_checks_compatible(bool owns_info_id)
{
	return !owns_info_id;
}

/* A filter that can't be referenced returns without unlocking. */
int obs_source_filter_chain_ref_step(bool referenced)
{
	return referenced ? OBS_SOURCE_FILTER_CHAIN_REF_ADD : OBS_SOURCE_FILTER_CHAIN_REF_RETURN;
}

int obs_source_filter_chain_add_target(size_t count)
{
	return !count ? OBS_SOURCE_FILTER_CHAIN_TARGET_SOURCE : OBS_SOURCE_FILTER_CHAIN_TARGET_FILTER;
}

/* The retarget of obs_source_filter_remove_refless(). */
bool obs_source_filter_chain_remove_retarget(size_t idx, size_t *prev)
{
	if (idx > 0) {
		*prev = idx - 1;
		return true;
	}

	return false;
}

/* The movement branches of move_filter_dir(). */
int obs_source_filter_chain_move_route(int movement)
{
	if (movement == OBS_SOURCE_FILTER_CHAIN_MOVE_UP)
		return OBS_SOURCE_FILTER_CHAIN_ROUTE_UP;
	else if (movement == OBS_SOURCE_FILTER_CHAIN_MOVE_DOWN)
		return OBS_SOURCE_FILTER_CHAIN_ROUTE_DOWN;
	else if (movement == OBS_SOURCE_FILTER_CHAIN_MOVE_TOP)
		return OBS_SOURCE_FILTER_CHAIN_ROUTE_TOP;
	else if (movement == OBS_SOURCE_FILTER_CHAIN_MOVE_BOTTOM)
		return OBS_SOURCE_FILTER_CHAIN_ROUTE_BOTTOM;

	return OBS_SOURCE_FILTER_CHAIN_ROUTE_RETARGET;
}

/* The end checks and steps of find_next_filter() and find_prev_filter(). */
bool obs_source_filter_chain_walk_step(int route, size_t cur, size_t count, size_t *next)
{
	if (route == OBS_SOURCE_FILTER_CHAIN_ROUTE_UP) {
		if (cur == count - 1)
			return false;
		*next = cur + 1;
		return true;
	}

	if (cur == 0)
		return false;
	*next = cur - 1;
	return true;
}

/* The kind compare of find_next_filter() and find_prev_filter(). */
bool obs_source_filter_chain_same_kind(uint32_t filter_flags, uint32_t other_flags)
{
	bool cur_async = (filter_flags & OBS_SOURCE_FILTER_CHAIN_ASYNC) != 0;
	bool other_async = (other_flags & OBS_SOURCE_FILTER_CHAIN_ASYNC);

	return other_async == cur_async;
}

/* The top and bottom branches of move_filter_dir(). */
bool obs_source_filter_chain_edge_move(int route, size_t idx, size_t count, size_t *to)
{
	if (route == OBS_SOURCE_FILTER_CHAIN_ROUTE_TOP) {
		if (idx == count - 1)
			return false;
		*to = count - 1;
		return true;
	}

	if (idx == 0)
		return false;
	*to = 0;
	return true;
}

/* The next filter of reorder_filter_targets(). */
int obs_source_filter_chain_target_of(size_t i, size_t count)
{
	return (i == count - 1) ? OBS_SOURCE_FILTER_CHAIN_TARGET_SOURCE : OBS_SOURCE_FILTER_CHAIN_TARGET_FILTER;
}

/* The name compare of obs_source_restore_filters(). */
bool obs_source_filter_chain_restore_matches(const char *cur_name, const char *name)
{
	return cur_name && strcmp(cur_name, name) == 0;
}

/* A backed-up filter with no current one is loaded public. */
int obs_source_filter_chain_restore_step(bool found)
{
	return found ? OBS_SOURCE_FILTER_CHAIN_RESTORE_REUSE : OBS_SOURCE_FILTER_CHAIN_RESTORE_LOAD;
}

#undef OBS_SOURCE_FILTER_CHAIN_AV
