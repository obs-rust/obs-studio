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

/* The per-frame video output step decisions of obs-video.c, on plain values
 * (obs-video-output-step.c). Each one is made where the C made it, from
 * values the caller reads at that point:
 * - obs_video_step_activation(): update_active_state(), which frame data to
 *   clear when raw output, GPU encoding or either becomes active, and the
 *   new *_was_active flags;
 * - obs_video_step_stage(): stage_output_texture(), whether this frame's
 *   copy surfaces are staged, from which texture, and how many;
 * - obs_video_step_queue(), obs_video_step_queue_copy_uv() and
 *   obs_video_step_queue_count(): queue_frame(), whether a frame for the
 *   GPU encoders is a duplicate of the last queued one or a new one (copied
 *   or swapped in), whether the UV plane is copied separately, and the count
 *   left;
 * - obs_video_step_render_targets() and obs_video_step_render():
 *   render_video(), whether the output textures are rendered at all and
 *   from which textures, then whether to convert, feed the GPU encoders and
 *   stage;
 * - obs_video_step_ring(), obs_video_step_download() and
 *   obs_video_step_output(): output_frame(), the texture ring (the texture
 *   downloaded this frame and the next current one), whether to download,
 *   and whether a downloaded frame is output.
 *
 * obs-video.c keeps every gs_* call in its order and under its conditions,
 * the deques, os_sem_post(), the texture-frame host calls, the shared-planes
 * and GPU-encode-surface host queries (it passes their results), the maps of
 * download_frame(), the mix list and its mutex, and every atomic load (it
 * passes the loaded values). Nothing here takes a lock, calls into libobs,
 * touches a texture or keeps state.
 *
 * Hidden in libobs: not EXPORT. */

/* NUM_TEXTURES (obs-internal.h): the frames in flight between rendering and
 * download. */
#define OBS_VIDEO_STEP_TEXTURES 2
/* NUM_CHANNELS (obs-internal.h): the planes of a converted frame. */
#define OBS_VIDEO_STEP_CHANNELS 3

/* What update_active_state() looks at. */
struct obs_video_step_activation_input {
	/* video->raw_was_active, gpu_was_active and was_active */
	bool raw_was_active;
	bool gpu_was_active;
	bool was_active;
	/* os_atomic_load_long() of video->raw_active and gpu_encoder_active,
	 * loaded by obs-video.c in that order */
	long raw_active;
	long gpu_encoder_active;
};

/* What update_active_state() does. */
struct obs_video_step_activation {
	/* clear_base_frame_data(), then clear_raw_frame_data(), then
	 * clear_gpu_frame_data(), in that order */
	bool clear_base;
	bool clear_raw;
	bool clear_gpu;
	/* The new video->raw_was_active, gpu_was_active and was_active */
	bool raw_active;
	bool gpu_active;
	bool active;
};

/* What stage_output_texture() does after unmap_last_surface(). */
struct obs_video_step_stage {
	/* The copy surfaces are staged: active_copy_surfaces[cur_texture] and
	 * textures_copied[cur_texture] are written. Otherwise nothing is. */
	bool staged;
	/* Copy surface 0 stages the output texture; otherwise copy surface i
	 * stages convert texture i. */
	bool from_output;
	/* Copy surfaces 0 to channels - 1 are staged (each one that is not
	 * NULL) and made active; the rest, up to NUM_CHANNELS, are cleared. */
	size_t channels;
};

/* What queue_frame() does with a frame for the GPU encoders. */
enum obs_video_step_queue_action {
	/* Count the last queued frame once more (or stop, when there is
	 * none). */
	OBS_VIDEO_STEP_QUEUE_DUPLICATE = 0,
	/* Take a free frame and copy the encode textures into it. */
	OBS_VIDEO_STEP_QUEUE_COPY = 1,
	/* Take a free frame and swap its textures with the encode
	 * textures. */
	OBS_VIDEO_STEP_QUEUE_SWAP = 2,
};

/* What render_video() renders once the main texture is drawn. */
struct obs_video_step_render_targets {
	/* Render the output texture and run the steps below. */
	bool outputs;
	/* Use the encode textures, and the copy surfaces and channel count
	 * of obs_host_gpu_encode_surfaces(). */
	bool encode;
};

/* What render_video() does with the output texture, in this order. */
struct obs_video_step_render {
	/* render_convert_texture() */
	bool convert;
	/* output_gpu_encoders() */
	bool encode;
	/* stage_output_texture() */
	bool stage;
};

/* The texture ring of output_frame(). */
struct obs_video_step_ring {
	/* The texture before cur_texture, which this frame downloads. */
	int prev;
	/* The texture after cur_texture, the next frame's. */
	int next;
};

/* update_active_state(): a frame data clear for each of base (either
 * output), raw and GPU that was not active and is now. */
struct obs_video_step_activation obs_video_step_activation(const struct obs_video_step_activation_input *input);

/* stage_output_texture(): without GPU conversion, copy surface 0 from the
 * output texture; with it, once the texture is converted, channel_count copy
 * surfaces from the convert textures; else nothing. */
struct obs_video_step_stage obs_video_step_stage(bool gpu_conversion, bool texture_converted, size_t channel_count);

/* queue_frame(): avail_size and queue_size are the byte sizes of
 * gpu_encoder_avail_queue and gpu_encoder_queue, count is vframe_info's.
 * A duplicate when no frame is free, or when one is queued and this frame
 * repeats; else a copy for raw output or a repeat, or a swap. */
enum obs_video_step_queue_action obs_video_step_queue(size_t avail_size, size_t queue_size, int count, bool raw_active);

/* queue_frame(): after the copy of the Y texture, whether the UV texture is
 * copied on its own (obs_host_nv12_planes_share_texture() is false). */
bool obs_video_step_queue_copy_uv(bool planes_share_texture);

/* queue_frame(): the count left after a frame is queued or duplicated;
 * queue_frame() returns whether it is not 0. */
int obs_video_step_queue_count(int count);

/* render_video(): the outputs are rendered when raw output or GPU encoding
 * is active, from the encode textures for GPU encoding. */
struct obs_video_step_render_targets obs_video_step_render_targets(bool raw_active, bool gpu_active);

/* render_video(): convert with GPU conversion, feed the GPU encoders when GPU
 * encoding is active, stage for raw output. */
struct obs_video_step_render obs_video_step_render(bool raw_active, bool gpu_active, bool gpu_conversion);

/* output_frame(): the textures before and after cur_texture in the ring of
 * NUM_TEXTURES. */
struct obs_video_step_ring obs_video_step_ring(int cur_texture);

/* output_frame(): download the previous texture for raw output. */
bool obs_video_step_download(bool raw_active);

/* output_frame(): output the downloaded frame for raw output, once it is
 * ready. */
bool obs_video_step_output(bool raw_active, bool frame_ready);
