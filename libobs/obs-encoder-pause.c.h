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

#include "libobs/build/libobs-ported-encoder-pause.pch.h"

bool obs_encoder_pause_off(uint64_t ts_start)
{
	return !ts_start;
}

bool obs_encoder_pause_video_end(uint64_t ts_end, uint64_t ts)
{
	return ts == ts_end;
}

bool obs_encoder_pause_video_drop(uint64_t ts_start, uint64_t ts)
{
	return ts >= ts_start;
}

uint64_t obs_encoder_pause_audio_end(uint64_t timestamp, uint64_t duration_ns)
{
	return timestamp + duration_ns;
}

int obs_encoder_pause_audio_start(uint64_t ts_start, uint64_t timestamp, uint64_t end_ts)
{
	if (ts_start >= timestamp) {
		if (ts_start <= end_ts) {
			return OBS_ENCODER_PAUSE_AUDIO_START_INSIDE;
		}
		return OBS_ENCODER_PAUSE_AUDIO_START_AFTER;
	}

	return OBS_ENCODER_PAUSE_AUDIO_START_BEFORE;
}

int obs_encoder_pause_audio_paused(uint64_t ts_end, uint64_t timestamp, uint64_t end_ts)
{
	if (ts_end >= timestamp && ts_end <= end_ts) {
		return OBS_ENCODER_PAUSE_AUDIO_PAUSED_UNPAUSE;
	}

	return OBS_ENCODER_PAUSE_AUDIO_PAUSED_DROP;
}

uint64_t obs_encoder_pause_audio_cut_ns(uint64_t ts, uint64_t timestamp)
{
	return ts - timestamp;
}

uint32_t obs_encoder_pause_audio_kept(uint64_t cut_frames)
{
	return (uint32_t)cut_frames;
}

uint32_t obs_encoder_pause_audio_left(uint32_t frames, uint64_t cut_frames)
{
	return frames - (uint32_t)cut_frames;
}

bool obs_encoder_pause_audio_drop(uint32_t frames)
{
	return !frames;
}
