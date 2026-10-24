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

#include "libobs/build/libobs-ported-source-transition-render.pch.h"

/* The render and audio step decisions of the transitions, moved from
 * obs-source-transition.c: the stop steps of obs_transition_video_render2(),
 * obs_transition_video_render_direct() and obs_transition_audio_render(),
 * the draw choice of render2, the child the direct render draws, the mix or
 * copy of the audio render, the checks and start frame of
 * transition_process_audio() and the offsets of
 * recalculate_transition_matrix(), on plain values. obs-source-transition.c
 * keeps the locks, the clock reads, the state copy and the refs,
 * obs_transition_stop(), handle_stop() and the signals, the child reads, the
 * texture lock, the renders, the callbacks, mix_child(), the (int) casts,
 * add_alignment() and the matrix calls, in their order. Declared in
 * obs-source-transition-render.h without EXPORT. In hybrid libobs the Rust
 * port (rust/obs-source) replaces this file. */

bool obs_source_transition_render_video_due(float t, bool transitioning_video)
{
	return t >= 1.0f && transitioning_video;
}

enum obs_source_transition_render_video_step obs_source_transition_render_video_stop(bool transitioning_audio,
										     bool active)
{
	if (!active)
		return OBS_SOURCE_TRANSITION_RENDER_VIDEO_STOP_AUDIO;

	if (!transitioning_audio)
		return OBS_SOURCE_TRANSITION_RENDER_VIDEO_STOP;

	return OBS_SOURCE_TRANSITION_RENDER_VIDEO_END;
}

enum obs_source_transition_render_draw_step obs_source_transition_render_draw(bool transitioning_video,
									      bool textures_locked, bool has_callback,
									      bool transitioning_audio)
{
	if (transitioning_video && textures_locked && has_callback)
		return OBS_SOURCE_TRANSITION_RENDER_DRAW_CALLBACK;
	else if (transitioning_audio)
		return OBS_SOURCE_TRANSITION_RENDER_DRAW_B;
	else
		return OBS_SOURCE_TRANSITION_RENDER_DRAW_A;
}

int obs_source_transition_render_direct_index(bool transitioning, int target)
{
	bool render_b = target == OBS_SOURCE_TRANSITION_RENDER_TARGET_B;
	return (transitioning && render_b) ? 1 : 0;
}

enum obs_source_transition_render_audio_step
obs_source_transition_render_audio_stop(bool has_min_ts, float t, bool transitioning_video, bool transitioning_audio)
{
	if (has_min_ts) {
		if (!(t >= 1.0f && transitioning_audio))
			return OBS_SOURCE_TRANSITION_RENDER_AUDIO_KEEP;

	} else if (!(!transitioning_video && transitioning_audio)) {
		return OBS_SOURCE_TRANSITION_RENDER_AUDIO_KEEP;
	}

	/* transition_stop_audio() */
	if (!transitioning_video)
		return OBS_SOURCE_TRANSITION_RENDER_AUDIO_STOP;

	return OBS_SOURCE_TRANSITION_RENDER_AUDIO_END;
}

uint32_t obs_source_transition_render_audio_mix(bool transitioning_audio, bool has_a, bool has_b)
{
	uint32_t mix = 0;

	if (transitioning_audio) {
		if (has_a)
			mix |= OBS_SOURCE_TRANSITION_RENDER_MIX_A;
		if (has_b)
			mix |= OBS_SOURCE_TRANSITION_RENDER_MIX_B;
	} else if (has_a) {
		mix |= OBS_SOURCE_TRANSITION_RENDER_COPY_A;
	}

	return mix;
}

bool obs_source_transition_render_audio_valid(bool present, bool pending, bool duplicated)
{
	return present && !pending && !duplicated;
}

enum obs_source_transition_render_audio_place obs_source_transition_render_audio_place(uint64_t ts, uint64_t min_ts,
										       size_t sample_rate, size_t *pos)
{
	if (!ts)
		return OBS_SOURCE_TRANSITION_RENDER_PLACE_NONE;

	/* ns_to_audio_frames(sample_rate, ts - min_ts) */
	*pos = (size_t)util_mul_div64(ts - min_ts, sample_rate, 1000000000ULL);

	if (*pos > OBS_SOURCE_TRANSITION_RENDER_AUDIO_FRAMES)
		return OBS_SOURCE_TRANSITION_RENDER_PLACE_LATE;

	return OBS_SOURCE_TRANSITION_RENDER_PLACE_MIX;
}

struct obs_source_transition_render_offset obs_source_transition_render_offset(float tr_cx, float tr_cy,
									       float source_cx, float source_cy,
									       float scale_x, float scale_y)
{
	struct obs_source_transition_render_offset offset;

	source_cx *= scale_x;
	source_cy *= scale_y;

	offset.x = tr_cx - source_cx;
	offset.y = tr_cy - source_cy;
	return offset;
}
