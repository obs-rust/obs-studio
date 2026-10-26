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

#include "libobs/build/libobs-ported-source-audio-place.pch.h"

/* The source audio placement and action timeline of obs-source.c, moved
 * from get_buf_placement(), source_output_audio_place(),
 * source_output_audio_push_back(), source_output_audio_data() and
 * apply_audio_actions(): the decisions, on plain values. obs-source.c keeps
 * every read of the source's fields, both mutexes, the deques and every
 * call where they were, passes the values in and acts on what comes back.
 * Declared in obs-source-audio-place.h without EXPORT. In hybrid libobs the
 * Rust port (rust/obs-source, audio_place) replaces this file. */

bool obs_source_audio_place_started(uint64_t audio_ts)
{
	return audio_ts != 0;
}

bool obs_source_audio_place_older(uint64_t timestamp, uint64_t audio_ts)
{
	return timestamp < audio_ts;
}

size_t obs_source_audio_place_size(uint32_t frames)
{
	return frames * sizeof(float);
}

size_t obs_source_audio_place_offset(uint64_t timestamp, uint64_t audio_ts, uint32_t sample_rate)
{
	return (size_t)util_mul_div64(timestamp - audio_ts, sample_rate, 1000000000ULL) * sizeof(float);
}

bool obs_source_audio_place_fits(size_t start, size_t size)
{
	return !((start + size) > OBS_SOURCE_AUDIO_PLACE_MAX_BUF_SIZE);
}

size_t obs_source_audio_place_pop_back(size_t buf_size, size_t placement, size_t size)
{
	return buf_size - (placement + size);
}

bool obs_source_audio_place_outputs(int monitoring_type)
{
	return monitoring_type != OBS_SOURCE_AUDIO_PLACE_MONITOR_ONLY;
}

size_t obs_source_audio_place_action_frame(uint64_t timestamp, uint64_t audio_ts, size_t sample_rate)
{
	return (size_t)util_mul_div64(timestamp - audio_ts, sample_rate, 1000000000ULL);
}

bool obs_source_audio_place_action_stops(size_t frame)
{
	return frame >= OBS_SOURCE_AUDIO_PLACE_FRAMES;
}

size_t obs_source_audio_place_fill_end(size_t frame_num, size_t new_frame_num)
{
	return new_frame_num > frame_num ? new_frame_num : frame_num;
}

bool obs_source_audio_place_mix_on(uint32_t audio_mixers, size_t mix)
{
	return (audio_mixers & (1 << mix)) != 0;
}
