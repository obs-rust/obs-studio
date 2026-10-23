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

#include "libobs/build/libobs-ported-source-deinterlace-pacing.pch.h"

/* The 2x deinterlace frame pacing of obs-source-deinterlace.c, moved from
 * deinterlace_get_closest_frames() and ready_deinterlace_frames(): the
 * decisions, on plain values. obs-source-deinterlace.c reads the source's
 * fields and obs->video, calls these and stores the results; it keeps the
 * async_frames queue, the buffered walk, first_frame(), the textures and the
 * render. Declared in obs-source-deinterlace-pacing.h without EXPORT. In hybrid
 * libobs the Rust port (rust/obs-source) replaces this file. */

bool obs_source_deinterlace_pacing_hold(uint64_t frame_ts, uint64_t offset, uint32_t half_duration, uint64_t sys_time)
{
	// Want to keep frame if it has not elapsed.
	const uint64_t frame_end =
		frame_ts + offset + ((uint64_t)half_duration * 2) - OBS_SOURCE_DEINTERLACE_PACING_TWOX_TOLERANCE;
	if (sys_time < frame_end) {
		// Process new frames if we think time jumped.
		const uint64_t diff = frame_end - sys_time;
		if (diff < OBS_SOURCE_DEINTERLACE_PACING_TS_JUMP_THRESHOLD) {
			return true;
		}
	}
	return false;
}

bool obs_source_deinterlace_pacing_wait_for_next(uint64_t timestamp, uint64_t next_timestamp, uint64_t offset,
						 uint64_t sys_time)
{
	const uint64_t duration = next_timestamp - timestamp;
	const uint64_t frame_end = timestamp + offset + duration;
	// Don't skip ahead prematurely.
	return sys_time < frame_end;
}

uint64_t obs_source_deinterlace_pacing_waiting_frame_ts(uint64_t timestamp, uint64_t next_timestamp)
{
	const uint64_t duration = next_timestamp - timestamp;
	return timestamp - duration;
}

uint32_t obs_source_deinterlace_pacing_half_duration(uint64_t timestamp, uint64_t prev_ts, uint32_t prev_half,
						     uint64_t frame_interval_ns)
{
	/* Half the time from the frame before, unless there is no usable
	 * previous timestamp: none (a source that never showed a frame), one
	 * at or after this frame (a timestamp going backwards), or one more
	 * than twice TS_JUMP_THRESHOLD before it, so a half duration past it
	 * (a pause, a new clock). Then the gap is no frame duration: half of
	 * it skipped the second field, or held the frame too long. Keep the
	 * half duration before when it is set and within the same limit, else
	 * take half a canvas interval, so the second field comes at the next
	 * tick. A regular stream of frames up to 140 ms apart (7.1 frames a
	 * second) keeps its half durations. */
	if (prev_ts && timestamp > prev_ts &&
	    timestamp - prev_ts <= 2 * OBS_SOURCE_DEINTERLACE_PACING_TS_JUMP_THRESHOLD)
		return (uint32_t)((timestamp - prev_ts) / 2);
	if (prev_half && prev_half <= OBS_SOURCE_DEINTERLACE_PACING_TS_JUMP_THRESHOLD)
		return prev_half;
	return (uint32_t)(frame_interval_ns / 2);
}

uint64_t obs_source_deinterlace_pacing_buffered_offset(uint64_t video_time, uint64_t last_frame_ts)
{
	/* Show the second field by the rule that picks frames: once the
	 * source clock (last_frame_ts) is FRAME_TOLERANCE past the field's
	 * time, so the offset maps source time to video time. Anchoring it at
	 * the tick that picked the frame put the second field up to a canvas
	 * interval late, after the next frame when the field rate does not
	 * match the canvas (50i into 60 fps skipped a field in four). */
	return video_time - last_frame_ts + OBS_SOURCE_DEINTERLACE_PACING_FRAME_TOLERANCE +
	       OBS_SOURCE_DEINTERLACE_PACING_TWOX_TOLERANCE;
}

uint64_t obs_source_deinterlace_pacing_reanchor(uint64_t offset, uint64_t video_time, uint64_t frame_ts,
						uint32_t half_duration, uint64_t frame_interval_ns)
{
	/* Unbuffered frames are picked at the first tick after they arrive
	 * and the previous frame ends, so the tick lags the frame by anything
	 * up to a canvas interval. Re-anchoring the offset at every pick held
	 * each frame for its duration rounded up to whole ticks, so a 50i
	 * source fell behind a 60 fps canvas and had frames dropped. Keep the
	 * offset unless the frame comes earlier than it, or missed its tick
	 * and its first field (a late delivery, a new clock). */
	const uint64_t field_or_tick = half_duration > frame_interval_ns ? half_duration : frame_interval_ns;
	const uint64_t new_offset = video_time - frame_ts;
	const int64_t late = (int64_t)(new_offset - offset);

	if (!offset || late < 0 || (uint64_t)late + OBS_SOURCE_DEINTERLACE_PACING_TWOX_TOLERANCE >= field_or_tick)
		return new_offset;
	return offset;
}
