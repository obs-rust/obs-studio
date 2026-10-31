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
#include <stddef.h>
#include <stdint.h>

#include "media-io/video-io.h"

/* The decisions of the swscale scaler (video-scaler-ffmpeg.c), on plain
 * values (video-scaler-plan.c):
 * - video_scaler_sws_flags(): the swscale flags for a scale type;
 * - video_scaler_sws_colorspace(): the swscale coefficient set for a
 *   colorspace;
 * - video_scaler_sws_range(): the swscale range for a range type;
 * - video_scaler_plane_heights(): the height of each plane the destination
 *   format has;
 * - video_scaler_plane_copy(): how one scaled plane is copied to the output.
 *
 * video-scaler-ffmpeg.c keeps every swscale and libavutil call, the pixel
 * format table and the descriptor, the copies themselves and every log.
 * Nothing here includes an FFmpeg header: the swscale values are given as
 * their values, which video-scaler-ffmpeg.c checks against libswscale's. Not
 * EXPORT: only libobs calls these. */

/* The swscale flags (SWS_*) the scaler uses. */
#define VIDEO_SCALER_SWS_FAST_BILINEAR 0x01
#define VIDEO_SCALER_SWS_BILINEAR 0x02
#define VIDEO_SCALER_SWS_BICUBIC 0x04
#define VIDEO_SCALER_SWS_POINT 0x10
#define VIDEO_SCALER_SWS_AREA 0x20

/* The swscale coefficient sets (SWS_CS_*) the scaler uses. */
#define VIDEO_SCALER_SWS_CS_ITU709 1
#define VIDEO_SCALER_SWS_CS_ITU601 5
#define VIDEO_SCALER_SWS_CS_BT2020 9

int video_scaler_sws_flags(enum video_scale_type type);
int video_scaler_sws_colorspace(enum video_colorspace cs);
int video_scaler_sws_range(enum video_range_type type);

/* The heights of the planes the destination format has, from the plane of
 * each of its four components (AVComponentDescriptor.plane, 0 to 3) and its
 * vertical chroma shift (AVPixFmtDescriptor.log2_chroma_h), read only for a
 * format with plane 1 or 2. heights[0] is always written; a plane the format
 * does not have is left as it is. */
void video_scaler_plane_heights(int heights[4], uint32_t height, const int comp_planes[4],
				const uint8_t *log2_chroma_h);

/* The copy of a scaled plane of height rows to an output plane: true for one
 * copy of *size bytes (the line sizes match), false for height copies of
 * *size bytes, one per row, each line size apart. */
bool video_scaler_plane_copy(size_t scaled_linesize, size_t plane_linesize, size_t height, size_t *size);
