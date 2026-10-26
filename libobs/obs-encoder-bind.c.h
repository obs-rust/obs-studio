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

#include "libobs/build/libobs-ported-encoder-bind.pch.h"

bool obs_encoder_bind_not_audio(int type)
{
	return type != OBS_ENCODER_BIND_TYPE_AUDIO;
}

struct obs_encoder_bind_timebase obs_encoder_bind_video_timebase(bool has_video, uint32_t fps_num, uint32_t fps_den)
{
	struct obs_encoder_bind_timebase timebase;

	if (has_video) {
		timebase.num = fps_den;
		timebase.den = fps_num;
	} else {
		timebase.num = 0;
		timebase.den = 0;
	}
	return timebase;
}

struct obs_encoder_bind_timebase obs_encoder_bind_audio_timebase(bool has_audio, uint32_t sample_rate)
{
	struct obs_encoder_bind_timebase timebase;

	if (has_audio) {
		timebase.num = 1;
		timebase.den = sample_rate;
	} else {
		timebase.num = 0;
		timebase.den = 0;
	}
	return timebase;
}

bool obs_encoder_bind_divisor_override(uint32_t frame_rate_divisor)
{
	return frame_rate_divisor;
}

bool obs_encoder_bind_locked(int trylock_result)
{
	return trylock_result == 0;
}

bool obs_encoder_bind_shutdown_unstarted(bool initialized, bool needed)
{
	return initialized && !needed;
}
