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

#include "libobs/build/libobs-ported-output-pause.pch.h"

bool obs_output_pause_allowed(uint32_t flags)
{
	return (flags & OBS_OUTPUT_PAUSE_CAN_PAUSE) != 0;
}

bool obs_output_pause_already(bool paused, bool pause)
{
	return paused == pause;
}

void obs_output_pause_lock_set(const uintptr_t video[OBS_OUTPUT_PAUSE_MAX_VIDEO_ENCODERS],
			       const uintptr_t audio[OBS_OUTPUT_PAUSE_MAX_AUDIO_ENCODERS], uint32_t *video_lock,
			       uint32_t *audio_lock)
{
	*video_lock = 0;
	*audio_lock = 0;

	for (uint32_t i = 0; i < OBS_OUTPUT_PAUSE_MAX_VIDEO_ENCODERS; i++) {
		if (video[i]) {
			*video_lock |= (uint32_t)1 << i;
		}
	}
	for (uint32_t i = 0; i < OBS_OUTPUT_PAUSE_MAX_AUDIO_ENCODERS; i++) {
		if (audio[i]) {
			*audio_lock |= (uint32_t)1 << i;
		}
	}
}

uint64_t obs_output_pause_closest_ts(uint64_t last_video_ts, uint64_t interval, uint64_t now)
{
	uint64_t i2 = interval * 2;

	return last_video_ts + ((now - last_video_ts + i2) / interval) * interval;
}

bool obs_output_pause_started(uint64_t ts_start)
{
	return !!ts_start;
}

bool obs_output_pause_ended(uint64_t ts_end)
{
	return !!ts_end;
}

uint64_t obs_output_pause_offset(uint64_t ts_offset, uint64_t ts_start, uint64_t ts_end)
{
	return ts_offset + (ts_end - ts_start);
}
