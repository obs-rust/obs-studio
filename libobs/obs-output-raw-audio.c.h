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

#include "libobs/build/libobs-ported-output-raw-audio.pch.h"

bool obs_output_raw_audio_passthrough(uint32_t flags)
{
	return (flags & OBS_OUTPUT_RAW_AUDIO_FLAG_VIDEO) == 0;
}

bool obs_output_raw_audio_video_unset(uint64_t video_start_ts)
{
	return !video_start_ts;
}

bool obs_output_raw_audio_before_video(uint64_t timestamp, uint64_t video_start_ts)
{
	return timestamp < video_start_ts;
}

bool obs_output_raw_audio_ends_before(uint64_t timestamp, uint64_t duration_ns, uint64_t video_start_ts)
{
	uint64_t end_ts = (timestamp + duration_ns);
	return end_ts <= video_start_ts;
}

uint64_t obs_output_raw_audio_cut_ns(uint64_t video_start_ts, uint64_t timestamp)
{
	return video_start_ts - timestamp;
}

uint64_t obs_output_raw_audio_cut_ts(uint64_t timestamp, uint64_t cut_ns)
{
	return timestamp + cut_ns;
}

size_t obs_output_raw_audio_cut_bytes(size_t audio_size, uint64_t cut_frames)
{
	return audio_size * (uint32_t)cut_frames;
}

uint32_t obs_output_raw_audio_cut_frames(uint32_t frames, uint64_t cut_frames)
{
	return frames - (uint32_t)cut_frames;
}

bool obs_output_raw_audio_start_unset(uint64_t audio_start_ts)
{
	return !audio_start_ts;
}

size_t obs_output_raw_audio_block_bytes(size_t audio_size)
{
	return OBS_OUTPUT_RAW_AUDIO_FRAMES * audio_size;
}

size_t obs_output_raw_audio_push_bytes(uint32_t frames, size_t audio_size)
{
	return frames * audio_size;
}

bool obs_output_raw_audio_can_pop(size_t buffered, size_t block_bytes)
{
	return buffered > block_bytes;
}

uint64_t obs_output_raw_audio_block_ts(uint64_t audio_start_ts, uint64_t frames_ns)
{
	return audio_start_ts + frames_ns;
}

uint64_t obs_output_raw_audio_paused_ts(uint64_t timestamp, uint64_t ts_offset)
{
	return timestamp + ts_offset;
}

size_t obs_output_raw_audio_next_frames(size_t total_audio_frames)
{
	return total_audio_frames + OBS_OUTPUT_RAW_AUDIO_FRAMES;
}

bool obs_output_raw_audio_mix_on(size_t mixer_mask, size_t idx)
{
	return (mixer_mask & ((size_t)1 << idx)) != 0;
}

bool obs_output_raw_audio_interleave(bool has_video, bool has_audio)
{
	return has_video && has_audio;
}

bool obs_output_raw_audio_delay_on(uint32_t delay_sec)
{
	return delay_sec != 0;
}

uint64_t obs_output_raw_audio_delay_ns(uint32_t delay_sec)
{
	return (uint64_t)delay_sec * 1000000000ULL;
}
