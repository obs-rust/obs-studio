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

#include "libobs/build/libobs-ported-source-async-pacing.pch.h"

/* The async video frame pacing of obs-source.c, moved from
 * get_closest_frame() and ready_async_frame(): the decisions, on the queue's
 * timestamps. obs-source.c copies the timestamps, calls this and applies the
 * result: it keeps the async_frames queue, da_erase(), remove_async_frame(),
 * the async cache, async_mutex and the frame pointers. Declared in
 * obs-source-async-pacing.h without EXPORT. In hybrid libobs the Rust port
 * (rust/obs-source) replaces this file. */

/* frame_out_of_bounds() of obs-internal.h, on values: that inline stays for
 * obs-source-deinterlace.c. */
static inline bool async_pacing_out_of_bounds(uint64_t last_frame_ts, uint64_t ts)
{
	if (ts < last_frame_ts)
		return ((last_frame_ts - ts) > OBS_SOURCE_ASYNC_PACING_MAX_TS_VAR);
	else
		return ((ts - last_frame_ts) > OBS_SOURCE_ASYNC_PACING_MAX_TS_VAR);
}

/* ready_async_frame(). The queue is timestamps[first] to
 * timestamps[count - 1]: async_frames.array[0] is timestamps[first], and
 * each da_erase(source->async_frames, 0) of the C before is first++. Every
 * erase was at index 0, the frame it erased was the front one, and
 * remove_async_frame() followed it, so obs-source.c erases
 * result->drop_front frames from the front in the same order.
 * remove_async_frame(source, NULL), where the walk had no frame yet, did
 * nothing (no cached frame is NULL). */
static bool async_pacing_ready(struct obs_source_async_pacing_result *result, const uint64_t *timestamps, size_t count,
			       uint64_t sys_time, uint64_t last_sys_timestamp, bool async_unbuffered)
{
	size_t first = 0;
	size_t num = count;
	uint64_t next_frame = timestamps[0];
	bool frame = false;
	uint64_t sys_offset = sys_time - last_sys_timestamp;
	uint64_t frame_time = next_frame;
	uint64_t frame_offset = 0;

	if (async_unbuffered) {
		while (num > 1) {
			first++;
			num--;
			next_frame = timestamps[first];
		}

		result->drop_front = first;
		result->last_frame_ts = next_frame;
		return true;
	}

	/* account for timestamp invalidation */
	if (async_pacing_out_of_bounds(result->last_frame_ts, frame_time)) {
		result->last_frame_ts = next_frame;
		return true;
	} else {
		frame_offset = frame_time - result->last_frame_ts;
		result->last_frame_ts += sys_offset;
	}

	while (result->last_frame_ts > next_frame) {

		/* this tries to reduce the needless frame duplication, also
		 * helps smooth out async rendering to frame boundaries.  In
		 * other words, tries to keep the framerate as smooth as
		 * possible */
		if (frame && (result->last_frame_ts - next_frame) < OBS_SOURCE_ASYNC_PACING_FRAME_TOLERANCE)
			break;

		if (frame) {
			first++;
			num--;
		}

		if (num == 1) {
			result->drop_front = first;
			return true;
		}

		frame = true;
		next_frame = timestamps[first + 1];

		/* more timestamp checking and compensating */
		if ((next_frame - frame_time) > OBS_SOURCE_ASYNC_PACING_MAX_TS_VAR) {
			result->last_frame_ts = next_frame - frame_offset;
		}

		frame_time = next_frame;
		frame_offset = frame_time - result->last_frame_ts;
	}

	result->drop_front = first;
	return frame;
}

/* get_closest_frame() */
struct obs_source_async_pacing_result obs_source_async_pacing(const uint64_t *timestamps, size_t count,
							      uint64_t sys_time, uint64_t last_frame_ts,
							      uint64_t last_sys_timestamp, bool async_unbuffered)
{
	struct obs_source_async_pacing_result result = {0, last_frame_ts, false};

	if (!count)
		return result;

	if (!last_frame_ts ||
	    async_pacing_ready(&result, timestamps, count, sys_time, last_sys_timestamp, async_unbuffered)) {
		result.take_first = true;

		if (!result.last_frame_ts)
			result.last_frame_ts = timestamps[result.drop_front];
	}

	return result;
}
