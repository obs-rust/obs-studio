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

#include "libobs/build/libobs-ported-source-size.pch.h"

/* The size query routing of obs_source_get_width() and
 * obs_source_get_height() in obs-source.c, moved: the decisions of
 * get_async_width(), get_base_width(), get_recurse_width() and
 * obs_source_get_width() (and their height twins), on plain values.
 * obs-source.c keeps data_valid(), filter_mutex, the recursion, the callbacks
 * and the field reads. Declared in obs-source-size.h without EXPORT. In
 * hybrid libobs the Rust port (rust/obs-source) replaces this file. */

enum obs_source_size_ask obs_source_size_route(struct obs_source_size_node node)
{
	const bool is_filter = node.has_filter_parent;
	const bool func_valid = node.has_data && node.has_callback;

	if (node.type == OBS_SOURCE_SIZE_TYPE_TRANSITION) {
		return node.enabled ? OBS_SOURCE_SIZE_TRANSITION : OBS_SOURCE_SIZE_ZERO;

	} else if (func_valid && (!is_filter || node.enabled)) {
		return OBS_SOURCE_SIZE_CALLBACK;

	} else if (is_filter) {
		return OBS_SOURCE_SIZE_ASK_TARGET;
	}

	if (!node.async_active)
		return OBS_SOURCE_SIZE_ZERO;

	return ((node.async_rotation % 180) == 0) ? OBS_SOURCE_SIZE_ASYNC : OBS_SOURCE_SIZE_ASYNC_SWAPPED;
}

enum obs_source_size_start obs_source_size_entry(int type, bool has_filters)
{
	if (type == OBS_SOURCE_SIZE_TYPE_FILTER)
		return OBS_SOURCE_SIZE_ENTRY_SELF;

	return has_filters ? OBS_SOURCE_SIZE_ENTRY_FIRST_FILTER : OBS_SOURCE_SIZE_ENTRY_SELF;
}
