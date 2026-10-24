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

/* The size query routing of obs_source_get_width() and
 * obs_source_get_height() in obs-source.c, on plain values
 * (obs-source-size.c):
 * - obs_source_size_route(): where get_base_width() and get_base_height()
 *   take a source's size from. A transition reports its actual size while
 *   it is enabled, and 0 otherwise; a source with data and a size callback
 *   asks the callback (a filter only while it is enabled); a filter without
 *   one asks its target; an active async source reports its frame's size,
 *   the axes swapped unless the rotation is a multiple of 180; anything
 *   else reports 0. A filter here is a source with a filter parent.
 * - obs_source_size_entry(): where obs_source_get_width() and
 *   obs_source_get_height() start. A filter starts at itself; any other
 *   source starts at its first filter if it has filters, else at itself.
 *
 * One route serves both axes: obs-source.c reads the callback of the axis
 * it asks for into the node, and applies the route to the width or the
 * height. It keeps data_valid(), filter_mutex (a filter takes no lock; any
 * other source holds its filter_mutex while it reads its filters and asks
 * the start), the recursion into the target, the get_width and get_height
 * callbacks, and the fields each route reads. Neither function takes a
 * lock, calls into libobs or keeps state, and neither follows a source
 * pointer.
 *
 * Hidden in libobs: not EXPORT. */

/* obs-source.c checks that they are OBS_SOURCE_TYPE_FILTER and
 * OBS_SOURCE_TYPE_TRANSITION */
#define OBS_SOURCE_SIZE_TYPE_FILTER 1
#define OBS_SOURCE_SIZE_TYPE_TRANSITION 2

/* The fields of the source the route reads. */
struct obs_source_size_node {
	/* source->info.type, an enum obs_source_type */
	int type;
	/* source->context.data != NULL */
	bool has_data;
	/* source->info.get_width != NULL, or info.get_height for the height */
	bool has_callback;
	/* source->filter_parent != NULL: the source is a filter in a chain */
	bool has_filter_parent;
	/* source->enabled */
	bool enabled;
	/* source->async_active */
	bool async_active;
	/* source->async_rotation, in 0-359 since obs_source_set_async_rotation()
	 * normalizes it; the route takes any value */
	long async_rotation;
};

/* Where get_base_width() and get_base_height() take the size from. */
enum obs_source_size_ask {
	/* transition_actual_cx or transition_actual_cy */
	OBS_SOURCE_SIZE_TRANSITION,
	/* info.get_width or info.get_height */
	OBS_SOURCE_SIZE_CALLBACK,
	/* get_base_width() or get_base_height() of source->filter_target */
	OBS_SOURCE_SIZE_ASK_TARGET,
	/* async_width for the width, async_height for the height */
	OBS_SOURCE_SIZE_ASYNC,
	/* async_height for the width, async_width for the height */
	OBS_SOURCE_SIZE_ASYNC_SWAPPED,
	/* 0 */
	OBS_SOURCE_SIZE_ZERO,
};

/* Where obs_source_get_width() and obs_source_get_height() start. */
enum obs_source_size_start {
	/* get_base_width() or get_base_height() of the source itself */
	OBS_SOURCE_SIZE_ENTRY_SELF,
	/* get_base_width() or get_base_height() of source->filters.array[0] */
	OBS_SOURCE_SIZE_ENTRY_FIRST_FILTER,
};

enum obs_source_size_ask obs_source_size_route(struct obs_source_size_node node);

/* type is source->info.type; has_filters is source->filters.num != 0, read
 * under filter_mutex. */
enum obs_source_size_start obs_source_size_entry(int type, bool has_filters);
