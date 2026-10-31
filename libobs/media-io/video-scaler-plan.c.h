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

#include "libobs/build/libobs-video-scaler-plan.pch.h"

/* The decisions of the swscale scaler (video-scaler-ffmpeg.c), moved: the
 * scale flags, the coefficient set, the range, the plane heights and the
 * plane copy. video-scaler-ffmpeg.c keeps every swscale and libavutil call,
 * the copies and every log. Declared in video-scaler-plan.h without
 * EXPORT. */

int video_scaler_sws_flags(enum video_scale_type type)
{
	switch (type) {
	case VIDEO_SCALE_DEFAULT:
		return VIDEO_SCALER_SWS_FAST_BILINEAR;
	case VIDEO_SCALE_POINT:
		return VIDEO_SCALER_SWS_POINT;
	case VIDEO_SCALE_FAST_BILINEAR:
		return VIDEO_SCALER_SWS_FAST_BILINEAR;
	case VIDEO_SCALE_BILINEAR:
		return VIDEO_SCALER_SWS_BILINEAR | VIDEO_SCALER_SWS_AREA;
	case VIDEO_SCALE_BICUBIC:
		return VIDEO_SCALER_SWS_BICUBIC;
	}

	return VIDEO_SCALER_SWS_POINT;
}

int video_scaler_sws_colorspace(enum video_colorspace cs)
{
	int colorspace = VIDEO_SCALER_SWS_CS_ITU709;

	switch (cs) {
	case VIDEO_CS_DEFAULT:
	case VIDEO_CS_709:
	case VIDEO_CS_SRGB:
	default:
		colorspace = VIDEO_SCALER_SWS_CS_ITU709;
		break;
	case VIDEO_CS_601:
		colorspace = VIDEO_SCALER_SWS_CS_ITU601;
		break;
	case VIDEO_CS_2100_PQ:
	case VIDEO_CS_2100_HLG:
		colorspace = VIDEO_SCALER_SWS_CS_BT2020;
	}

	return colorspace;
}

int video_scaler_sws_range(enum video_range_type type)
{
	switch (type) {
	case VIDEO_RANGE_DEFAULT:
		return 0;
	case VIDEO_RANGE_PARTIAL:
		return 0;
	case VIDEO_RANGE_FULL:
		return 1;
	}

	return 0;
}

void video_scaler_plane_heights(int heights[4], uint32_t height, const int comp_planes[4], const uint8_t *log2_chroma_h)
{
	bool has_plane[4] = {0};
	for (size_t i = 0; i < 4; i++)
		has_plane[comp_planes[i]] = 1;

	heights[0] = (int)height;
	for (size_t i = 1; i < 4; ++i) {
		if (has_plane[i]) {
			const int s = (i == 1 || i == 2) ? *log2_chroma_h : 0;
			heights[i] = (int)(height >> s);
		}
	}
}

bool video_scaler_plane_copy(size_t scaled_linesize, size_t plane_linesize, size_t height, size_t *size)
{
	if (scaled_linesize == plane_linesize) {
		*size = scaled_linesize * height;
		return true;
	}

	size_t linesize = scaled_linesize;
	if (linesize > plane_linesize)
		linesize = plane_linesize;

	*size = linesize;
	return false;
}
