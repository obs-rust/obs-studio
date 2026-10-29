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

#include "libobs/build/libobs-ported-output-begin.pch.h"

/* calculate_batch_size(): step 1, a video encoder. */
uint64_t obs_output_begin_video_interval(uint32_t fps_num, uint32_t fps_den, uint32_t frame_rate_divisor)
{
	uint32_t den = fps_den * frame_rate_divisor;
	return util_mul_div64(1000000000ULL, den, fps_num);
}

/* calculate_batch_size(): step 1, an audio encoder. */
uint64_t obs_output_begin_audio_interval(uint32_t sample_rate, size_t frame_size)
{
	return util_mul_div64(1000000000ULL, frame_size, sample_rate);
}

/* calculate_batch_size(): the largest interval, then step 2. */
size_t obs_output_begin_batch_size(const uint64_t *intervals, size_t num)
{
	uint64_t largest_interval = 0;
	size_t batch_size = 0;

	for (size_t i = 0; i < num; i++)
		largest_interval = intervals[i] > largest_interval ? intervals[i] : largest_interval;

	/* Step 2: Calculate how many packets would fit into double that interval given each encoder's packet rate.
	 * The doubling is done to provide some amount of wiggle room as the largest interval may not be evenly
	 * divisible by all smaller ones. For example, 33.3... ms video (30 FPS) and 21.3... ms audio (48 kHz AAC). */
	for (size_t i = 0; i < num; i++) {
		uint64_t packets = (largest_interval * 2) / intervals[i];
		batch_size += (size_t)packets;
	}

	return batch_size;
}

/* pair_encoders(): the first video encoder. */
bool obs_output_begin_pair_video(bool video_active)
{
	return !video_active;
}

/* pair_encoders(): one audio encoder. */
bool obs_output_begin_pair_audio(bool audio_active, size_t paired_num)
{
	return !audio_active && !paired_num;
}
