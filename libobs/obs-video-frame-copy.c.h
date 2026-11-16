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

#include "libobs/build/libobs-ported-video-frame-copy.pch.h"

/* One copy of the plan (obs-video-copy-plan.c): returns where the source
 * ended. */
static const uint8_t *set_gpu_converted_plane(const struct obs_video_copy *copy, const uint8_t *in, uint8_t *out)
{
	if (copy->whole) {
		memcpy(out, in, copy->bytes);
		in += copy->bytes;
	} else {
		for (size_t y = 0; y < copy->rows; y++) {
			memcpy(out, in, copy->bytes);
			out += copy->dst_stride;
			in += copy->src_stride;
		}
	}

	return in;
}

void obs_video_set_gpu_converted_data(struct video_frame *output, const struct video_data *input,
				      const struct video_output_info *info)
{
	const struct obs_video_copy_plan plan =
		obs_video_gpu_copy_plan(info->format, info->width, info->height, input->linesize, output->linesize);
	const uint8_t *after_plane0 = NULL;

	for (size_t i = 0; i < plan.count && i < OBS_VIDEO_COPY_MAX; i++) {
		const struct obs_video_copy *copy = &plan.copies[i];
		const uint8_t *in = copy->src_after_plane0 ? after_plane0 : input->data[i];
		const uint8_t *end = set_gpu_converted_plane(copy, in, output->data[i]);
		if (i == 0)
			after_plane0 = end;
	}
}

void obs_video_copy_rgbx_frame(struct video_frame *output, const struct video_data *input,
			       const struct video_output_info *info)
{
	const struct obs_video_copy copy =
		obs_video_rgbx_copy_plan(info->width, info->height, input->linesize[0], output->linesize[0]);

	set_gpu_converted_plane(&copy, input->data[0], output->data[0]);
}
