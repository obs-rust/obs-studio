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

/* The frame timing decisions of the graphics thread loop in obs-video.c, on
 * plain values (obs-video-frame-timing.c). obs_graphics_thread_loop() runs
 * once per frame:
 * - obs_video_frame_timing_tick(): tick_sources(), the time since the last
 *   tick (on the first frame, one interval) and its seconds as a float, which
 *   every tick callback and every source's video_tick gets;
 * - obs_video_frame_timing_stats(): the end of obs_graphics_thread_loop(), the
 *   sums of the render time, the frame time and the frames, and once the sums
 *   cover at least one second, the fps and the average render time for
 *   obs_video_stats_set_timing(), and the reset sums.
 *
 * obs-video.c keeps os_gettime_ns() (it passes the times), the tick callbacks
 * and the source list with their mutexes, the profiler,
 * obs_video_stats_set_timing() and the stats mutex. Nothing here takes a
 * lock, calls into libobs or keeps state.
 *
 * Hidden in libobs: not EXPORT. */

/* The time tick_sources() gives the tick callbacks and the sources. */
struct obs_video_frame_tick {
	/* cur_time - last_time, with last_time one interval back on the first
	 * frame. */
	uint64_t delta_ns;
	/* delta_ns in seconds: (float)((double)delta_ns / 1e9). */
	float seconds;
};

/* The sums of struct obs_graphics_context, between two reports. */
struct obs_video_frame_sums {
	/* context->frame_time_total_ns: the render time of each frame. */
	uint64_t frame_time_total_ns;
	/* context->fps_total_ns: the frame time steps since the last tick. */
	uint64_t fps_total_ns;
	/* context->fps_total_frames. */
	uint32_t fps_total_frames;
};

/* What the end of obs_graphics_thread_loop() does with the sums. */
struct obs_video_frame_stats {
	/* The new context sums (all 0 after a report). */
	struct obs_video_frame_sums sums;
	/* The sums cover at least one second: report fps and avg_frame_time_ns
	 * through obs_video_stats_set_timing(). */
	bool report;
	/* The frames per second over the sums. */
	double fps;
	/* The average render time of a frame over the sums. */
	uint64_t avg_frame_time_ns;
};

/* tick_sources(): cur_time is the frame time (video->video_time), last_time
 * the time of the last tick (0 before the first frame), interval_ns the frame
 * interval. The caller keeps cur_time as the next last_time. */
struct obs_video_frame_tick obs_video_frame_timing_tick(uint64_t cur_time, uint64_t last_time, uint64_t interval_ns);

/* obs_graphics_thread_loop(): sums are the context's, frame_time_ns this
 * frame's render time, video_time the frame time after video_sleep(),
 * last_time the time of this frame's tick. */
struct obs_video_frame_stats obs_video_frame_timing_stats(struct obs_video_frame_sums sums, uint64_t frame_time_ns,
							  uint64_t video_time, uint64_t last_time);
