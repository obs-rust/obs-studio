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

#include "libobs/build/libobs-ported-source-conversion-plan.pch.h"

/* The async video GPU conversion plan, moved from obs-source.c:
 * get_convert_type(), the set_*_sizes() of init_gpu_conversion(),
 * select_conversion_technique() and need_linear_output(), on plain values.
 * The setters write a plan instead of the source; obs-source.c copies it
 * into the source and keeps the asserts for a frame that needs no
 * conversion. Declared in obs-source-conversion-plan.h without EXPORT. In
 * hybrid libobs the Rust port (rust/obs-source) replaces this file. */

_Static_assert(sizeof(enum video_format) == sizeof(int), "enum video_format is passed as int");
_Static_assert(sizeof(enum gs_color_format) == sizeof(int), "enum gs_color_format is passed as int");

/* The fields of struct obs_source_frame the plan reads, with their types. */
struct obs_source_conversion_frame {
	enum video_format format;
	bool full_range;
	uint8_t trc; /* enum video_trc */
	uint32_t width;
	uint32_t height;
};

static inline enum obs_source_convert_type conversion_plan_convert_type(enum video_format format, bool full_range,
									uint8_t trc)
{
	switch (format) {
	case VIDEO_FORMAT_I420:
		return (trc == VIDEO_TRC_PQ) ? OBS_SOURCE_CONVERT_420_PQ : OBS_SOURCE_CONVERT_420;
	case VIDEO_FORMAT_NV12:
		return OBS_SOURCE_CONVERT_NV12;
	case VIDEO_FORMAT_I444:
		return OBS_SOURCE_CONVERT_444;
	case VIDEO_FORMAT_I412:
		return OBS_SOURCE_CONVERT_444P12LE;
	case VIDEO_FORMAT_I422:
		return OBS_SOURCE_CONVERT_422;
	case VIDEO_FORMAT_I210:
		return OBS_SOURCE_CONVERT_422P10LE;

	case VIDEO_FORMAT_YVYU:
	case VIDEO_FORMAT_YUY2:
	case VIDEO_FORMAT_UYVY:
		return OBS_SOURCE_CONVERT_422_PACK;

	case VIDEO_FORMAT_Y800:
		return OBS_SOURCE_CONVERT_800;

	case VIDEO_FORMAT_NONE:
	case VIDEO_FORMAT_RGBA:
	case VIDEO_FORMAT_BGRA:
	case VIDEO_FORMAT_BGRX:
		return full_range ? OBS_SOURCE_CONVERT_NONE : OBS_SOURCE_CONVERT_RGB_LIMITED;

	case VIDEO_FORMAT_BGR3:
		return OBS_SOURCE_CONVERT_BGR3;

	case VIDEO_FORMAT_I40A:
		return OBS_SOURCE_CONVERT_420_A;

	case VIDEO_FORMAT_I42A:
		return OBS_SOURCE_CONVERT_422_A;

	case VIDEO_FORMAT_YUVA:
		return OBS_SOURCE_CONVERT_444_A;

	case VIDEO_FORMAT_YA2L:
		return OBS_SOURCE_CONVERT_444P12LE_A;

	case VIDEO_FORMAT_AYUV:
		return OBS_SOURCE_CONVERT_444_A_PACK;

	case VIDEO_FORMAT_I010:
		return OBS_SOURCE_CONVERT_I010;

	case VIDEO_FORMAT_P010:
		return OBS_SOURCE_CONVERT_P010;

	case VIDEO_FORMAT_V210:
		return OBS_SOURCE_CONVERT_V210;

	case VIDEO_FORMAT_R10L:
		return OBS_SOURCE_CONVERT_R10L;

	case VIDEO_FORMAT_P216:
	case VIDEO_FORMAT_P416:
		/* Unimplemented */
		break;
	}

	return OBS_SOURCE_CONVERT_NONE;
}

static inline bool set_packed422_sizes(struct obs_source_conversion_plan *plan,
				       const struct obs_source_conversion_frame *frame)
{
	const uint32_t width = frame->width;
	const uint32_t height = frame->height;
	const uint32_t half_width = (width + 1) / 2;
	plan->width[0] = half_width;
	plan->height[0] = height;
	plan->format[0] = GS_BGRA;
	plan->channel_count = 1;
	return true;
}

static inline bool set_packed444_alpha_sizes(struct obs_source_conversion_plan *plan,
					     const struct obs_source_conversion_frame *frame)
{
	plan->width[0] = frame->width;
	plan->height[0] = frame->height;
	plan->format[0] = GS_BGRA;
	plan->channel_count = 1;
	return true;
}

static inline bool set_planar444_sizes(struct obs_source_conversion_plan *plan,
				       const struct obs_source_conversion_frame *frame)
{
	plan->width[0] = frame->width;
	plan->width[1] = frame->width;
	plan->width[2] = frame->width;
	plan->height[0] = frame->height;
	plan->height[1] = frame->height;
	plan->height[2] = frame->height;
	plan->format[0] = GS_R8;
	plan->format[1] = GS_R8;
	plan->format[2] = GS_R8;
	plan->channel_count = 3;
	return true;
}

static inline bool set_planar444_16_sizes(struct obs_source_conversion_plan *plan,
					  const struct obs_source_conversion_frame *frame)
{
	plan->width[0] = frame->width;
	plan->width[1] = frame->width;
	plan->width[2] = frame->width;
	plan->height[0] = frame->height;
	plan->height[1] = frame->height;
	plan->height[2] = frame->height;
	plan->format[0] = GS_R16;
	plan->format[1] = GS_R16;
	plan->format[2] = GS_R16;
	plan->channel_count = 3;
	return true;
}

static inline bool set_planar444_alpha_sizes(struct obs_source_conversion_plan *plan,
					     const struct obs_source_conversion_frame *frame)
{
	plan->width[0] = frame->width;
	plan->width[1] = frame->width;
	plan->width[2] = frame->width;
	plan->width[3] = frame->width;
	plan->height[0] = frame->height;
	plan->height[1] = frame->height;
	plan->height[2] = frame->height;
	plan->height[3] = frame->height;
	plan->format[0] = GS_R8;
	plan->format[1] = GS_R8;
	plan->format[2] = GS_R8;
	plan->format[3] = GS_R8;
	plan->channel_count = 4;
	return true;
}

static inline bool set_planar444_16_alpha_sizes(struct obs_source_conversion_plan *plan,
						const struct obs_source_conversion_frame *frame)
{
	plan->width[0] = frame->width;
	plan->width[1] = frame->width;
	plan->width[2] = frame->width;
	plan->width[3] = frame->width;
	plan->height[0] = frame->height;
	plan->height[1] = frame->height;
	plan->height[2] = frame->height;
	plan->height[3] = frame->height;
	plan->format[0] = GS_R16;
	plan->format[1] = GS_R16;
	plan->format[2] = GS_R16;
	plan->format[3] = GS_R16;
	plan->channel_count = 4;
	return true;
}

static inline bool set_planar420_sizes(struct obs_source_conversion_plan *plan,
				       const struct obs_source_conversion_frame *frame)
{
	const uint32_t width = frame->width;
	const uint32_t height = frame->height;
	const uint32_t half_width = (width + 1) / 2;
	const uint32_t half_height = (height + 1) / 2;
	plan->width[0] = width;
	plan->width[1] = half_width;
	plan->width[2] = half_width;
	plan->height[0] = height;
	plan->height[1] = half_height;
	plan->height[2] = half_height;
	plan->format[0] = GS_R8;
	plan->format[1] = GS_R8;
	plan->format[2] = GS_R8;
	plan->channel_count = 3;
	return true;
}

static inline bool set_planar420_alpha_sizes(struct obs_source_conversion_plan *plan,
					     const struct obs_source_conversion_frame *frame)
{
	const uint32_t width = frame->width;
	const uint32_t height = frame->height;
	const uint32_t half_width = (width + 1) / 2;
	const uint32_t half_height = (height + 1) / 2;
	plan->width[0] = width;
	plan->width[1] = half_width;
	plan->width[2] = half_width;
	plan->width[3] = width;
	plan->height[0] = height;
	plan->height[1] = half_height;
	plan->height[2] = half_height;
	plan->height[3] = height;
	plan->format[0] = GS_R8;
	plan->format[1] = GS_R8;
	plan->format[2] = GS_R8;
	plan->format[3] = GS_R8;
	plan->channel_count = 4;
	return true;
}

static inline bool set_planar422_sizes(struct obs_source_conversion_plan *plan,
				       const struct obs_source_conversion_frame *frame)
{
	const uint32_t width = frame->width;
	const uint32_t height = frame->height;
	const uint32_t half_width = (width + 1) / 2;
	plan->width[0] = width;
	plan->width[1] = half_width;
	plan->width[2] = half_width;
	plan->height[0] = height;
	plan->height[1] = height;
	plan->height[2] = height;
	plan->format[0] = GS_R8;
	plan->format[1] = GS_R8;
	plan->format[2] = GS_R8;
	plan->channel_count = 3;
	return true;
}
static inline bool set_planar422_16_sizes(struct obs_source_conversion_plan *plan,
					  const struct obs_source_conversion_frame *frame)
{
	const uint32_t width = frame->width;
	const uint32_t height = frame->height;
	const uint32_t half_width = (width + 1) / 2;
	plan->width[0] = width;
	plan->width[1] = half_width;
	plan->width[2] = half_width;
	plan->height[0] = height;
	plan->height[1] = height;
	plan->height[2] = height;
	plan->format[0] = GS_R16;
	plan->format[1] = GS_R16;
	plan->format[2] = GS_R16;
	plan->channel_count = 3;
	return true;
}

static inline bool set_planar422_alpha_sizes(struct obs_source_conversion_plan *plan,
					     const struct obs_source_conversion_frame *frame)
{
	const uint32_t width = frame->width;
	const uint32_t height = frame->height;
	const uint32_t half_width = (width + 1) / 2;
	plan->width[0] = width;
	plan->width[1] = half_width;
	plan->width[2] = half_width;
	plan->width[3] = width;
	plan->height[0] = height;
	plan->height[1] = height;
	plan->height[2] = height;
	plan->height[3] = height;
	plan->format[0] = GS_R8;
	plan->format[1] = GS_R8;
	plan->format[2] = GS_R8;
	plan->format[3] = GS_R8;
	plan->channel_count = 4;
	return true;
}

static inline bool set_nv12_sizes(struct obs_source_conversion_plan *plan,
				  const struct obs_source_conversion_frame *frame)
{
	const uint32_t width = frame->width;
	const uint32_t height = frame->height;
	const uint32_t half_width = (width + 1) / 2;
	const uint32_t half_height = (height + 1) / 2;
	plan->width[0] = width;
	plan->width[1] = half_width;
	plan->height[0] = height;
	plan->height[1] = half_height;
	plan->format[0] = GS_R8;
	plan->format[1] = GS_R8G8;
	plan->channel_count = 2;
	return true;
}

static inline bool set_y800_sizes(struct obs_source_conversion_plan *plan,
				  const struct obs_source_conversion_frame *frame)
{
	plan->width[0] = frame->width;
	plan->height[0] = frame->height;
	plan->format[0] = GS_R8;
	plan->channel_count = 1;
	return true;
}

static inline bool set_rgb_limited_sizes(struct obs_source_conversion_plan *plan,
					 const struct obs_source_conversion_frame *frame)
{
	plan->width[0] = frame->width;
	plan->height[0] = frame->height;
	plan->format[0] = (int)convert_video_format(frame->format, frame->trc);
	plan->channel_count = 1;
	return true;
}

static inline bool set_bgr3_sizes(struct obs_source_conversion_plan *plan,
				  const struct obs_source_conversion_frame *frame)
{
	plan->width[0] = frame->width * 3;
	plan->height[0] = frame->height;
	plan->format[0] = GS_R8;
	plan->channel_count = 1;
	return true;
}

static inline bool set_i010_sizes(struct obs_source_conversion_plan *plan,
				  const struct obs_source_conversion_frame *frame)
{
	const uint32_t width = frame->width;
	const uint32_t height = frame->height;
	const uint32_t half_width = (width + 1) / 2;
	const uint32_t half_height = (height + 1) / 2;
	plan->width[0] = width;
	plan->width[1] = half_width;
	plan->width[2] = half_width;
	plan->height[0] = height;
	plan->height[1] = half_height;
	plan->height[2] = half_height;
	plan->format[0] = GS_R16;
	plan->format[1] = GS_R16;
	plan->format[2] = GS_R16;
	plan->channel_count = 3;
	return true;
}

static inline bool set_p010_sizes(struct obs_source_conversion_plan *plan,
				  const struct obs_source_conversion_frame *frame)
{
	const uint32_t width = frame->width;
	const uint32_t height = frame->height;
	const uint32_t half_width = (width + 1) / 2;
	const uint32_t half_height = (height + 1) / 2;
	plan->width[0] = width;
	plan->width[1] = half_width;
	plan->height[0] = height;
	plan->height[1] = half_height;
	plan->format[0] = GS_R16;
	plan->format[1] = GS_RG16;
	plan->channel_count = 2;
	return true;
}

static inline bool set_v210_sizes(struct obs_source_conversion_plan *plan,
				  const struct obs_source_conversion_frame *frame)
{
	const uint32_t width = frame->width;
	const uint32_t height = frame->height;
	const uint32_t adjusted_width = ((width + 5) / 6) * 4;
	plan->width[0] = adjusted_width;
	plan->height[0] = height;
	plan->format[0] = GS_R10G10B10A2;
	plan->channel_count = 1;
	return true;
}

static inline bool set_r10l_sizes(struct obs_source_conversion_plan *plan,
				  const struct obs_source_conversion_frame *frame)
{
	plan->width[0] = frame->width;
	plan->height[0] = frame->height;
	plan->format[0] = GS_BGRA_UNORM;
	plan->channel_count = 1;
	return true;
}

static inline bool conversion_plan_sizes(struct obs_source_conversion_plan *plan,
					 const struct obs_source_conversion_frame *frame)
{
	switch (conversion_plan_convert_type(frame->format, frame->full_range, frame->trc)) {
	case OBS_SOURCE_CONVERT_422_PACK:
		return set_packed422_sizes(plan, frame);

	case OBS_SOURCE_CONVERT_420:
	case OBS_SOURCE_CONVERT_420_PQ:
		return set_planar420_sizes(plan, frame);

	case OBS_SOURCE_CONVERT_422:
		return set_planar422_sizes(plan, frame);

	case OBS_SOURCE_CONVERT_422P10LE:
		return set_planar422_16_sizes(plan, frame);

	case OBS_SOURCE_CONVERT_NV12:
		return set_nv12_sizes(plan, frame);

	case OBS_SOURCE_CONVERT_444:
		return set_planar444_sizes(plan, frame);

	case OBS_SOURCE_CONVERT_444P12LE:
		return set_planar444_16_sizes(plan, frame);

	case OBS_SOURCE_CONVERT_800:
		return set_y800_sizes(plan, frame);

	case OBS_SOURCE_CONVERT_RGB_LIMITED:
		return set_rgb_limited_sizes(plan, frame);

	case OBS_SOURCE_CONVERT_BGR3:
		return set_bgr3_sizes(plan, frame);

	case OBS_SOURCE_CONVERT_420_A:
		return set_planar420_alpha_sizes(plan, frame);

	case OBS_SOURCE_CONVERT_422_A:
		return set_planar422_alpha_sizes(plan, frame);

	case OBS_SOURCE_CONVERT_444_A:
		return set_planar444_alpha_sizes(plan, frame);

	case OBS_SOURCE_CONVERT_444P12LE_A:
		return set_planar444_16_alpha_sizes(plan, frame);

	case OBS_SOURCE_CONVERT_444_A_PACK:
		return set_packed444_alpha_sizes(plan, frame);

	case OBS_SOURCE_CONVERT_I010:
		return set_i010_sizes(plan, frame);

	case OBS_SOURCE_CONVERT_P010:
		return set_p010_sizes(plan, frame);

	case OBS_SOURCE_CONVERT_V210:
		return set_v210_sizes(plan, frame);

	case OBS_SOURCE_CONVERT_R10L:
		return set_r10l_sizes(plan, frame);

	case OBS_SOURCE_CONVERT_NONE:
		/* obs-source.c asserts */
		break;
	}
	return false;
}

static const char *conversion_plan_technique(enum video_format format, bool full_range, uint8_t trc)
{
	switch (format) {
	case VIDEO_FORMAT_UYVY:
		return "UYVY_Reverse";

	case VIDEO_FORMAT_YUY2:
		switch (trc) {
		case VIDEO_TRC_PQ:
			return "YUY2_PQ_Reverse";
		case VIDEO_TRC_HLG:
			return "YUY2_HLG_Reverse";
		default:
			return "YUY2_Reverse";
		}

	case VIDEO_FORMAT_YVYU:
		return "YVYU_Reverse";

	case VIDEO_FORMAT_I420:
		switch (trc) {
		case VIDEO_TRC_PQ:
			return "I420_PQ_Reverse";
		case VIDEO_TRC_HLG:
			return "I420_HLG_Reverse";
		default:
			return "I420_Reverse";
		}

	case VIDEO_FORMAT_NV12:
		switch (trc) {
		case VIDEO_TRC_PQ:
			return "NV12_PQ_Reverse";
		case VIDEO_TRC_HLG:
			return "NV12_HLG_Reverse";
		default:
			return "NV12_Reverse";
		}

	case VIDEO_FORMAT_I444:
		return "I444_Reverse";

	case VIDEO_FORMAT_I412:
		switch (trc) {
		case VIDEO_TRC_PQ:
			return "I412_PQ_Reverse";
		case VIDEO_TRC_HLG:
			return "I412_HLG_Reverse";
		default:
			return "I412_Reverse";
		}

	case VIDEO_FORMAT_Y800:
		return full_range ? "Y800_Full" : "Y800_Limited";

	case VIDEO_FORMAT_BGR3:
		return full_range ? "BGR3_Full" : "BGR3_Limited";

	case VIDEO_FORMAT_I422:
		return "I422_Reverse";

	case VIDEO_FORMAT_I210:
		switch (trc) {
		case VIDEO_TRC_PQ:
			return "I210_PQ_Reverse";
		case VIDEO_TRC_HLG:
			return "I210_HLG_Reverse";
		default:
			return "I210_Reverse";
		}

	case VIDEO_FORMAT_I40A:
		return "I40A_Reverse";

	case VIDEO_FORMAT_I42A:
		return "I42A_Reverse";

	case VIDEO_FORMAT_YUVA:
		return "YUVA_Reverse";

	case VIDEO_FORMAT_YA2L:
		return "YA2L_Reverse";

	case VIDEO_FORMAT_AYUV:
		return "AYUV_Reverse";

	case VIDEO_FORMAT_I010: {
		switch (trc) {
		case VIDEO_TRC_PQ:
			return "I010_PQ_2020_709_Reverse";
		case VIDEO_TRC_HLG:
			return "I010_HLG_2020_709_Reverse";
		default:
			return "I010_SRGB_Reverse";
		}
	}

	case VIDEO_FORMAT_P010: {
		switch (trc) {
		case VIDEO_TRC_PQ:
			return "P010_PQ_2020_709_Reverse";
		case VIDEO_TRC_HLG:
			return "P010_HLG_2020_709_Reverse";
		default:
			return "P010_SRGB_Reverse";
		}
	}

	case VIDEO_FORMAT_V210: {
		switch (trc) {
		case VIDEO_TRC_PQ:
			return "V210_PQ_2020_709_Reverse";
		case VIDEO_TRC_HLG:
			return "V210_HLG_2020_709_Reverse";
		default:
			return "V210_SRGB_Reverse";
		}
	}

	case VIDEO_FORMAT_R10L: {
		switch (trc) {
		case VIDEO_TRC_PQ:
			return full_range ? "R10L_PQ_2020_709_Full_Reverse" : "R10L_PQ_2020_709_Limited_Reverse";
		case VIDEO_TRC_HLG:
			return full_range ? "R10L_HLG_2020_709_Full_Reverse" : "R10L_HLG_2020_709_Limited_Reverse";
		default:
			return full_range ? "R10L_SRGB_Full_Reverse" : "R10L_SRGB_Limited_Reverse";
		}
	}

	case VIDEO_FORMAT_BGRA:
	case VIDEO_FORMAT_BGRX:
	case VIDEO_FORMAT_RGBA:
	case VIDEO_FORMAT_NONE:
		/* full range: obs-source.c asserts */
		if (!full_range)
			return "RGB_Limited";
		break;

	case VIDEO_FORMAT_P216:
	case VIDEO_FORMAT_P416:
		/* Unimplemented */
		break;
	}
	return NULL;
}

static bool conversion_plan_linear(enum video_format format)
{
	return (format == VIDEO_FORMAT_I010) || (format == VIDEO_FORMAT_P010) || (format == VIDEO_FORMAT_I210) ||
	       (format == VIDEO_FORMAT_I412) || (format == VIDEO_FORMAT_YA2L);
}

void obs_source_conversion_plan_fill(int format, bool full_range, int trc, uint32_t width, uint32_t height,
				     struct obs_source_conversion_plan *plan)
{
	const struct obs_source_conversion_frame frame = {
		.format = (enum video_format)format,
		.full_range = full_range,
		.trc = (uint8_t)trc,
		.width = width,
		.height = height,
	};

	memset(plan, 0, sizeof(*plan));
	plan->convert_type = (int)conversion_plan_convert_type(frame.format, frame.full_range, frame.trc);
	conversion_plan_sizes(plan, &frame);
	plan->technique = conversion_plan_technique(frame.format, frame.full_range, frame.trc);
	plan->linear = conversion_plan_linear(frame.format);
}
