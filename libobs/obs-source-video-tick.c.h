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

#include "libobs/build/libobs-ported-source-video-tick.pch.h"

/* The per-frame decisions of obs-source.c, moved from
 * obs_source_video_tick() and async_tick(): the decisions, on plain values.
 * obs-source.c keeps every read of the source's fields, the three atomic
 * loads and every call where they were, passes the values in and acts on
 * what comes back. Declared in obs-source-video-tick.h without EXPORT. In
 * hybrid libobs the Rust port (rust/obs-source, video_tick) replaces this
 * file. */

bool obs_source_video_tick_transition(int type)
{
	return type == OBS_SOURCE_VIDEO_TICK_TYPE_TRANSITION;
}

bool obs_source_video_tick_async(uint32_t output_flags)
{
	return (output_flags & OBS_SOURCE_VIDEO_TICK_ASYNC) != 0;
}

bool obs_source_video_tick_media(uint32_t output_flags)
{
	return (output_flags & OBS_SOURCE_VIDEO_TICK_CONTROLLABLE_MEDIA) != 0;
}

bool obs_source_video_tick_deferred_update(long defer_update_count)
{
	return defer_update_count > 0;
}

enum obs_source_video_tick_edge obs_source_video_tick_edge(long refs, bool was)
{
	bool now = refs != 0;
	if (now == was)
		return OBS_SOURCE_VIDEO_TICK_EDGE_NONE;

	return now ? OBS_SOURCE_VIDEO_TICK_EDGE_ON : OBS_SOURCE_VIDEO_TICK_EDGE_OFF;
}

bool obs_source_video_tick_calls_tick(bool has_data, bool has_callback)
{
	return has_data && has_callback;
}

enum obs_source_video_tick_async_route obs_source_video_tick_async_route(bool deinterlacing)
{
	return deinterlacing ? OBS_SOURCE_VIDEO_TICK_ASYNC_DEINTERLACE : OBS_SOURCE_VIDEO_TICK_ASYNC_CLOSEST;
}
