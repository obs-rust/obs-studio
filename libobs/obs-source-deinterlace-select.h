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

/* The deinterlace frame selection of obs-source-deinterlace.c, on plain
 * values (obs-source-deinterlace-select.c): which queued async frames a
 * deinterlaced source drops, which it takes as its current and previous
 * frame, and what it does with the textures afterwards.
 *
 * - obs_source_deinterlace_select_first(): first_frame(), a source that has
 *   no last_frame_ts yet takes its front frame, paired with the next when
 *   two or more are queued;
 * - obs_source_deinterlace_select_walk(): ready_deinterlace_frames() on the
 *   queue's timestamps: an unbuffered source drops all but two frames; a
 *   buffered one resets on a timestamp out of bounds, else walks the queue
 *   on its own clock, dropping a frame pair by pair until the clock is
 *   within FRAME_TOLERANCE of the next frame, stops with two frames or fewer
 *   left (#2009), and re-anchors the clock on a timestamp jump;
 * - obs_source_deinterlace_select_unbuffered_pair(): the "don't skip ahead"
 *   guard of an unbuffered source left with two frames: its front frame
 *   pairs with the next one unless the source waits for it;
 * - obs_source_deinterlace_select_take_*(): deinterlace_get_closest_frames()
 *   after a frame is ready: whether the taken frame's prev_frame flag is
 *   read, whether it takes a pair, whether last_frame_ts starts there, and
 *   how deinterlace_offset is set;
 * - obs_source_deinterlace_select_update(): deinterlace_update_async_video(),
 *   upload the previous frame, swap the textures, or neither.
 *
 * obs-source-deinterlace.c keeps async_mutex (async_tick() holds it around
 * the selection), the async_frames queue and its da_erase(),
 * remove_async_frame() and the cache, every prev_frame write on a frame,
 * the frame pointers, set_async_texture_size(), update_async_textures(),
 * the texture swaps and obs->video. It copies the queued timestamps before
 * the walk, reads each of the source's fields where it did before, passes
 * the values in and applies what comes back: the dropped frames off the
 * front, each erased and then released, in the walk's order; then
 * last_frame_ts, deinterlace_offset, deinterlace_frame_ts and the front
 * frame's prev_frame. None of these functions takes a lock, calls into
 * libobs, touches memory other than the timestamps it is handed, or keeps
 * state. All arithmetic is uint64_t and wraps, as in
 * obs-source-deinterlace.c; nothing divides.
 *
 * Hidden in libobs: not EXPORT. */

/* in nanoseconds; obs-source-deinterlace.c checks that it is
 * obs-internal.h's MAX_TS_VAR */
/* A timestamp more than this away from the source clock is out of bounds,
 * and a step of more than this between queued frames re-anchors it. */
#define OBS_SOURCE_DEINTERLACE_SELECT_MAX_TS_VAR 2000000000ULL

/* What first_frame() does. */
struct obs_source_deinterlace_select_first_result {
	/* no last_frame_ts yet: take the front frame without the walk */
	bool first;
	/* set prev_frame on the front frame (first, and two or more queued) */
	bool flag_front;
};

/* first_frame() of a source whose last_frame_ts is last_frame_ts, with
 * count frames queued. */
struct obs_source_deinterlace_select_first_result obs_source_deinterlace_select_first(uint64_t last_frame_ts,
										      size_t count);

/* What ready_deinterlace_frames() does with the queue. drop_front is at
 * most count - 1 (unbuffered: count - 2 from three frames on). */
struct obs_source_deinterlace_select_walk_result {
	/* frames to erase from the front, each with remove_async_frame() */
	size_t drop_front;
	/* the new source->last_frame_ts, stored after the drops */
	uint64_t last_frame_ts;
	/* then set source->deinterlace_offset to 0 */
	bool offset_reset;
	/* unbuffered, two frames left: the front frame's prev_frame and
	 * deinterlace_frame_ts come from
	 * obs_source_deinterlace_select_unbuffered_pair() */
	bool pairs;
	/* buffered: set prev_frame on the front frame, last */
	bool flag_front;
	/* ready_deinterlace_frames() returns true: take the front frame */
	bool ready;
};

/* ready_deinterlace_frames() over a queue of count frames whose timestamps,
 * oldest first, are timestamps[0] to timestamps[count - 1] (timestamps may
 * be NULL when count is 0: nothing is dropped or ready, last_frame_ts is
 * kept), at the canvas time sys_time, from the source's last_frame_ts
 * (read only for a buffered source; ignored when unbuffered),
 * last_sys_timestamp and async_unbuffered. */
struct obs_source_deinterlace_select_walk_result
obs_source_deinterlace_select_walk(const uint64_t *timestamps, size_t count, uint64_t sys_time, uint64_t last_frame_ts,
				   uint64_t last_sys_timestamp, bool async_unbuffered);

/* What an unbuffered source left with two frames does with the front one. */
struct obs_source_deinterlace_select_pair_result {
	/* the front frame's prev_frame: it pairs with the next frame */
	bool prev_frame;
	/* then set source->deinterlace_frame_ts to frame_ts (before the flag) */
	bool set_frame_ts;
	uint64_t frame_ts;
};

/* The two frames of an unbuffered source are at timestamp and
 * next_timestamp; offset is its deinterlace_offset, or 0 when the source
 * reads async_unbuffered as false again. With an offset, the source waits
 * while sys_time is before timestamp + offset + the frame's duration
 * (obs_source_deinterlace_pacing_wait_for_next()): the front frame is shown
 * alone, from obs_source_deinterlace_pacing_waiting_frame_ts(). Otherwise it
 * pairs. */
struct obs_source_deinterlace_select_pair_result obs_source_deinterlace_select_unbuffered_pair(uint64_t timestamp,
											       uint64_t next_timestamp,
											       uint64_t offset,
											       uint64_t sys_time);

/* remaining > 0: with frames left after the taken one, its prev_frame flag
 * is read. */
bool obs_source_deinterlace_select_take_reads_flag(size_t remaining);

/* remaining > 0 && prev_frame: the taken frame becomes the previous frame
 * and the next one is taken as the current one. */
bool obs_source_deinterlace_select_take_pair(size_t remaining, bool prev_frame);

/* !last_frame_ts: last_frame_ts starts at the current frame's timestamp. */
bool obs_source_deinterlace_select_take_init_last(uint64_t last_frame_ts);

/* How deinterlace_get_closest_frames() sets deinterlace_offset. */
enum obs_source_deinterlace_select_offset_kind {
	/* obs_source_deinterlace_pacing_buffered_offset() */
	OBS_SOURCE_DEINTERLACE_SELECT_OFFSET_BUFFERED = 0,
	/* obs_source_deinterlace_pacing_reanchor() */
	OBS_SOURCE_DEINTERLACE_SELECT_OFFSET_REANCHOR = 1,
};

/* BUFFERED unless async_unbuffered, then REANCHOR. */
enum obs_source_deinterlace_select_offset_kind obs_source_deinterlace_select_take_offset(bool async_unbuffered);

/* What deinterlace_update_async_video() does with the textures. */
enum obs_source_deinterlace_select_update_step {
	/* nothing */
	OBS_SOURCE_DEINTERLACE_SELECT_UPDATE_NONE = 0,
	/* upload the previous frame into the previous textures */
	OBS_SOURCE_DEINTERLACE_SELECT_UPDATE_UPLOAD = 1,
	/* swap the current and previous textures */
	OBS_SOURCE_DEINTERLACE_SELECT_UPDATE_SWAP = 2,
};

/* UPLOAD with a previous frame; else SWAP when the current frame was
 * updated; else NONE. */
enum obs_source_deinterlace_select_update_step obs_source_deinterlace_select_update(bool has_prev, bool updated);
