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

/* The GPU rescale mix decisions of obs-encoder.c, on plain values
 * (obs-encoder-gpu-rescale.c). An encoder with a GPU scale type and a scaled
 * size, or a preferred format, colorspace or range, encodes from an
 * encoder-only video mix of the same view:
 * - obs_encoder_gpu_rescale_wanted(): the gates of maybe_set_up_gpu_rescale():
 *   no media, OBS_SCALE_DISABLE, or nothing requested;
 * - obs_encoder_gpu_rescale_target(): the size, format, colorspace and range
 *   the mix must output: the encoder's value, else the video output's;
 * - obs_encoder_gpu_rescale_matches(): whether a mix serves that target: the
 *   same view and scale type, and an output of the target size, format,
 *   colorspace and range. maybe_set_up_gpu_rescale() asks it in its first
 *   search and again after it created a mix;
 * - obs_encoder_gpu_rescale_release_step() and
 *   obs_encoder_gpu_rescale_release_refs(): maybe_clear_encoder_core_video_mix(),
 *   which mix an encoder's shutdown releases, and when the mix's last
 *   reference goes.
 *
 * obs-encoder.c keeps get_mix_for_video(), video_output_get_info(), the
 * mixes array and mixes_mutex, the reads and writes of encoder_refs,
 * original_video and encoder_only_mix, obs_create_video_mix(),
 * obs_free_video_mix(), da_push_back() and da_erase(), and
 * obs_encoder_set_video() / encoder_set_video(). It reads the fields, calls
 * these and does the stores. Nothing here takes a lock, calls into libobs or
 * keeps state.
 *
 * The constants are copied below, so this header needs no core header;
 * obs-encoder.c checks them against obs.h and video-io.h.
 *
 * Hidden in libobs: not EXPORT. */

/* OBS_SCALE_DISABLE (enum obs_scale_type) */
#define OBS_ENCODER_GPU_RESCALE_SCALE_DISABLE 0
/* VIDEO_FORMAT_NONE, VIDEO_CS_DEFAULT, VIDEO_RANGE_DEFAULT */
#define OBS_ENCODER_GPU_RESCALE_FORMAT_NONE 0
#define OBS_ENCODER_GPU_RESCALE_CS_DEFAULT 0
#define OBS_ENCODER_GPU_RESCALE_RANGE_DEFAULT 0

/* What an encoder asks of its rescale: struct obs_encoder's gpu_scale_type,
 * scaled_width, scaled_height, preferred_format, preferred_space and
 * preferred_range. The enums are passed as int. */
struct obs_encoder_gpu_rescale_request {
	int scale_type;
	uint32_t scaled_width;
	uint32_t scaled_height;
	int format;
	int colorspace;
	int range;
};

/* A video output's size, format, colorspace and range (the fields of
 * struct video_output_info that a mix is matched on). */
struct obs_encoder_gpu_rescale_output {
	uint32_t width;
	uint32_t height;
	int format;
	int colorspace;
	int range;
};

/* Which mix maybe_clear_encoder_core_video_mix() releases, for each mix in
 * order. */
enum obs_encoder_gpu_rescale_release {
	/* not the encoder's mix: the next one */
	OBS_ENCODER_GPU_RESCALE_RELEASE_SKIP = 0,
	/* the encoder's mix, but not an encoder-only one: nothing to release */
	OBS_ENCODER_GPU_RESCALE_RELEASE_STOP = 1,
	/* the encoder's encoder-only mix: back to the original video, and the
	 * mix loses one reference */
	OBS_ENCODER_GPU_RESCALE_RELEASE_DROP = 2,
};

/* maybe_set_up_gpu_rescale()'s gates: false when the encoder has no media,
 * its scale type is OBS_SCALE_DISABLE, or it asks for no scaled size and no
 * preferred format, colorspace or range. */
bool obs_encoder_gpu_rescale_wanted(bool has_media, const struct obs_encoder_gpu_rescale_request *request);

/* The output the encoder's mix must have: each field the request's when it
 * is set (a scaled width or height other than 0; a format, colorspace or
 * range other than the "none" or "default" value), else video's. */
struct obs_encoder_gpu_rescale_output
obs_encoder_gpu_rescale_target(const struct obs_encoder_gpu_rescale_request *request,
			       const struct obs_encoder_gpu_rescale_output *video);

/* A mix serves the target when it belongs to the same view (same_view), has
 * the encoder's scale type, and its video output has the target's size,
 * format, colorspace and range. */
bool obs_encoder_gpu_rescale_matches(bool same_view, int mix_scale_type,
				     const struct obs_encoder_gpu_rescale_output *mix, int scale_type,
				     const struct obs_encoder_gpu_rescale_output *target);

/* For one mix of maybe_clear_encoder_core_video_mix(): is_media when the mix's
 * video is the encoder's media, encoder_only the mix's encoder_only_mix. */
enum obs_encoder_gpu_rescale_release obs_encoder_gpu_rescale_release_step(bool is_media, bool encoder_only);

/* A dropped reference: the mix's encoder_refs after it, and in *last
 * whether that was the last one (the mix is erased and freed). */
long obs_encoder_gpu_rescale_release_refs(long encoder_refs, bool *last);
