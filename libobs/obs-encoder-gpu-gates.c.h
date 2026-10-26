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

#include "libobs/build/libobs-ported-encoder-gpu-gates.pch.h"

bool obs_encoder_gpu_gates_warming(int wait_frames)
{
	return wait_frames;
}

int obs_encoder_gpu_gates_warm_next(int wait_frames)
{
	return wait_frames - 1;
}

uint32_t obs_encoder_gpu_gates_divisor_inc(uint32_t counter)
{
	return counter + 1;
}

bool obs_encoder_gpu_gates_divisor_wrap(uint32_t counter, uint32_t frame_rate_divisor)
{
	return counter == frame_rate_divisor;
}

bool obs_encoder_gpu_gates_divisor_skip(uint32_t skip)
{
	return skip;
}

uint64_t obs_encoder_gpu_gates_next_key(size_t lock_count, size_t num_encoders, uint64_t next_key)
{
	if (lock_count == num_encoders)
		return 0;

	return next_key + 1;
}

bool obs_encoder_gpu_gates_record_timing(uint64_t frame_ts)
{
	return frame_ts;
}

int obs_encoder_gpu_gates_count_next(int count)
{
	return count - 1;
}

bool obs_encoder_gpu_gates_requeue(int count)
{
	return count;
}

uint64_t obs_encoder_gpu_gates_requeue_ts(uint64_t timestamp, uint64_t interval)
{
	return timestamp + interval;
}

bool obs_encoder_gpu_gates_p010(int format)
{
	return format == OBS_ENCODER_GPU_GATES_FORMAT_P010;
}
