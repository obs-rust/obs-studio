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

/* The 2x deinterlace frame pacing of obs-source-deinterlace.c, on plain
 * values (obs-source-deinterlace-pacing.c). With a double-rate mode (Yadif
 * 2x, Linear 2x, Discard 2x) each async frame is shown as two fields: the
 * first from the tick that picks the frame, the second from
 * deinterlace_frame_ts + deinterlace_offset + deinterlace_half_duration -
 * TWOX_TOLERANCE on the canvas clock. These functions decide those three
 * values and when an unbuffered source moves on to the next frame:
 * - obs_source_deinterlace_pacing_hold(): the start of
 *   deinterlace_get_closest_frames(), for an unbuffered source with an
 *   offset: keep the current frame while its end is ahead, unless the clock
 *   jumped;
 * - obs_source_deinterlace_pacing_wait_for_next() and
 *   obs_source_deinterlace_pacing_waiting_frame_ts(): the "don't skip
 *   ahead" guard of ready_deinterlace_frames(), for an unbuffered source
 *   with two frames queued and an offset;
 * - obs_source_deinterlace_pacing_half_duration(): half a frame's duration,
 *   from the frame before it, or the one before or half a canvas interval
 *   when there is no usable frame before it (#1965);
 * - obs_source_deinterlace_pacing_buffered_offset(): the offset of a
 *   buffered source (#395);
 * - obs_source_deinterlace_pacing_reanchor(): the offset of an unbuffered
 *   source (#395).
 *
 * obs-source-deinterlace.c reads the source's fields and obs->video, passes
 * the values in and stores what comes back, in the order it did before; it
 * keeps the async_frames queue, the buffered walk, first_frame(), the
 * textures and the render. None of these functions takes a lock, calls into
 * libobs or keeps state. All arithmetic is uint64_t and wraps, as in
 * obs-source-deinterlace.c.
 *
 * Hidden in libobs: not EXPORT. */

/* in nanoseconds; obs-source-deinterlace.c checks that they are its
 * FRAME_TOLERANCE and TWOX_TOLERANCE */
/* A frame is due once the source clock is this far past its timestamp. */
#define OBS_SOURCE_DEINTERLACE_PACING_FRAME_TOLERANCE 2000000ULL
/* The second field is shown this much early. */
#define OBS_SOURCE_DEINTERLACE_PACING_TWOX_TOLERANCE 1000000ULL
/* A frame end this far ahead of the clock or more means the clock jumped. */
#define OBS_SOURCE_DEINTERLACE_PACING_TS_JUMP_THRESHOLD 70000000ULL

/* Whether deinterlace_get_closest_frames() keeps the current frame of an
 * unbuffered source whose deinterlace_offset is set: true while sys_time is
 * before the frame's end (frame_ts + offset + 2 * half_duration -
 * TWOX_TOLERANCE) by less than TS_JUMP_THRESHOLD. */
bool obs_source_deinterlace_pacing_hold(uint64_t frame_ts, uint64_t offset, uint32_t half_duration, uint64_t sys_time);

/* Whether ready_deinterlace_frames() waits with an unbuffered source's two
 * queued frames (timestamp, then next_timestamp) whose deinterlace_offset is
 * set: true while sys_time is before timestamp + offset + the frame's
 * duration. Then the first frame is shown alone (no prev_frame) and
 * deinterlace_frame_ts becomes obs_source_deinterlace_pacing_waiting_frame_ts(). */
bool obs_source_deinterlace_pacing_wait_for_next(uint64_t timestamp, uint64_t next_timestamp, uint64_t offset,
						 uint64_t sys_time);
/* timestamp minus the frame's duration (next_timestamp - timestamp). */
uint64_t obs_source_deinterlace_pacing_waiting_frame_ts(uint64_t timestamp, uint64_t next_timestamp);

/* deinterlace_half_duration of a frame at timestamp after one at prev_ts:
 * half the difference. With no usable previous timestamp, that is prev_ts 0
 * (a source that never showed a frame), timestamp at or before prev_ts (a
 * timestamp going backwards) or more than twice TS_JUMP_THRESHOLD after it,
 * a half duration past TS_JUMP_THRESHOLD (a pause or a new clock):
 * prev_half, the deinterlace_half_duration before,
 * when it is not 0 and at most TS_JUMP_THRESHOLD, else half of
 * frame_interval_ns, the canvas interval, truncated to 32 bits (#1965). */
uint32_t obs_source_deinterlace_pacing_half_duration(uint64_t timestamp, uint64_t prev_ts, uint32_t prev_half,
						     uint64_t frame_interval_ns);

/* deinterlace_offset of a buffered source: video_time - last_frame_ts +
 * FRAME_TOLERANCE + TWOX_TOLERANCE. */
uint64_t obs_source_deinterlace_pacing_buffered_offset(uint64_t video_time, uint64_t last_frame_ts);

/* deinterlace_offset of an unbuffered source after it picked the frame at
 * frame_ts, from its offset before (0 for none): video_time - frame_ts when
 * there was none, when that is less than the offset (the frame came early),
 * or when it is at least the larger of the half duration and the canvas
 * interval more, less TWOX_TOLERANCE (the frame missed its tick and its first
 * field); the offset before otherwise. */
uint64_t obs_source_deinterlace_pacing_reanchor(uint64_t offset, uint64_t video_time, uint64_t frame_ts,
						uint32_t half_duration, uint64_t frame_interval_ns);
