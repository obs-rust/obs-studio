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

#include "libobs/build/libobs-ported-encoder-gpu-rescale.pch.h"

bool obs_encoder_gpu_rescale_wanted(bool has_media, const struct obs_encoder_gpu_rescale_request *request)
{
	if (!has_media)
		return false;
	if (request->scale_type == OBS_ENCODER_GPU_RESCALE_SCALE_DISABLE)
		return false;
	if (!request->scaled_height && !request->scaled_width &&
	    request->format == OBS_ENCODER_GPU_RESCALE_FORMAT_NONE &&
	    request->colorspace == OBS_ENCODER_GPU_RESCALE_CS_DEFAULT &&
	    request->range == OBS_ENCODER_GPU_RESCALE_RANGE_DEFAULT)
		return false;
	return true;
}

struct obs_encoder_gpu_rescale_output
obs_encoder_gpu_rescale_target(const struct obs_encoder_gpu_rescale_request *request,
			       const struct obs_encoder_gpu_rescale_output *video)
{
	struct obs_encoder_gpu_rescale_output target;

	target.width = request->scaled_width ? request->scaled_width : video->width;
	target.height = request->scaled_height ? request->scaled_height : video->height;
	target.format = request->format != OBS_ENCODER_GPU_RESCALE_FORMAT_NONE ? request->format : video->format;
	target.colorspace = request->colorspace != OBS_ENCODER_GPU_RESCALE_CS_DEFAULT ? request->colorspace
										      : video->colorspace;
	target.range = request->range != OBS_ENCODER_GPU_RESCALE_RANGE_DEFAULT ? request->range : video->range;
	return target;
}

bool obs_encoder_gpu_rescale_matches(bool same_view, int mix_scale_type,
				     const struct obs_encoder_gpu_rescale_output *mix, int scale_type,
				     const struct obs_encoder_gpu_rescale_output *target)
{
	if (!same_view)
		return false;

	if (mix_scale_type != scale_type)
		return false;

	if (mix->width != target->width || mix->height != target->height)
		return false;

	if (mix->format != target->format || mix->colorspace != target->colorspace || mix->range != target->range)
		return false;

	return true;
}

enum obs_encoder_gpu_rescale_release obs_encoder_gpu_rescale_release_step(bool is_media, bool encoder_only)
{
	if (!is_media)
		return OBS_ENCODER_GPU_RESCALE_RELEASE_SKIP;

	if (!encoder_only)
		return OBS_ENCODER_GPU_RESCALE_RELEASE_STOP;

	return OBS_ENCODER_GPU_RESCALE_RELEASE_DROP;
}

long obs_encoder_gpu_rescale_release_refs(long encoder_refs, bool *last)
{
	encoder_refs -= 1;
	*last = encoder_refs == 0;
	return encoder_refs;
}
