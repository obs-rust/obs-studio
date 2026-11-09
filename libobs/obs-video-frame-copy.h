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

#include "media-io/video-io.h"

/* Private adapter, not a new public API: the output frame copy of
 * obs-video.c (obs-video-frame-copy.c), declared without EXPORT so
 * output_video_data() can call it from another file.
 * - obs_video_set_gpu_converted_data(): set_gpu_converted_data(), every copy
 *   of the GPU conversion plan (obs_video_gpu_copy_plan()) from the mapped
 *   staging surfaces into the locked output frame;
 * - obs_video_copy_rgbx_frame(): copy_rgbx_frame(), the one plane of an RGBA,
 *   BGRA or BGRX frame (obs_video_rgbx_copy_plan()). */

#ifdef __cplusplus
extern "C" {
#endif

void obs_video_set_gpu_converted_data(struct video_frame *output, const struct video_data *input,
				      const struct video_output_info *info);
void obs_video_copy_rgbx_frame(struct video_frame *output, const struct video_data *input,
			       const struct video_output_info *info);

#ifdef __cplusplus
}
#endif
