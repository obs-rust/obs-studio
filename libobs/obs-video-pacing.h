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

/* The frame pacing decisions of video_sleep() in obs-video.c, on plain values
 * (obs-video-pacing.c). The graphics thread calls video_sleep() once per frame,
 * after the frame is rendered:
 * - obs_video_pacing_step(): the frame time step, from the current frame time,
 *   the frame interval, whether os_sleepto_ns() reached the next frame time,
 *   and the time after a late wake-up. It gives the new frame time and the
 *   number of frames the step covers (the frames lagged are one less);
 * - obs_video_pacing_group_starts(): whether the group of a ready encoder gets
 *   its start timestamp.
 *
 * obs-video.c keeps os_sleepto_ns() and os_gettime_ns() (it passes their
 * results), obs_video_stats_add_frames(), the ready encoder groups with their
 * weak references and mutexes, and the vframe_info pushes into the mixes'
 * deques with the mix list and its mutex. Nothing here takes a lock, calls
 * into libobs or keeps state.
 *
 * Hidden in libobs: not EXPORT. */

/* The frame time step of video_sleep(). */
struct obs_video_pacing_step {
	/* The new video->video_time: the next frame time, or after a late
	 * wake-up the frame time count intervals on. */
	uint64_t frame_time;
	/* The frames this step covers: 1 when the sleep reached the next frame
	 * time, else the whole intervals since frame_time, at least 1. The
	 * frames lagged are count - 1. */
	int count;
};

/* video_sleep(): frame_time is video->video_time, interval_ns the frame
 * interval. slept is the result of os_sleepto_ns(frame_time + interval_ns);
 * now is os_gettime_ns() after it, read only when slept is false (otherwise
 * not used). */
struct obs_video_pacing_step obs_video_pacing_step(uint64_t frame_time, uint64_t interval_ns, bool slept, uint64_t now);

/* video_sleep(): a ready encoder's group gets its start timestamp (the new
 * frame time) once every encoder in it has started and it has none yet. */
bool obs_video_pacing_group_starts(uint32_t num_encoders_started, size_t num_encoders, uint64_t start_timestamp);
