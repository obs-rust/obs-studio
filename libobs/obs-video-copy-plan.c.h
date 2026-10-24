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

#include "libobs/build/libobs-ported-video-copy-plan.pch.h"

/* The output frame copy plan and the scale effect choice, moved from
 * obs-video.c: only numbers, no frame, graphics device or libobs state.
 * obs-video.c runs the copies and maps the effect. Ported to rust/obs-core
 * (video_copy_plan): the hybrid libobs drops this file. */

/* set_gpu_converted_plane(), without the copies: one memcpy() when the row
 * size and both line sizes match, else a row at a time. */
static struct obs_video_copy video_copy_plane(uint32_t width, uint32_t height, uint32_t linesize_input,
					      uint32_t linesize_output, bool src_after_plane0)
{
	struct obs_video_copy copy = {
		.src_after_plane0 = src_after_plane0,
		.rows = height,
		.src_stride = linesize_input,
		.dst_stride = linesize_output,
	};

	if ((width == linesize_input) && (width == linesize_output)) {
		size_t total = (size_t)width * (size_t)height;
		copy.whole = true;
		copy.bytes = total;
	} else {
		copy.bytes = width;
	}

	return copy;
}

static inline void video_copy_push(struct obs_video_copy_plan *plan, struct obs_video_copy copy)
{
	plan->copies[plan->count++] = copy;
}

struct obs_video_copy_plan obs_video_gpu_copy_plan(enum video_format format, uint32_t width, uint32_t height,
						   const uint32_t *in_linesize, const uint32_t *out_linesize)
{
	struct obs_video_copy_plan plan = {0};

	switch (format) {
	case VIDEO_FORMAT_I420: {
		video_copy_push(&plan, video_copy_plane(width, height, in_linesize[0], out_linesize[0], false));

		const uint32_t width_d2 = width / 2;
		const uint32_t height_d2 = height / 2;

		video_copy_push(&plan, video_copy_plane(width_d2, height_d2, in_linesize[1], out_linesize[1], false));

		video_copy_push(&plan, video_copy_plane(width_d2, height_d2, in_linesize[2], out_linesize[2], false));

		break;
	}
	case VIDEO_FORMAT_NV12: {
		const uint32_t height_d2 = height / 2;
		if (in_linesize[1]) {
			video_copy_push(&plan, video_copy_plane(width, height, in_linesize[0], out_linesize[0], false));
			video_copy_push(&plan,
					video_copy_plane(width, height_d2, in_linesize[1], out_linesize[1], false));
		} else {
			video_copy_push(&plan, video_copy_plane(width, height, in_linesize[0], out_linesize[0], false));
			video_copy_push(&plan,
					video_copy_plane(width, height_d2, in_linesize[0], out_linesize[1], true));
		}

		break;
	}
	case VIDEO_FORMAT_I444: {
		video_copy_push(&plan, video_copy_plane(width, height, in_linesize[0], out_linesize[0], false));

		video_copy_push(&plan, video_copy_plane(width, height, in_linesize[1], out_linesize[1], false));

		video_copy_push(&plan, video_copy_plane(width, height, in_linesize[2], out_linesize[2], false));

		break;
	}
	case VIDEO_FORMAT_I010: {
		video_copy_push(&plan, video_copy_plane(width * 2, height, in_linesize[0], out_linesize[0], false));

		const uint32_t height_d2 = height / 2;

		video_copy_push(&plan, video_copy_plane(width, height_d2, in_linesize[1], out_linesize[1], false));

		video_copy_push(&plan, video_copy_plane(width, height_d2, in_linesize[2], out_linesize[2], false));

		break;
	}
	case VIDEO_FORMAT_P010: {
		const uint32_t width_x2 = width * 2;
		const uint32_t height_d2 = height / 2;
		if (in_linesize[1]) {
			video_copy_push(&plan,
					video_copy_plane(width_x2, height, in_linesize[0], out_linesize[0], false));
			video_copy_push(&plan,
					video_copy_plane(width_x2, height_d2, in_linesize[1], out_linesize[1], false));
		} else {
			video_copy_push(&plan,
					video_copy_plane(width_x2, height, in_linesize[0], out_linesize[0], false));
			video_copy_push(&plan,
					video_copy_plane(width_x2, height_d2, in_linesize[0], out_linesize[1], true));
		}

		break;
	}
	case VIDEO_FORMAT_P216: {
		const uint32_t width_x2 = width * 2;

		video_copy_push(&plan, video_copy_plane(width_x2, height, in_linesize[0], out_linesize[0], false));

		video_copy_push(&plan, video_copy_plane(width_x2, height, in_linesize[1], out_linesize[1], false));

		break;
	}
	case VIDEO_FORMAT_P416: {
		video_copy_push(&plan, video_copy_plane(width * 2, height, in_linesize[0], out_linesize[0], false));

		video_copy_push(&plan, video_copy_plane(width * 4, height, in_linesize[1], out_linesize[1], false));

		break;
	}

	case VIDEO_FORMAT_NONE:
	case VIDEO_FORMAT_YVYU:
	case VIDEO_FORMAT_YUY2:
	case VIDEO_FORMAT_UYVY:
	case VIDEO_FORMAT_RGBA:
	case VIDEO_FORMAT_BGRA:
	case VIDEO_FORMAT_BGRX:
	case VIDEO_FORMAT_Y800:
	case VIDEO_FORMAT_BGR3:
	case VIDEO_FORMAT_I412:
	case VIDEO_FORMAT_I422:
	case VIDEO_FORMAT_I210:
	case VIDEO_FORMAT_I40A:
	case VIDEO_FORMAT_I42A:
	case VIDEO_FORMAT_YUVA:
	case VIDEO_FORMAT_YA2L:
	case VIDEO_FORMAT_AYUV:
	case VIDEO_FORMAT_V210:
	case VIDEO_FORMAT_R10L:
		/* unimplemented */
		;
	}

	return plan;
}

struct obs_video_copy obs_video_rgbx_copy_plan(uint32_t width, uint32_t height, uint32_t in_linesize,
					       uint32_t out_linesize)
{
	struct obs_video_copy copy = {
		.rows = height,
		.src_stride = in_linesize,
		.dst_stride = out_linesize,
	};

	/* if the line sizes match, do a single copy */
	if (in_linesize == out_linesize) {
		copy.whole = true;
		copy.bytes = (size_t)in_linesize * (size_t)height;
	} else {
		const size_t copy_size = (size_t)width * 4;
		copy.bytes = copy_size;
	}

	return copy;
}

static enum obs_video_scale_effect video_scale_effect_internal(const struct obs_video_scale_input *input)
{
	/* if the dimension is under half the size of the original image,
	 * bicubic/lanczos can't sample enough pixels to create an accurate
	 * image, so use the bilinear low resolution effect instead */
	if (input->output_width < (input->base_width / 2) && input->output_height < (input->base_height / 2)) {
		return OBS_VIDEO_SCALE_EFFECT_BILINEAR_LOWRES;
	}

	switch (input->scale_type) {
	case OBS_VIDEO_COPY_SCALE_BILINEAR:
		return OBS_VIDEO_SCALE_EFFECT_DEFAULT;
	case OBS_VIDEO_COPY_SCALE_LANCZOS:
		return OBS_VIDEO_SCALE_EFFECT_LANCZOS;
	case OBS_VIDEO_COPY_SCALE_AREA:
		return OBS_VIDEO_SCALE_EFFECT_AREA;
	case OBS_VIDEO_COPY_SCALE_BICUBIC:
	default:;
	}

	return OBS_VIDEO_SCALE_EFFECT_BICUBIC;
}

bool obs_video_scale_resolution_close(uint32_t base_width, uint32_t base_height, uint32_t width, uint32_t height)
{
	long width_cmp = (long)base_width - (long)width;
	long height_cmp = (long)base_height - (long)height;

	return labs(width_cmp) <= 16 && labs(height_cmp) <= 16;
}

enum obs_video_scale_effect obs_video_scale_effect_choice(const struct obs_video_scale_input *input)
{
	if (obs_video_scale_resolution_close(input->base_width, input->base_height, input->target_width,
					     input->target_height)) {
		return OBS_VIDEO_SCALE_EFFECT_DEFAULT;
	} else {
		/* if the scale method couldn't be loaded, use either bicubic
		 * or bilinear by default */
		enum obs_video_scale_effect effect = video_scale_effect_internal(input);
		if (!(input->loaded & OBS_VIDEO_SCALE_LOADED(effect)))
			effect = !!(input->loaded & OBS_VIDEO_SCALE_LOADED(OBS_VIDEO_SCALE_EFFECT_BICUBIC))
					 ? OBS_VIDEO_SCALE_EFFECT_BICUBIC
					 : OBS_VIDEO_SCALE_EFFECT_DEFAULT;
		return effect;
	}
}

bool obs_video_mix_reuse_match(const struct obs_video_mix_reuse_key *other, const struct obs_video_mix_reuse_key *mix)
{
	if (other->view != mix->view)
		return false;
	if (other->render_space != mix->render_space)
		return false;
	if (other->base_width != mix->base_width || other->base_height != mix->base_height)
		return false;
	if (!other->rendered)
		return false;

	return true;
}
