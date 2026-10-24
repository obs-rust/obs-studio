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

#include "libobs/build/libobs-ported-video-output-step.pch.h"

/* The per-frame video output step decisions, moved from obs-video.c: only
 * flags and counts, no texture, deque or libobs state. obs-video.c makes the
 * calls they decide on. Ported to rust/obs-core (video_output_step): the
 * hybrid libobs drops this file. */

/* update_active_state(), without the clears and the stores. */
struct obs_video_step_activation obs_video_step_activation(const struct obs_video_step_activation_input *input)
{
	const bool raw_was_active = input->raw_was_active;
	const bool gpu_was_active = input->gpu_was_active;
	const bool was_active = input->was_active;

	bool raw_active = input->raw_active > 0;
	const bool gpu_active = input->gpu_encoder_active > 0;
	const bool active = raw_active || gpu_active;

	struct obs_video_step_activation step = {0};

	if (!was_active && active)
		step.clear_base = true;
	if (!raw_was_active && raw_active)
		step.clear_raw = true;
	if (!gpu_was_active && gpu_active)
		step.clear_gpu = true;

	step.gpu_active = gpu_active;
	step.raw_active = raw_active;
	step.active = active;
	return step;
}

/* stage_output_texture(), without the staging. */
struct obs_video_step_stage obs_video_step_stage(bool gpu_conversion, bool texture_converted, size_t channel_count)
{
	struct obs_video_step_stage step = {0};

	if (!gpu_conversion) {
		step.staged = true;
		step.from_output = true;
		step.channels = 1;
	} else if (texture_converted) {
		step.staged = true;
		step.channels = channel_count;
	}

	return step;
}

/* queue_frame(): duplicate, or a new frame copied or swapped in. */
enum obs_video_step_queue_action obs_video_step_queue(size_t avail_size, size_t queue_size, int count, bool raw_active)
{
	bool duplicate = !avail_size || (queue_size && count > 1);

	if (duplicate)
		return OBS_VIDEO_STEP_QUEUE_DUPLICATE;

	/* the count > 1 case causing a copy can only happen if by some chance
	 * the very first frame has to be duplicated for whatever reason.
	 * otherwise, it goes to the 'duplicate' case above, which will ensure
	 * better performance. */
	if (raw_active || count > 1)
		return OBS_VIDEO_STEP_QUEUE_COPY;

	return OBS_VIDEO_STEP_QUEUE_SWAP;
}

/* Y and UV textures are views of the same texture on D3D, and
 * gs_copy_texture will copy all views of the underlying texture. On other
 * platforms, these are two distinct textures that must be copied
 * separately. */
bool obs_video_step_queue_copy_uv(bool planes_share_texture)
{
	return !planes_share_texture;
}

int obs_video_step_queue_count(int count)
{
	return --count;
}

struct obs_video_step_render_targets obs_video_step_render_targets(bool raw_active, bool gpu_active)
{
	struct obs_video_step_render_targets targets = {0};

	if (raw_active || gpu_active) {
		targets.outputs = true;
		targets.encode = gpu_active;
	}

	return targets;
}

struct obs_video_step_render obs_video_step_render(bool raw_active, bool gpu_active, bool gpu_conversion)
{
	struct obs_video_step_render step = {
		.convert = gpu_conversion,
		.encode = gpu_active,
		.stage = raw_active,
	};

	return step;
}

struct obs_video_step_ring obs_video_step_ring(int cur_texture)
{
	struct obs_video_step_ring ring;

	ring.prev = cur_texture == 0 ? OBS_VIDEO_STEP_TEXTURES - 1 : cur_texture - 1;

	ring.next = cur_texture + 1;
	if (ring.next == OBS_VIDEO_STEP_TEXTURES)
		ring.next = 0;

	return ring;
}

bool obs_video_step_download(bool raw_active)
{
	return raw_active;
}

bool obs_video_step_output(bool raw_active, bool frame_ready)
{
	return raw_active && frame_ready;
}
