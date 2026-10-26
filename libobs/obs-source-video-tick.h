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
#include <stdint.h>

/* The per-frame decisions of obs_source_video_tick() and async_tick() in
 * obs-source.c, on plain values (obs-source-video-tick.c):
 * - which steps run for the source's type and flags: the transition tick,
 *   the async tick and the media actions, each asked for at its own point;
 * - whether a deferred update is due;
 * - the show/hide edge and the activate/deactivate edge: whether the loaded
 *   reference count changed the source's showing or active flag, and which
 *   way;
 * - whether the plugin's video_tick callback is called;
 * - in async_tick(): whether the source deinterlaces its last frame or takes
 *   the closest frame, and whether it filters the previous frame too.
 *
 * obs-source.c keeps every read and call where it was:
 * os_atomic_load_long() of defer_update_count, show_refs and activate_refs
 * (unlocked: the show refs are loaded before the show edge runs, the
 * activate refs only after it, since a show callback can change them),
 * obs_transition_tick(), async_tick(), process_media_actions(),
 * obs_source_deferred_update(), gs_texrender_reset(), show_source(),
 * hide_source(), activate_source(), deactivate_source(), the filter walks in
 * reverse, the stores of showing and active, the plugin callback, the
 * per-tick resets, async_mutex, deinterlacing_enabled() (called twice, as
 * before), the frame calls and set_async_texture_size(). It passes the
 * values in and acts on what comes back. None of these functions takes a
 * lock, calls into libobs or keeps state.
 *
 * The source type crosses as int and the counts as the C's long, so this
 * header needs no other libobs header; obs-source.c checks that the
 * constants below match obs-source.h and that enum obs_source_type is
 * int-sized.
 *
 * Hidden in libobs: not EXPORT. */

/* OBS_SOURCE_TYPE_TRANSITION (enum obs_source_type, obs-source.h). */
#define OBS_SOURCE_VIDEO_TICK_TYPE_TRANSITION 2
/* OBS_SOURCE_ASYNC and OBS_SOURCE_CONTROLLABLE_MEDIA (obs-source.h). */
#define OBS_SOURCE_VIDEO_TICK_ASYNC (1u << 2)
#define OBS_SOURCE_VIDEO_TICK_CONTROLLABLE_MEDIA (1u << 13)

/* How a reference count moved a source's showing or active flag. */
enum obs_source_video_tick_edge {
	/* the flag is the same: nothing is called */
	OBS_SOURCE_VIDEO_TICK_EDGE_NONE,
	/* the count is not 0 and the flag was false: show or activate */
	OBS_SOURCE_VIDEO_TICK_EDGE_ON,
	/* the count is 0 and the flag was true: hide or deactivate */
	OBS_SOURCE_VIDEO_TICK_EDGE_OFF,
};

/* What async_tick() does with the source's frames. */
enum obs_source_video_tick_async_route {
	/* drop the current frame and take get_closest_frame() */
	OBS_SOURCE_VIDEO_TICK_ASYNC_CLOSEST,
	/* deinterlace_process_last_frame(); then the previous frame is filtered
	 * too */
	OBS_SOURCE_VIDEO_TICK_ASYNC_DEINTERLACE,
};

/* Whether obs_transition_tick() runs: a type of
 * OBS_SOURCE_TYPE_TRANSITION. */
bool obs_source_video_tick_transition(int type);

/* Whether async_tick() runs: OBS_SOURCE_ASYNC in the output flags. */
bool obs_source_video_tick_async(uint32_t output_flags);

/* Whether process_media_actions() runs: OBS_SOURCE_CONTROLLABLE_MEDIA in the
 * output flags. */
bool obs_source_video_tick_media(uint32_t output_flags);

/* Whether obs_source_deferred_update() runs: a loaded defer_update_count
 * above 0 (a negative count does not). */
bool obs_source_video_tick_deferred_update(long defer_update_count);

/* The edge of a loaded reference count (show_refs or activate_refs) against
 * the flag it sets (showing or active): the flag is now refs != 0; ON or OFF
 * when that differs from was, NONE when it does not. */
enum obs_source_video_tick_edge obs_source_video_tick_edge(long refs, bool was);

/* Whether the plugin's video_tick callback is called: the source has data
 * and a callback. */
bool obs_source_video_tick_calls_tick(bool has_data, bool has_callback);

/* The route of async_tick() for deinterlacing_enabled(): DEINTERLACE when
 * it is, CLOSEST when it is not. async_tick() asks again with its second
 * deinterlacing_enabled() for the previous frame's filter. */
enum obs_source_video_tick_async_route obs_source_video_tick_async_route(bool deinterlacing);
