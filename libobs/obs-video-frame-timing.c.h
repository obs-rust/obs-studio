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

#include "libobs/build/libobs-ported-video-frame-timing.pch.h"

/* tick_sources(), without the callbacks and the sources. */
struct obs_video_frame_tick obs_video_frame_timing_tick(uint64_t cur_time, uint64_t last_time, uint64_t interval_ns)
{
	struct obs_video_frame_tick tick;

	if (!last_time)
		last_time = cur_time - interval_ns;

	tick.delta_ns = cur_time - last_time;
	tick.seconds = (float)((double)tick.delta_ns / 1000000000.0);
	return tick;
}

/* The end of obs_graphics_thread_loop(), without obs_video_stats_set_timing(). */
struct obs_video_frame_stats obs_video_frame_timing_stats(struct obs_video_frame_sums sums, uint64_t frame_time_ns,
							  uint64_t video_time, uint64_t last_time)
{
	struct obs_video_frame_stats stats = {0};

	sums.frame_time_total_ns += frame_time_ns;
	sums.fps_total_ns += (video_time - last_time);
	sums.fps_total_frames++;

	if (sums.fps_total_ns >= 1000000000ULL) {
		stats.report = true;
		stats.fps = (double)sums.fps_total_frames / ((double)sums.fps_total_ns / 1000000000.0);
		stats.avg_frame_time_ns = sums.frame_time_total_ns / (uint64_t)sums.fps_total_frames;

		sums.frame_time_total_ns = 0;
		sums.fps_total_ns = 0;
		sums.fps_total_frames = 0;
	}

	stats.sums = sums;
	return stats;
}
