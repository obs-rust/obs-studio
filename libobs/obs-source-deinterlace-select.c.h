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

#include "libobs/build/libobs-ported-source-deinterlace-select.pch.h"

/* The deinterlace frame selection of obs-source-deinterlace.c, moved from
 * first_frame(), ready_deinterlace_frames(),
 * deinterlace_get_closest_frames() and deinterlace_update_async_video():
 * the decisions, on the queue's timestamps and the source's values.
 * obs-source-deinterlace.c copies the timestamps, reads the fields, calls
 * these and applies the results: it keeps the async_frames queue,
 * da_erase(), remove_async_frame(), the prev_frame writes, the frame
 * pointers, the textures and async_mutex. Declared in
 * obs-source-deinterlace-select.h without EXPORT. In hybrid libobs the Rust
 * port (rust/obs-source, deinterlace_select) replaces this file. */

/* frame_out_of_bounds() of obs-internal.h, on values. */
static inline bool deinterlace_select_out_of_bounds(uint64_t last_frame_ts, uint64_t ts)
{
	if (ts < last_frame_ts)
		return ((last_frame_ts - ts) > OBS_SOURCE_DEINTERLACE_SELECT_MAX_TS_VAR);
	else
		return ((ts - last_frame_ts) > OBS_SOURCE_DEINTERLACE_SELECT_MAX_TS_VAR);
}

struct obs_source_deinterlace_select_first_result obs_source_deinterlace_select_first(uint64_t last_frame_ts,
										      size_t count)
{
	struct obs_source_deinterlace_select_first_result result = {false, false};

	if (last_frame_ts)
		return result;

	result.first = true;
	if (count >= 2)
		result.flag_front = true;
	return result;
}

/* ready_deinterlace_frames(). The queue is timestamps[first] to
 * timestamps[count - 1]: async_frames.array[0] is timestamps[first], and
 * each da_erase(source->async_frames, 0) of the C before is first++. Every
 * erase was at index 0, and remove_async_frame() of the frame it erased
 * followed it (the buffered walk's prev_frame was always the front frame),
 * so obs-source-deinterlace.c erases result.drop_front frames from the front
 * in the same order. frame and prev_frame are whether the walk had taken
 * them; the frame prev_frame ends on is the front one. */
struct obs_source_deinterlace_select_walk_result
obs_source_deinterlace_select_walk(const uint64_t *timestamps, size_t count, uint64_t sys_time, uint64_t last_frame_ts,
				   uint64_t last_sys_timestamp, bool async_unbuffered)
{
	struct obs_source_deinterlace_select_walk_result result = {0, last_frame_ts, false, false, false, false};

	if (!count)
		return result;

	size_t first = 0;
	size_t num = count;
	uint64_t next_frame = timestamps[0];
	bool prev_frame = false;
	bool frame = false;
	uint64_t sys_offset = sys_time - last_sys_timestamp;
	uint64_t frame_time = next_frame;
	uint64_t frame_offset = 0;
	size_t idx = 1;

	if (async_unbuffered) {
		while (num > 2) {
			first++;
			num--;
			next_frame = timestamps[first];
		}

		result.drop_front = first;
		result.pairs = num == 2;
		result.last_frame_ts = next_frame;
		result.ready = true;
		return result;
	}

	/* account for timestamp invalidation */
	if (deinterlace_select_out_of_bounds(result.last_frame_ts, frame_time)) {
		result.last_frame_ts = next_frame;
		result.offset_reset = true;
		result.ready = true;
		return result;
	} else {
		frame_offset = frame_time - result.last_frame_ts;
		result.last_frame_ts += sys_offset;
	}

	while (result.last_frame_ts > next_frame) {

		/* this tries to reduce the needless frame duplication, also
		 * helps smooth out async rendering to frame boundaries.  In
		 * other words, tries to keep the framerate as smooth as
		 * possible */
		if ((result.last_frame_ts - next_frame) < OBS_SOURCE_DEINTERLACE_PACING_FRAME_TOLERANCE)
			break;

		if (prev_frame) {
			first++;
			num--;
		}

		if (num <= 2) {
			bool exit = true;

			/* Two frames or fewer left: stop and show the front one
			 * alone, unless none was taken yet. A prev_frame was just
			 * erased and has gone back to the cache, so it is no
			 * pair for the front frame and is not flagged (#2009). */
			if (!frame && num == 2)
				exit = false;

			if (exit) {
				result.drop_front = first;
				result.offset_reset = true;
				result.ready = true;
				return result;
			}
		}

		if (frame)
			idx = 2;
		else
			idx = 1;

		prev_frame = frame;
		frame = true;
		next_frame = timestamps[first + idx];

		/* more timestamp checking and compensating */
		if ((next_frame - frame_time) > OBS_SOURCE_DEINTERLACE_SELECT_MAX_TS_VAR) {
			result.last_frame_ts = next_frame - frame_offset;
			result.offset_reset = true;
		}

		frame_time = next_frame;
		frame_offset = frame_time - result.last_frame_ts;
	}

	result.drop_front = first;
	result.flag_front = prev_frame;
	result.ready = frame;
	return result;
}

struct obs_source_deinterlace_select_pair_result obs_source_deinterlace_select_unbuffered_pair(uint64_t timestamp,
											       uint64_t next_timestamp,
											       uint64_t offset,
											       uint64_t sys_time)
{
	struct obs_source_deinterlace_select_pair_result result = {true, false, 0};

	if (offset) {
		if (obs_source_deinterlace_pacing_wait_for_next(timestamp, next_timestamp, offset, sys_time)) {
			// Don't skip ahead prematurely.
			result.prev_frame = false;
			result.set_frame_ts = true;
			result.frame_ts = obs_source_deinterlace_pacing_waiting_frame_ts(timestamp, next_timestamp);
		}
	}
	return result;
}

bool obs_source_deinterlace_select_take_reads_flag(size_t remaining)
{
	return remaining > 0;
}

bool obs_source_deinterlace_select_take_pair(size_t remaining, bool prev_frame)
{
	return (remaining > 0) && prev_frame;
}

bool obs_source_deinterlace_select_take_init_last(uint64_t last_frame_ts)
{
	return !last_frame_ts;
}

enum obs_source_deinterlace_select_offset_kind obs_source_deinterlace_select_take_offset(bool async_unbuffered)
{
	if (!async_unbuffered)
		return OBS_SOURCE_DEINTERLACE_SELECT_OFFSET_BUFFERED;
	return OBS_SOURCE_DEINTERLACE_SELECT_OFFSET_REANCHOR;
}

enum obs_source_deinterlace_select_update_step obs_source_deinterlace_select_update(bool has_prev, bool updated)
{
	if (has_prev)
		return OBS_SOURCE_DEINTERLACE_SELECT_UPDATE_UPLOAD;
	else if (updated) /* swap cur/prev if no previous texture */
		return OBS_SOURCE_DEINTERLACE_SELECT_UPDATE_SWAP;
	return OBS_SOURCE_DEINTERLACE_SELECT_UPDATE_NONE;
}
