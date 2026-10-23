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

#include "libobs/build/libobs-ported-source-color-space.pch.h"

/* The colour space query routing of obs_source_get_color_space() in
 * obs-source.c, moved: the decisions, on plain values.
 * obs_source_get_color_space() keeps data_valid(), reads the source's fields,
 * follows the route (the recursion, convert_video_space() and the callback)
 * and returns what comes back. Declared in obs-source-color-space.h without
 * EXPORT. In hybrid libobs the Rust port (rust/obs-source) replaces this
 * file. */

enum obs_source_color_space_ask obs_source_color_space_route(struct obs_source_color_space_node node)
{
	const bool is_filter = node.type == OBS_SOURCE_COLOR_SPACE_TYPE_FILTER;
	const bool has_video = (node.output_flags & OBS_SOURCE_COLOR_SPACE_VIDEO) != 0;

	if (!is_filter && !has_video) {
		if (node.has_filter_parent)
			return OBS_SOURCE_COLOR_SPACE_ASK_PARENT;
	}

	/* a filter that produces no video must not answer for the chain below it */
	if (is_filter && !has_video) {
		if (node.has_filter_target)
			return OBS_SOURCE_COLOR_SPACE_ASK_TARGET;
	}

	if (!node.has_data || !node.enabled) {
		if (node.has_filter_target)
			return OBS_SOURCE_COLOR_SPACE_ASK_TARGET;
	}

	if (node.output_flags & OBS_SOURCE_COLOR_SPACE_ASYNC)
		return OBS_SOURCE_COLOR_SPACE_ASYNC_PICK;

	return OBS_SOURCE_COLOR_SPACE_CALLBACK;
}

int obs_source_color_space_pick(int video_space, size_t count, const int *preferred_spaces)
{
	int space = video_space;
	for (size_t i = 0; i < count; ++i) {
		space = preferred_spaces[i];
		if (space == video_space)
			break;
	}

	return space;
}
