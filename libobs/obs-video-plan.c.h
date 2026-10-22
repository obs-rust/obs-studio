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

#include "libobs/build/libobs-ported-video-plan.pch.h"

/* The video output plan, moved from obs.c: what obs_reset_video() accepts,
 * how it aligns the output size, and, for each output format, the GPU
 * conversion (shader techniques and texel sizes), the size and format of
 * each conversion plane, and the format and color space of the render
 * textures. Pure: no graphics device and no libobs state. obs.c creates the
 * textures and surfaces from it and copies the rest into the video mix.
 * Ported to rust/obs-core (video_plan): the hybrid libobs drops this file. */

#define OBS_SIZE_MIN 2
#define OBS_SIZE_MAX (32 * 1024)

bool obs_video_size_valid(uint32_t width, uint32_t height)
{
	return (width >= OBS_SIZE_MIN && height >= OBS_SIZE_MIN && width <= OBS_SIZE_MAX && height <= OBS_SIZE_MAX);
}

#undef OBS_SIZE_MIN
#undef OBS_SIZE_MAX

void obs_video_align_output_size(uint32_t *width, uint32_t *height)
{
	/* align to multiple-of-two and SSE alignment sizes */
	*width &= 0xFFFFFFFC;
	*height &= 0xFFFFFFFE;
}

/* The output size obs_reset_video() accepts: valid as given and still valid
 * once aligned. The alignment makes an output width of 2 or 3 into 0. */
bool obs_video_output_size_valid(uint32_t width, uint32_t height)
{
	if (!obs_video_size_valid(width, height))
		return false;
	obs_video_align_output_size(&width, &height);
	return obs_video_size_valid(width, height);
}

const char *obs_video_scale_type_name(enum obs_scale_type scale_type)
{
	const char *scale_type_name = "";
	switch (scale_type) {
	case OBS_SCALE_DISABLE:
		scale_type_name = "Disabled";
		break;
	case OBS_SCALE_POINT:
		scale_type_name = "Point";
		break;
	case OBS_SCALE_BICUBIC:
		scale_type_name = "Bicubic";
		break;
	case OBS_SCALE_BILINEAR:
		scale_type_name = "Bilinear";
		break;
	case OBS_SCALE_LANCZOS:
		scale_type_name = "Lanczos";
		break;
	case OBS_SCALE_AREA:
		scale_type_name = "Area";
		break;
	}
	return scale_type_name;
}

static inline void set_plane(struct obs_video_plane *plane, uint32_t width, uint32_t height,
			     enum gs_color_format format)
{
	plane->width = width;
	plane->height = height;
	plane->format = format;
}

static inline void calc_gpu_conversion_sizes(struct obs_video_output_plan *plan, enum video_format format,
					     enum video_colorspace colorspace, uint32_t width, uint32_t height)
{
	switch (format) {
	case VIDEO_FORMAT_I420:
		plan->conversion_needed = true;
		plan->conversion_techs[0] = "Planar_Y";
		plan->conversion_techs[1] = "Planar_U_Left";
		plan->conversion_techs[2] = "Planar_V_Left";
		plan->conversion_width_i = 1.f / (float)width;
		break;
	case VIDEO_FORMAT_NV12:
		plan->conversion_needed = true;
		plan->conversion_techs[0] = "NV12_Y";
		plan->conversion_techs[1] = "NV12_UV";
		plan->conversion_width_i = 1.f / (float)width;
		break;
	case VIDEO_FORMAT_I444:
		plan->conversion_needed = true;
		plan->conversion_techs[0] = "Planar_Y";
		plan->conversion_techs[1] = "Planar_U";
		plan->conversion_techs[2] = "Planar_V";
		break;
	case VIDEO_FORMAT_I010:
		plan->conversion_needed = true;
		plan->conversion_width_i = 1.f / (float)width;
		plan->conversion_height_i = 1.f / (float)height;
		if (colorspace == VIDEO_CS_2100_PQ) {
			plan->conversion_techs[0] = "I010_PQ_Y";
			plan->conversion_techs[1] = "I010_PQ_U";
			plan->conversion_techs[2] = "I010_PQ_V";
		} else if (colorspace == VIDEO_CS_2100_HLG) {
			plan->conversion_techs[0] = "I010_HLG_Y";
			plan->conversion_techs[1] = "I010_HLG_U";
			plan->conversion_techs[2] = "I010_HLG_V";
		} else {
			plan->conversion_techs[0] = "I010_SRGB_Y";
			plan->conversion_techs[1] = "I010_SRGB_U";
			plan->conversion_techs[2] = "I010_SRGB_V";
		}
		break;
	case VIDEO_FORMAT_P010:
		plan->conversion_needed = true;
		plan->conversion_width_i = 1.f / (float)width;
		plan->conversion_height_i = 1.f / (float)height;
		if (colorspace == VIDEO_CS_2100_PQ) {
			plan->conversion_techs[0] = "P010_PQ_Y";
			plan->conversion_techs[1] = "P010_PQ_UV";
		} else if (colorspace == VIDEO_CS_2100_HLG) {
			plan->conversion_techs[0] = "P010_HLG_Y";
			plan->conversion_techs[1] = "P010_HLG_UV";
		} else {
			plan->conversion_techs[0] = "P010_SRGB_Y";
			plan->conversion_techs[1] = "P010_SRGB_UV";
		}
		break;
	case VIDEO_FORMAT_P216:
		plan->conversion_needed = true;
		plan->conversion_width_i = 1.f / (float)width;
		plan->conversion_height_i = 1.f / (float)height;
		if (colorspace == VIDEO_CS_2100_PQ) {
			plan->conversion_techs[0] = "P216_PQ_Y";
			plan->conversion_techs[1] = "P216_PQ_UV";
		} else if (colorspace == VIDEO_CS_2100_HLG) {
			plan->conversion_techs[0] = "P216_HLG_Y";
			plan->conversion_techs[1] = "P216_HLG_UV";
		} else {
			plan->conversion_techs[0] = "P216_SRGB_Y";
			plan->conversion_techs[1] = "P216_SRGB_UV";
		}
		break;
	case VIDEO_FORMAT_P416:
		plan->conversion_needed = true;
		plan->conversion_width_i = 1.f / (float)width;
		plan->conversion_height_i = 1.f / (float)height;
		if (colorspace == VIDEO_CS_2100_PQ) {
			plan->conversion_techs[0] = "P416_PQ_Y";
			plan->conversion_techs[1] = "P416_PQ_UV";
		} else if (colorspace == VIDEO_CS_2100_HLG) {
			plan->conversion_techs[0] = "P416_HLG_Y";
			plan->conversion_techs[1] = "P416_HLG_UV";
		} else {
			plan->conversion_techs[0] = "P416_SRGB_Y";
			plan->conversion_techs[1] = "P416_SRGB_UV";
		}
		break;
	default:
		break;
	}
}

/* The conversion textures and the copy surfaces of each plane. */
static inline void calc_gpu_conversion_planes(struct obs_video_output_plan *plan, enum video_format format,
					      uint32_t width, uint32_t height)
{
	switch (format) {
	case VIDEO_FORMAT_I420:
		set_plane(&plan->planes[0], width, height, GS_R8);
		set_plane(&plan->planes[1], width / 2, height / 2, GS_R8);
		set_plane(&plan->planes[2], width / 2, height / 2, GS_R8);
		break;
	case VIDEO_FORMAT_NV12:
		set_plane(&plan->planes[0], width, height, GS_R8);
		set_plane(&plan->planes[1], width / 2, height / 2, GS_R8G8);
		break;
	case VIDEO_FORMAT_I444:
		set_plane(&plan->planes[0], width, height, GS_R8);
		set_plane(&plan->planes[1], width, height, GS_R8);
		set_plane(&plan->planes[2], width, height, GS_R8);
		break;
	case VIDEO_FORMAT_I010:
		set_plane(&plan->planes[0], width, height, GS_R16);
		set_plane(&plan->planes[1], width / 2, height / 2, GS_R16);
		set_plane(&plan->planes[2], width / 2, height / 2, GS_R16);
		break;
	case VIDEO_FORMAT_P010:
		set_plane(&plan->planes[0], width, height, GS_R16);
		set_plane(&plan->planes[1], width / 2, height / 2, GS_RG16);
		break;
	case VIDEO_FORMAT_P216:
		set_plane(&plan->planes[0], width, height, GS_R16);
		set_plane(&plan->planes[1], width / 2, height, GS_RG16);
		break;
	case VIDEO_FORMAT_P416:
		set_plane(&plan->planes[0], width, height, GS_R16);
		set_plane(&plan->planes[1], width, height, GS_RG16);
		break;
	default:
		break;
	}
}

/* The render and output textures, and the copy surface without GPU
 * conversion. */
static inline void calc_render_textures(struct obs_video_output_plan *plan, enum video_format format,
					enum video_colorspace colorspace)
{
	enum gs_color_format texture_format = GS_BGRA;
	switch (format) {
	case VIDEO_FORMAT_I010:
	case VIDEO_FORMAT_P010:
	case VIDEO_FORMAT_I210:
	case VIDEO_FORMAT_I412:
	case VIDEO_FORMAT_YA2L:
	case VIDEO_FORMAT_P216:
	case VIDEO_FORMAT_P416:
		texture_format = GS_RGBA16F;
		break;
	default:
		break;
	}

	enum gs_color_space space = GS_CS_SRGB;
	switch (colorspace) {
	case VIDEO_CS_2100_PQ:
	case VIDEO_CS_2100_HLG:
		space = GS_CS_709_EXTENDED;
		break;
	default:
		switch (format) {
		case VIDEO_FORMAT_I010:
		case VIDEO_FORMAT_P010:
		case VIDEO_FORMAT_P216:
		case VIDEO_FORMAT_P416:
			space = GS_CS_SRGB_16F;
			break;
		default:
			space = GS_CS_SRGB;
			break;
		}
		break;
	}

	plan->texture_format = texture_format;
	plan->render_space = space;
}

struct obs_video_output_plan obs_video_output_plan(enum video_format format, enum video_colorspace colorspace,
						   uint32_t width, uint32_t height)
{
	struct obs_video_output_plan plan = {0};

	calc_gpu_conversion_sizes(&plan, format, colorspace, width, height);
	calc_gpu_conversion_planes(&plan, format, width, height);
	calc_render_textures(&plan, format, colorspace);
	return plan;
}
