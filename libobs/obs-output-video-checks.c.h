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

#include "libobs/build/libobs-ported-output-video-checks.pch.h"

size_t obs_output_first_mixer(size_t mixer_mask)
{
	for (size_t i = 0; i < OBS_OUTPUT_VIDEO_CHECKS_MAX_AUDIO_MIXES; i++) {
		if ((((size_t)1 << i) & mixer_mask) != 0) {
			return i;
		}
	}

	return 0;
}

enum obs_output_size_source obs_output_size_route(bool encoded, bool has_encoder, uint32_t scaled)
{
	if (encoded) {
		if (has_encoder)
			return OBS_OUTPUT_SIZE_ENCODER;
		else
			return OBS_OUTPUT_SIZE_ZERO;
	} else
		return scaled != 0 ? OBS_OUTPUT_SIZE_SCALED : OBS_OUTPUT_SIZE_VIDEO;
}

static inline bool checks_flag_encoded(const struct obs_output_capture_presence *p)
{
	return (p->flags & OBS_OUTPUT_VIDEO_CHECKS_ENCODED) != 0;
}

static inline bool checks_video_valid(const struct obs_output_capture_presence *p)
{
	if (checks_flag_encoded(p)) {
		for (size_t i = 0; i < OBS_OUTPUT_VIDEO_CHECKS_MAX_VIDEO_ENCODERS; i++) {
			if ((p->video_encoders & ((uint32_t)1 << i)) != 0) {
				return true;
			}
		}
		return false;
	} else {
		return p->video;
	}
}

static inline bool checks_audio_valid(const struct obs_output_capture_presence *p)
{
	if (checks_flag_encoded(p)) {
		for (size_t i = 0; i < OBS_OUTPUT_VIDEO_CHECKS_MAX_AUDIO_ENCODERS; i++) {
			if ((p->audio_encoders & ((uint32_t)1 << i)) != 0) {
				return true;
			}
		}
		return false;
	}

	return p->audio;
}

bool obs_output_capture_ready(const struct obs_output_capture_presence *presence)
{
	if ((presence->flags & OBS_OUTPUT_VIDEO_CHECKS_VIDEO) != 0 && !checks_video_valid(presence))
		return false;

	if ((presence->flags & OBS_OUTPUT_VIDEO_CHECKS_AUDIO) != 0 && !checks_audio_valid(presence))
		return false;

	if ((presence->flags & OBS_OUTPUT_VIDEO_CHECKS_SERVICE) != 0 && !presence->service)
		return false;

	return true;
}

enum obs_output_begin_action obs_output_begin_gate(bool delay_active, bool active)
{
	if (delay_active)
		return OBS_OUTPUT_BEGIN_READY;
	if (active)
		return OBS_OUTPUT_BEGIN_REFUSE;

	return OBS_OUTPUT_BEGIN_CHECK;
}

static inline bool checks_has_scaling(const struct obs_output_conversion_sizes *s)
{
	uint32_t video_width = s->video_width;
	uint32_t video_height = s->video_height;

	return s->scaled_width && s->scaled_height &&
	       (video_width != s->scaled_width || video_height != s->scaled_height);
}

struct obs_output_conversion_choice obs_output_conversion_plan(const struct obs_output_conversion_sizes *sizes)
{
	struct obs_output_conversion_choice plan = {OBS_OUTPUT_CONVERSION_NONE, 0, 0};

	if (sizes->set) {
		/* obs_output_get_width() / _height() of a raw output: the scaled
		 * size when it is set, else the video's. */
		plan.kind = OBS_OUTPUT_CONVERSION_SET;
		plan.width = sizes->set_width;
		plan.height = sizes->set_height;

		if (!plan.width)
			plan.width = sizes->scaled_width != 0 ? sizes->scaled_width : sizes->video_width;

		if (!plan.height)
			plan.height = sizes->scaled_height != 0 ? sizes->scaled_height : sizes->video_height;

		return plan;

	} else if (checks_has_scaling(sizes)) {
		plan.kind = OBS_OUTPUT_CONVERSION_SCALED;
		plan.width = sizes->scaled_width;
		plan.height = sizes->scaled_height;
		return plan;
	}

	return plan;
}

float obs_output_congestion_clamp(float congestion)
{
	float val = congestion;
	if (val < 0.0f)
		val = 0.0f;
	else if (val > 1.0f)
		val = 1.0f;
	return val;
}
