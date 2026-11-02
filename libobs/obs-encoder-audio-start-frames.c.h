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

#include "libobs/build/libobs-ported-encoder-audio-start-frames.pch.h"

uint64_t obs_encoder_audio_start_frames_offset(uint64_t offset_ns, uint32_t samplerate)
{
	return util_mul_div64(offset_ns, samplerate, 1000000000ULL);
}

uint64_t obs_encoder_audio_start_frames_duration(uint32_t frames, uint32_t samplerate)
{
	return util_mul_div64(frames, 1000000000ULL, samplerate);
}
