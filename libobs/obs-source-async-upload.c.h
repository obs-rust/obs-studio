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

#include "libobs/build/libobs-ported-source-async-upload.pch.h"

/* The async video upload decisions of obs-source.c, moved from
 * set_async_texture_size(), update_async_texrender(),
 * update_async_textures(), check_to_swap_bgrx_bgra() and
 * obs_source_update_async_video(): the decisions and the shader parameter
 * values, on plain values. obs-source.c reads the fields and the levels,
 * calls these and keeps every gs_* call, get_convert_type(),
 * init_gpu_conversion() and log10f(). Declared in obs-source-async-upload.h
 * without EXPORT. In hybrid libobs the Rust port (rust/obs-source,
 * async_upload) replaces this file. */

bool obs_source_async_upload_texture_changed(struct obs_source_async_upload_format cur,
					     struct obs_source_async_upload_format frame)
{
	if (cur.width == frame.width && cur.height == frame.height && cur.format == frame.format &&
	    cur.full_range == frame.full_range && cur.trc == frame.trc)
		return false;

	return true;
}

bool obs_source_async_upload_tries_gpu(int convert_type)
{
	return convert_type != OBS_SOURCE_CONVERT_NONE;
}

struct obs_source_async_upload_size obs_source_async_upload_size_uniforms(uint32_t cx, uint32_t cy)
{
	struct obs_source_async_upload_size size;

	size.width = (float)cx;
	size.height = (float)cy;
	size.width_d2 = (float)cx * 0.5f;
	size.height_d2 = (float)cy * 0.5f;
	size.width_x2_i = 0.5f / (float)cx;
	size.height_x2_i = 0.5f / (float)cy;
	return size;
}

float obs_source_async_upload_hlg_peak(float hdr_nominal_peak_level)
{
	/* BT.2408 says higher than 1000 isn't comfortable */
	float hlg_peak_level = hdr_nominal_peak_level;
	if (hlg_peak_level > 1000.f)
		hlg_peak_level = 1000.f;

	return hlg_peak_level;
}

float obs_source_async_upload_maximum_nits(uint8_t trc, float hlg_peak_level)
{
	return (trc == VIDEO_TRC_HLG) ? hlg_peak_level : 10000.f;
}

float obs_source_async_upload_over_sdr_white(float maximum_nits, float sdr_white_level)
{
	return maximum_nits / sdr_white_level;
}

float obs_source_async_upload_hdr_lw(uint16_t max_luminance)
{
	return (float)max_luminance;
}

bool obs_source_async_upload_sets_range(bool full_range)
{
	return !full_range;
}

bool obs_source_async_upload_linear_alpha(uint8_t flags)
{
	return (flags & OBS_SOURCE_FRAME_LINEAR_ALPHA) != 0;
}

bool obs_source_async_upload_uses_texrender(bool gpu_conversion, bool has_texrender)
{
	return gpu_conversion && has_texrender;
}

bool obs_source_async_upload_plain(int convert_type)
{
	return convert_type == OBS_SOURCE_CONVERT_NONE;
}

int obs_source_async_upload_bgr_swap(int texture_format, int frame_format)
{
	if (texture_format == GS_BGRX && frame_format == VIDEO_FORMAT_BGRA) {
		return GS_BGRA;
	} else if (texture_format == GS_BGRA && frame_format == VIDEO_FORMAT_BGRX) {
		return GS_BGRX;
	}

	return GS_UNKNOWN;
}

bool obs_source_async_upload_reanchor_reads_unbuffered(bool decoupled)
{
	return decoupled;
}

bool obs_source_async_upload_reanchor(bool decoupled, bool unbuffered)
{
	return !decoupled || !unbuffered;
}
