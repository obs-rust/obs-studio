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

#pragma once

#include <stdbool.h>
#include <stdint.h>

/* The async video GPU conversion plan of obs-source.c, on plain values
 * (obs-source-conversion-plan.c): for an async frame's format, range,
 * transfer and size,
 * - the convert type (get_convert_type());
 * - the planes the frame is uploaded to: their count, widths, heights and
 *   texture formats (init_gpu_conversion() and its set_*_sizes());
 * - the technique of format_conversion.effect that converts them
 *   (select_conversion_technique()), or NULL;
 * - whether the conversion renders into a linear sRGB framebuffer
 *   (need_linear_output()).
 *
 * obs-source.c keeps set_async_texture_size() (the change check, the
 * textures and texrenders, the graphics context), upload_raw_frame(),
 * update_async_texrender() and its shader parameters, and the two asserts
 * for a frame that needs no conversion. init_gpu_conversion() fills a plan
 * and copies it into async_convert_width, async_convert_height,
 * async_texture_formats and async_channel_count. convert_video_format()
 * stays the inline of obs-internal.h; the plan uses it for the RGB limited
 * plane.
 *
 * The function takes no lock, calls into nothing but that inline, allocates
 * nothing and keeps no state. The technique is a string literal (static
 * storage) or NULL.
 *
 * The enums cross as int, so this header needs no graphics or media-io
 * header. obs-source.c checks the sizes.
 *
 * Hidden in libobs: not EXPORT. */

/* The planes a plan can have: async_channel_count is at most 4. */
#define OBS_SOURCE_CONVERSION_PLAN_PLANES 4

/* obs-source.c's enum convert_type, which takes these values. */
enum obs_source_convert_type {
	OBS_SOURCE_CONVERT_NONE,
	OBS_SOURCE_CONVERT_NV12,
	OBS_SOURCE_CONVERT_420,
	OBS_SOURCE_CONVERT_420_PQ,
	OBS_SOURCE_CONVERT_420_A,
	OBS_SOURCE_CONVERT_422,
	OBS_SOURCE_CONVERT_422P10LE,
	OBS_SOURCE_CONVERT_422_A,
	OBS_SOURCE_CONVERT_422_PACK,
	OBS_SOURCE_CONVERT_444,
	OBS_SOURCE_CONVERT_444P12LE,
	OBS_SOURCE_CONVERT_444_A,
	OBS_SOURCE_CONVERT_444P12LE_A,
	OBS_SOURCE_CONVERT_444_A_PACK,
	OBS_SOURCE_CONVERT_800,
	OBS_SOURCE_CONVERT_RGB_LIMITED,
	OBS_SOURCE_CONVERT_BGR3,
	OBS_SOURCE_CONVERT_I010,
	OBS_SOURCE_CONVERT_P010,
	OBS_SOURCE_CONVERT_V210,
	OBS_SOURCE_CONVERT_R10L,
};

/* The plan for one frame. Planes from channel_count on are zero (format 0
 * is GS_UNKNOWN), and with OBS_SOURCE_CONVERT_NONE every plane is. */
struct obs_source_conversion_plan {
	/* an enum obs_source_convert_type */
	int convert_type;
	/* async_channel_count: the planes set, 0 for OBS_SOURCE_CONVERT_NONE */
	int channel_count;
	/* async_convert_width and async_convert_height */
	uint32_t width[OBS_SOURCE_CONVERSION_PLAN_PLANES];
	uint32_t height[OBS_SOURCE_CONVERSION_PLAN_PLANES];
	/* async_texture_formats: enum gs_color_format values */
	int format[OBS_SOURCE_CONVERSION_PLAN_PLANES];
	/* a technique of format_conversion.effect, or NULL */
	const char *technique;
	/* render the conversion into a linear sRGB framebuffer */
	bool linear;
};

/* Fills plan for a frame: format is its enum video_format, trc its
 * enum video_trc (the frame's uint8_t field: the value is taken modulo
 * 256, as the conversion to uint8_t does), width and height its size.
 * Values outside the enums take the C's default paths. */
void obs_source_conversion_plan_fill(int format, bool full_range, int trc, uint32_t width, uint32_t height,
				     struct obs_source_conversion_plan *plan);
