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

#include "libobs/build/libobs-ported-output-slots.pch.h"

bool obs_output_slots_first_video(uint32_t occupied, size_t *index)
{
	for (size_t i = 0; i < OBS_OUTPUT_SLOTS_MAX_VIDEO_ENCODERS; i++) {
		if ((occupied & ((uint32_t)1 << i)) != 0) {
			*index = i;
			return true;
		}
	}

	return false;
}

bool obs_output_slots_first_audio(uint32_t occupied, size_t *index)
{
	for (size_t i = 0; i < OBS_OUTPUT_SLOTS_MAX_AUDIO_ENCODERS; i++) {
		if ((occupied & ((uint32_t)1 << i)) != 0) {
			*index = i;
			return true;
		}
	}

	return false;
}

bool obs_output_slots_mixer_in_range(size_t mixer_idx)
{
	return !(mixer_idx >= OBS_OUTPUT_SLOTS_MAX_AUDIO_MIXES);
}

size_t obs_output_slots_mixer_mask(size_t mixer_idx)
{
	return (size_t)1 << mixer_idx;
}

bool obs_output_slots_video_type_ok(int encoder_type)
{
	return !(encoder_type != OBS_OUTPUT_SLOTS_ENCODER_VIDEO);
}

bool obs_output_slots_audio_type_ok(int encoder_type)
{
	return !(encoder_type != OBS_OUTPUT_SLOTS_ENCODER_AUDIO);
}

bool obs_output_slots_video_index_ok(uint32_t flags, size_t idx)
{
	if ((flags & OBS_OUTPUT_SLOTS_MULTI_TRACK_VIDEO) != 0) {
		if (idx >= OBS_OUTPUT_SLOTS_MAX_VIDEO_ENCODERS) {
			return false;
		}
	} else {
		if (idx > 0) {
			return false;
		}
	}

	return true;
}

bool obs_output_slots_audio_index_ok(uint32_t flags, size_t idx)
{
	if ((flags & OBS_OUTPUT_SLOTS_MULTI_TRACK_AUDIO) != 0) {
		if (idx >= OBS_OUTPUT_SLOTS_MAX_AUDIO_ENCODERS) {
			return false;
		}
	} else {
		if (idx > 0) {
			return false;
		}
	}

	return true;
}

bool obs_output_slots_default_index(size_t idx)
{
	return idx == 0;
}

bool obs_output_slots_scaled_axis_set(uint32_t size)
{
	return size != 0;
}

bool obs_output_slots_video_slot(size_t idx)
{
	return !(idx >= OBS_OUTPUT_SLOTS_MAX_VIDEO_ENCODERS);
}

bool obs_output_slots_audio_slot(size_t idx)
{
	return !(idx >= OBS_OUTPUT_SLOTS_MAX_AUDIO_ENCODERS);
}
