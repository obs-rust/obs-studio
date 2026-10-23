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

/* The colour space query routing of obs_source_get_color_space() in
 * obs-source.c, on plain values (obs-source-color-space.c):
 * - obs_source_color_space_route(): which source answers. A non-filter
 *   source with no video asks its filter parent; a filter with no video
 *   asks its target (#1493: it must not answer for the chain below it); a
 *   source with no data, or a disabled one, asks its target; an async
 *   source picks from the preferred spaces; any other source asks its
 *   video_get_color_space callback.
 * - obs_source_color_space_pick(): the async pick. The first preferred
 *   space equal to the frame's space, else the last preferred space, else
 *   (no preferred spaces) the frame's space.
 *
 * obs_source_get_color_space() keeps data_valid(), reads the source's
 * fields into struct obs_source_color_space_node, follows the route
 * (the recursion into the parent or the target, convert_video_space() of
 * the async frame's format and transfer, and the callback, or GS_CS_SRGB
 * without one) and returns what comes back. Neither function takes a lock,
 * calls into libobs or keeps state, and neither follows a source pointer.
 *
 * Colour spaces are enum gs_color_space values passed as int, so this
 * header needs no graphics header; obs-source.c checks the sizes.
 *
 * Hidden in libobs: not EXPORT. */

/* obs-source.c checks that they are OBS_SOURCE_TYPE_FILTER, OBS_SOURCE_VIDEO
 * and OBS_SOURCE_ASYNC */
#define OBS_SOURCE_COLOR_SPACE_TYPE_FILTER 1
#define OBS_SOURCE_COLOR_SPACE_VIDEO (1u << 0)
#define OBS_SOURCE_COLOR_SPACE_ASYNC (1u << 2)

/* The fields of the source the route reads. */
struct obs_source_color_space_node {
	/* source->info.type, an enum obs_source_type */
	int type;
	/* source->info.output_flags */
	uint32_t output_flags;
	/* source->context.data != NULL */
	bool has_data;
	/* source->enabled */
	bool enabled;
	/* source->filter_parent != NULL */
	bool has_filter_parent;
	/* source->filter_target != NULL */
	bool has_filter_target;
};

/* Which source obs_source_get_color_space() asks. */
enum obs_source_color_space_ask {
	/* obs_source_get_color_space(source->filter_parent, ...) */
	OBS_SOURCE_COLOR_SPACE_ASK_PARENT,
	/* obs_source_get_color_space(source->filter_target, ...) */
	OBS_SOURCE_COLOR_SPACE_ASK_TARGET,
	/* obs_source_color_space_pick() against the async frame's space */
	OBS_SOURCE_COLOR_SPACE_ASYNC_PICK,
	/* info.video_get_color_space, or GS_CS_SRGB without one */
	OBS_SOURCE_COLOR_SPACE_CALLBACK,
};

enum obs_source_color_space_ask obs_source_color_space_route(struct obs_source_color_space_node node);

/* The async pick: count values from preferred_spaces (which may be NULL when
 * count is 0) against video_space, the async frame's space. */
int obs_source_color_space_pick(int video_space, size_t count, const int *preferred_spaces);
