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

#include "libobs/build/libobs.pch.h"

uint64_t obs_output_raw_audio_trim_duration(uint32_t frames, size_t sample_rate)
{
	return util_mul_div64(frames, 1000000000ULL, sample_rate);
}

uint64_t obs_output_raw_audio_trim_frames(uint64_t cut_ns, size_t sample_rate)
{
	return util_mul_div64(cut_ns, sample_rate, 1000000000ULL);
}
