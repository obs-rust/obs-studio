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

#include "libobs/build/libobs-ported-encoder-size.pch.h"

struct obs_encoder_audio_fill obs_encoder_size_audio_defaults(const struct obs_encoder_audio_fill *info,
							      const struct obs_encoder_audio_fill *mix)
{
	struct obs_encoder_audio_fill out = *info;

	if (out.format == OBS_ENCODER_SIZE_AUDIO_FORMAT_UNKNOWN)
		out.format = mix->format;
	if (!out.samples_per_sec)
		out.samples_per_sec = mix->samples_per_sec;
	if (out.speakers == OBS_ENCODER_SIZE_SPEAKERS_UNKNOWN)
		out.speakers = mix->speakers;

	return out;
}

struct obs_encoder_size obs_encoder_size_cap_override(uint32_t caps, uint32_t width, uint32_t height,
						      uint32_t voi_width, uint32_t voi_height)
{
	struct obs_encoder_size size = {width, height};

	/**
	 * Prevent video output from performing an actual scale. If GPU scaling is
	 * enabled, the voi will contain the scaled size. Therefore, GPU scaling
	 * takes priority over self-scaling functionality.
	 */
	if ((caps & OBS_ENCODER_SIZE_CAP_SCALING) != 0) {
		size.width = voi_width;
		size.height = voi_height;
	}

	return size;
}

enum obs_encoder_scaled_action obs_encoder_size_scaled_gate(int type, bool active, bool initialized)
{
	if (type != OBS_ENCODER_SIZE_TYPE_VIDEO)
		return OBS_ENCODER_SCALED_NOT_VIDEO;
	if (active)
		return OBS_ENCODER_SCALED_ACTIVE;
	if (initialized)
		return OBS_ENCODER_SCALED_INITIALIZED;

	return OBS_ENCODER_SCALED_CHECK;
}

enum obs_encoder_scaled_action obs_encoder_size_scaled_match(bool has_voi, uint32_t voi_width, uint32_t voi_height,
							     uint32_t width, uint32_t height)
{
	if (has_voi && voi_width == width && voi_height == height)
		return OBS_ENCODER_SCALED_DISABLE;

	return OBS_ENCODER_SCALED_SET;
}

bool obs_encoder_size_scaling(uint32_t scaled_width, uint32_t scaled_height)
{
	return scaled_width || scaled_height;
}

enum obs_encoder_size_source obs_encoder_size_video_route(int type, bool has_media, uint32_t scaled)
{
	if (type != OBS_ENCODER_SIZE_TYPE_VIDEO)
		return OBS_ENCODER_SIZE_WRONG_TYPE;
	if (!has_media)
		return OBS_ENCODER_SIZE_NO_MEDIA;

	return scaled != 0 ? OBS_ENCODER_SIZE_STORED : OBS_ENCODER_SIZE_MEDIA;
}

enum obs_encoder_size_source obs_encoder_size_rate_route(int type, bool has_media, uint32_t samplerate)
{
	if (type != OBS_ENCODER_SIZE_TYPE_AUDIO)
		return OBS_ENCODER_SIZE_WRONG_TYPE;
	if (!has_media)
		return OBS_ENCODER_SIZE_NO_MEDIA;

	return samplerate != 0 ? OBS_ENCODER_SIZE_STORED : OBS_ENCODER_SIZE_MEDIA;
}
