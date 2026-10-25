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

#include "libobs/build/libobs-ported-encoder-audio-start.pch.h"

bool obs_encoder_audio_start_group_ready(uint64_t group_start, uint64_t timestamp)
{
	return group_start == timestamp;
}

bool obs_encoder_audio_start_paired_late(uint64_t first_raw_ts, uint64_t timestamp)
{
	return first_raw_ts > timestamp;
}

int64_t obs_encoder_audio_start_video_pts(int64_t cur_pts, uint32_t timebase_num, uint32_t frame_rate_divisor)
{
	return cur_pts + timebase_num * frame_rate_divisor;
}

size_t obs_encoder_audio_start_size(uint32_t frames, size_t blocksize)
{
	return frames * blocksize;
}

bool obs_encoder_audio_start_paired(uint64_t start_ts, bool has_paired)
{
	return !start_ts && has_paired;
}

bool obs_encoder_audio_start_unpaired(uint64_t start_ts, bool has_paired)
{
	return !start_ts && !has_paired;
}

bool obs_encoder_audio_start_no_video(uint64_t v_start_ts)
{
	/* no video yet, so don't start audio */
	return !v_start_ts;
}

struct obs_encoder_audio_start_plan obs_encoder_audio_start_packet(uint64_t timestamp, uint64_t duration_ns,
								   uint64_t v_start_ts)
{
	struct obs_encoder_audio_start_plan plan = {0};
	uint64_t end_ts = timestamp;

	/* audio starting point still not synced with video starting
	 * point, so don't start audio */
	end_ts += duration_ns;
	if (end_ts <= v_start_ts) {
		plan.wait = true;
		return plan;
	}

	/* ready to start audio, truncate if necessary */
	plan.truncate = timestamp < v_start_ts;
	plan.clear = timestamp <= v_start_ts;

	/* use currently buffered audio instead */
	plan.from_buffer = v_start_ts < timestamp;
	return plan;
}

uint64_t obs_encoder_audio_start_offset_ns(uint64_t v_start_ts, uint64_t a_start_ts)
{
	return v_start_ts - a_start_ts;
}

size_t obs_encoder_audio_start_offset_size(uint64_t offset_frames, size_t blocksize)
{
	return (size_t)offset_frames * blocksize;
}

bool obs_encoder_audio_start_buffer_offset(uint64_t first_raw_ts, uint64_t v_start_ts)
{
	return first_raw_ts < v_start_ts;
}

size_t obs_encoder_audio_start_push_size(size_t size, size_t offset_size)
{
	if (offset_size >= size)
		return 0;

	return size - offset_size;
}

bool obs_encoder_audio_start_can_send(size_t buffered, size_t framesize_bytes)
{
	return buffered >= framesize_bytes;
}

int64_t obs_encoder_audio_start_audio_pts(int64_t cur_pts, size_t framesize)
{
	return cur_pts + framesize;
}

bool obs_encoder_audio_start_not_video(int type)
{
	return type != OBS_ENCODER_AUDIO_START_TYPE_VIDEO;
}

bool obs_encoder_audio_start_divisor_ok(uint32_t frame_rate_divisor)
{
	return frame_rate_divisor != 0;
}
