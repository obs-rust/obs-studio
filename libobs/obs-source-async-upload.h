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

/* The async video upload decisions of obs-source.c, on plain values
 * (obs-source-async-upload.c):
 * - set_async_texture_size(): whether a frame's size, format, range or
 *   transfer differs from the textures', so they are rebuilt, and whether
 *   the rebuild tries a GPU conversion;
 * - update_async_texrender(): the shader parameters of the conversion draw:
 *   the size uniforms, the HLG peak clamp (BT.2408), the maximum nits over
 *   the SDR white level, hdr_lw, and whether the colour range is set;
 * - update_async_textures(): whether the frame's alpha is linear, and
 *   whether it is drawn through the texrender, uploaded as one plain
 *   texture, or neither;
 * - check_to_swap_bgrx_bgra(): the format the texture is recreated with
 *   when a BGRX texture gets a BGRA frame or the other way round;
 * - obs_source_update_async_video(): whether the source's timing re-anchors
 *   on the frame.
 *
 * obs-source.c keeps every gs_* call in its order and under the same
 * conditions (gs_texrender_begin(), each gs_effect_get_param_by_name() and
 * gs_effect_set_*(), the textures and texrenders, the graphics context),
 * get_convert_type(), init_gpu_conversion(), log10f() and the HLG exponent,
 * obs->video.hdr_nominal_peak_level (read directly),
 * obs_get_video_sdr_white_level(), obs_get_video_hdr_nominal_peak_level(),
 * obs_source_get_frame() and obs_source_release_frame(). It reads each value
 * where it did before, passes it in and acts on what comes back: a level is
 * read only once gs_texrender_begin() has succeeded, in the order it was.
 * None of these functions takes a lock, calls into libobs or keeps state.
 *
 * Every float is single precision and computed with the operations
 * update_async_texrender() used, in its order: (float)cx * 0.5f,
 * 0.5f / (float)cx, maximum_nits / sdr_white_level. Nothing divides an
 * integer.
 *
 * The enums cross as int (video_format, gs_color_format and
 * obs_source_convert_type) or as the frame's uint8_t (video_trc), so this
 * header needs no graphics or media-io header. obs-source.c checks the
 * sizes.
 *
 * Hidden in libobs: not EXPORT. */

/* The values set_async_texture_size() compares: the frame's, or the
 * source's async_width, async_height, async_format, async_full_range and
 * async_trc. */
struct obs_source_async_upload_format {
	uint32_t width;
	uint32_t height;
	/* an enum video_format */
	int format;
	bool full_range;
	/* an enum video_trc */
	uint8_t trc;
};

/* Whether a frame's values differ from the textures' (cur) in any of the
 * five fields, so set_async_texture_size() stores them and rebuilds the
 * textures; false returns true at once. */
bool obs_source_async_upload_texture_changed(struct obs_source_async_upload_format cur,
					     struct obs_source_async_upload_format frame);

/* Whether the rebuild tries init_gpu_conversion() for a frame of this
 * convert type (an enum obs_source_convert_type, from get_convert_type()):
 * any but OBS_SOURCE_CONVERT_NONE. Otherwise, or when the plan fails, the
 * frame gets one plain texture. */
bool obs_source_async_upload_tries_gpu(int convert_type);

/* The size uniforms of the conversion draw for an async_width x
 * async_height texture: width and height as (float)uint32_t, their halves
 * ((float)cx * 0.5f) and the half texel (0.5f / (float)cx: +inf for 0). */
struct obs_source_async_upload_size {
	float width;
	float height;
	float width_d2;
	float height_d2;
	float width_x2_i;
	float height_x2_i;
};

struct obs_source_async_upload_size obs_source_async_upload_size_uniforms(uint32_t cx, uint32_t cy);

/* The HLG peak level from obs->video.hdr_nominal_peak_level: clamped to
 * 1000, as BT.2408 says higher isn't comfortable. A NaN stays a NaN (the
 * C's > is false). */
float obs_source_async_upload_hlg_peak(float hdr_nominal_peak_level);

/* The maximum nits of a frame with this transfer (the frame's uint8_t trc):
 * the HLG peak level for VIDEO_TRC_HLG, 10000 for any other value. */
float obs_source_async_upload_maximum_nits(uint8_t trc, float hlg_peak_level);

/* maximum_over_sdr_white_nits: maximum_nits / sdr_white_level, one float
 * division (obs_set_video_levels() keeps the level finite and above 0,
 * #2209). */
float obs_source_async_upload_over_sdr_white(float maximum_nits, float sdr_white_level);

/* hdr_lw: the frame's max_luminance as a float. */
float obs_source_async_upload_hdr_lw(uint16_t max_luminance);

/* Whether the draw sets color_range_min and color_range_max: for a limited
 * range frame only. */
bool obs_source_async_upload_sets_range(bool full_range);

/* async_linear_alpha: whether the frame's flags carry
 * OBS_SOURCE_FRAME_LINEAR_ALPHA. */
bool obs_source_async_upload_linear_alpha(uint8_t flags);

/* Whether update_async_textures() draws the frame through the texrender: a
 * GPU conversion (async_gpu_conversion) and a texrender to draw into. */
bool obs_source_async_upload_uses_texrender(bool gpu_conversion, bool has_texrender);

/* Otherwise, whether it uploads the frame's first plane as one plain
 * texture and returns true: a frame of convert type OBS_SOURCE_CONVERT_NONE.
 * Any other type uploads nothing and returns false. */
bool obs_source_async_upload_plain(int convert_type);

/* The format check_to_swap_bgrx_bgra() recreates the texture with
 * (texture_format, an enum gs_color_format from
 * gs_texture_get_color_format(), for a frame of frame_format, an enum
 * video_format): GS_BGRA for a GS_BGRX texture and a VIDEO_FORMAT_BGRA
 * frame, GS_BGRX for a GS_BGRA texture and a VIDEO_FORMAT_BGRX frame,
 * GS_UNKNOWN (keep the texture) for any other pair. */
int obs_source_async_upload_bgr_swap(int texture_format, int frame_format);

/* Whether obs_source_update_async_video() reads async_unbuffered to decide
 * the re-anchor: only for a decoupled source. */
bool obs_source_async_upload_reanchor_reads_unbuffered(bool decoupled);

/* Whether the source's timing re-anchors on the frame (timing_adjust and
 * timing_set): unless it is decoupled and unbuffered. unbuffered is not
 * read for a source that is not decoupled. */
bool obs_source_async_upload_reanchor(bool decoupled, bool unbuffered);
