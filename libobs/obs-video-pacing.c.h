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

#include "libobs/build/libobs-ported-video-pacing.pch.h"

/* video_sleep(), without the sleep and the clock read. */
struct obs_video_pacing_step obs_video_pacing_step(uint64_t frame_time, uint64_t interval_ns, bool slept, uint64_t now)
{
	struct obs_video_pacing_step step;

	if (slept) {
		step.frame_time = frame_time + interval_ns;
		step.count = 1;
	} else {
		const uint64_t udiff = now - frame_time;
		int64_t diff;
		memcpy(&diff, &udiff, sizeof(diff));
		const uint64_t clamped_diff = (diff > (int64_t)interval_ns) ? (uint64_t)diff : interval_ns;
		step.count = (int)(clamped_diff / interval_ns);
		/* The count as uint64_t, as the multiplication converts it. */
		step.frame_time = frame_time + interval_ns * (uint64_t)step.count;
	}

	return step;
}

/* video_sleep(), the start timestamp test of each ready encoder's group. */
bool obs_video_pacing_group_starts(uint32_t num_encoders_started, size_t num_encoders, uint64_t start_timestamp)
{
	return num_encoders_started >= num_encoders && !start_timestamp;
}
