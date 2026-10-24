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

/* The output frame copy plan and the scale effect choice of obs-video.c, on
 * plain values (obs-video-copy-plan.c). Each one is made where the C made it,
 * from values the caller reads at that point:
 * - obs_video_gpu_copy_plan(): the copies of set_gpu_converted_data(), one
 *   per plane of the GPU-converted frame: for each, the bytes of the one copy
 *   or of each row, the rows and the source and destination strides. The
 *   UV plane of a packed NV12 or P010 frame (input linesize[1] of 0) follows
 *   plane 0 in the same buffer;
 * - obs_video_rgbx_copy_plan(): the copy of copy_rgbx_frame(): one copy when
 *   the line sizes match, else rows of width * 4 bytes;
 * - obs_video_scale_resolution_close(): resolution_close(), whether the
 *   output texture is within 16 pixels of the base size on both axes;
 * - obs_video_scale_effect_choice(): get_scale_effect(): which scale effect
 *   render_output_texture() draws with, with the low resolution rule, the
 *   scale type table and the fallback when an effect did not load;
 * - obs_video_mix_reuse_match(): one comparison of can_reuse_mix_texture():
 *   whether an earlier mix's rendered texture can stand in for this mix's.
 *
 * obs-video.c keeps every memcpy() (it runs the plan), the frame lock and
 * unlock, video_output_get_info(), the effect pointers (it maps the choice to
 * obs->video's), and the walk of the mix list. Nothing here takes a lock,
 * calls into libobs, touches a frame or keeps state.
 *
 * The scale types are copied below, so this header needs no core header
 * past media-io/video-io.h; obs-video.c checks them against obs.h.
 *
 * Hidden in libobs: not EXPORT. */

/* enum obs_scale_type (obs.h) */
#define OBS_VIDEO_COPY_SCALE_DISABLE 0
#define OBS_VIDEO_COPY_SCALE_POINT 1
#define OBS_VIDEO_COPY_SCALE_BICUBIC 2
#define OBS_VIDEO_COPY_SCALE_BILINEAR 3
#define OBS_VIDEO_COPY_SCALE_LANCZOS 4
#define OBS_VIDEO_COPY_SCALE_AREA 5

/* At most one copy per GPU conversion plane (NUM_CHANNELS). */
#define OBS_VIDEO_COPY_MAX 3

/* One copy into a plane of the output frame. */
struct obs_video_copy {
	/* The source starts where copy 0's source ended (just past plane 0 of
	 * input->data[0]) instead of at its own input plane: the UV plane of a
	 * packed NV12 or P010 frame. */
	bool src_after_plane0;
	/* One memcpy() of bytes, after which the source moves on by bytes. */
	bool whole;
	/* whole: the size of the one copy. Otherwise the size of each row. */
	size_t bytes;
	/* Not whole: rows memcpy()s of bytes, the source moving on by
	 * src_stride and the destination by dst_stride after each. Set either
	 * way. */
	uint32_t rows;
	uint32_t src_stride;
	uint32_t dst_stride;
};

/* The copies of one frame: copy i writes output plane i and reads input plane
 * i (or plane 0's continuation, src_after_plane0), in this order. */
struct obs_video_copy_plan {
	size_t count;
	struct obs_video_copy copies[OBS_VIDEO_COPY_MAX];
};

/* The effect render_output_texture() scales with; obs-video.c maps it to
 * obs->video's effect of the same name. */
enum obs_video_scale_effect {
	/* default_effect */
	OBS_VIDEO_SCALE_EFFECT_DEFAULT = 0,
	/* bilinear_lowres_effect */
	OBS_VIDEO_SCALE_EFFECT_BILINEAR_LOWRES = 1,
	/* bicubic_effect */
	OBS_VIDEO_SCALE_EFFECT_BICUBIC = 2,
	/* lanczos_effect */
	OBS_VIDEO_SCALE_EFFECT_LANCZOS = 3,
	/* area_effect */
	OBS_VIDEO_SCALE_EFFECT_AREA = 4,
};

/* The bit of obs_video_scale_input.loaded that says the effect loaded (its
 * pointer is not NULL). */
#define OBS_VIDEO_SCALE_LOADED(effect) ((uint32_t)1 << (effect))

/* What get_scale_effect() looks at. */
struct obs_video_scale_input {
	/* mix->ovi.base_width and base_height */
	uint32_t base_width;
	uint32_t base_height;
	/* The output texture's width and height (gs_texture_get_width() and
	 * _height() of mix->output_texture). */
	uint32_t target_width;
	uint32_t target_height;
	/* video_output_get_info(mix->video)->width and height. obs-video.c
	 * reads them only when the target is not close to the base size, as
	 * before; they are not looked at when it is. */
	uint32_t output_width;
	uint32_t output_height;
	/* mix->ovi.scale_type, an enum obs_scale_type value */
	int scale_type;
	/* OBS_VIDEO_SCALE_LOADED() bits of the effects that loaded */
	uint32_t loaded;
};

/* What can_reuse_mix_texture() compares of a mix. */
struct obs_video_mix_reuse_key {
	/* mix->view, compared as a number only */
	uintptr_t view;
	/* mix->render_space, an enum gs_color_space value */
	int render_space;
	/* mix->ovi.base_width and base_height */
	uint32_t base_width;
	uint32_t base_height;
	/* mix->texture_rendered */
	bool rendered;
};

/* set_gpu_converted_data(): the copies of a frame of format with the given
 * size, from a GPU-converted frame with the line sizes in_linesize into one
 * with out_linesize (MAX_AV_PLANES each; the first three are read). No copy
 * for a format without a GPU conversion, or outside the enum. */
struct obs_video_copy_plan obs_video_gpu_copy_plan(enum video_format format, uint32_t width, uint32_t height,
						   const uint32_t *in_linesize, const uint32_t *out_linesize);

/* copy_rgbx_frame(): the copy of plane 0 of an RGBA, BGRA or BGRX frame. */
struct obs_video_copy obs_video_rgbx_copy_plan(uint32_t width, uint32_t height, uint32_t in_linesize,
					       uint32_t out_linesize);

/* resolution_close(): both axes of width x height within 16 of the base
 * size. */
bool obs_video_scale_resolution_close(uint32_t base_width, uint32_t base_height, uint32_t width, uint32_t height);

/* get_scale_effect(): the default effect when the target is close to the
 * base size; else the bilinear low resolution effect when the output is under
 * half the base size on both axes, else the scale type's; and, when that one
 * did not load, the bicubic effect if it loaded, else the default. */
enum obs_video_scale_effect obs_video_scale_effect_choice(const struct obs_video_scale_input *input);

/* One mix of can_reuse_mix_texture()'s walk: other (an earlier mix) can
 * stand in for mix when they share the view, the render space and the base
 * size, and other has rendered its texture. mix->rendered is not looked
 * at. */
bool obs_video_mix_reuse_match(const struct obs_video_mix_reuse_key *other, const struct obs_video_mix_reuse_key *mix);
