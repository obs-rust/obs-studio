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

/* The async video frame pacing of get_closest_frame() and
 * ready_async_frame() in obs-source.c, on plain values
 * (obs-source-async-pacing.c). Once a tick, async_tick() asks which queued
 * frame of a source without 2x deinterlacing to show:
 * - with no frame shown since the queue was reset (last_frame_ts 0), the
 *   oldest queued frame, whatever the buffering;
 * - unbuffered: the newest queued frame, dropping the older ones;
 * - buffered, after a timestamp jump (the oldest frame more than MAX_TS_VAR
 *   away from last_frame_ts, either way): the oldest frame, and
 *   last_frame_ts becomes its timestamp;
 * - buffered otherwise: last_frame_ts, the source clock, advances by the
 *   canvas time since the last tick (sys_time - last_sys_timestamp), and the
 *   walk drops the frames it has passed. It keeps the last frame it passed
 *   when the next one is due within FRAME_TOLERANCE (no needless
 *   duplicates), never drops the last queued frame, and re-anchors the
 *   source clock when the next frame's timestamp jumps more than MAX_TS_VAR
 *   ahead. With no frame passed it shows nothing new.
 * Then, if a frame is shown and last_frame_ts is 0, last_frame_ts becomes
 * that frame's timestamp.
 *
 * obs-source.c keeps the async_frames queue, the async cache, async_mutex
 * (held by async_tick()) and the frame pointers. It copies the queue's
 * timestamps, oldest first, calls obs_source_async_pacing(), erases
 * drop_front frames from the front (remove_async_frame() on each, as
 * before), stores last_frame_ts and takes the front frame when take_first
 * is set. The function takes no lock, does not call into libobs, keeps no
 * state and reads no further than count timestamps. All arithmetic is
 * uint64_t and wraps, as in obs-source.c.
 *
 * Hidden in libobs: not EXPORT. */

/* in nanoseconds; obs-source.c checks that MAX_TS_VAR is the same */
/* A timestamp this far from the source clock or more, plus 1 ns, is a jump. */
#define OBS_SOURCE_ASYNC_PACING_MAX_TS_VAR 2000000000ULL
/* The walk keeps a passed frame while the next is due within this. */
#define OBS_SOURCE_ASYNC_PACING_FRAME_TOLERANCE 2000000ULL

/* What get_closest_frame() does with the queue. drop_front is at most
 * count - 1: the last queued frame is never dropped. A frame is dropped
 * only when take_first is set. */
struct obs_source_async_pacing_result {
	/* frames to erase from the front, each with remove_async_frame() */
	size_t drop_front;
	/* the new source->last_frame_ts */
	uint64_t last_frame_ts;
	/* then take the front frame as cur_async_frame */
	bool take_first;
};

/* The pacing of a queue of count frames whose timestamps, oldest first, are
 * timestamps[0] to timestamps[count - 1] (timestamps may be NULL when count
 * is 0: nothing is dropped or taken), at the canvas time sys_time, from the
 * source's last_frame_ts, last_sys_timestamp and async_unbuffered. */
struct obs_source_async_pacing_result obs_source_async_pacing(const uint64_t *timestamps, size_t count,
							      uint64_t sys_time, uint64_t last_frame_ts,
							      uint64_t last_sys_timestamp, bool async_unbuffered);
