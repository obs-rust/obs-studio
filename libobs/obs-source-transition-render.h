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

/* The render and audio step decisions of the transitions in
 * obs-source-transition.c, on plain values (obs-source-transition-render.c):
 * - obs_source_transition_render_video_due() and _video_stop(): the stop
 *   step of obs_transition_video_render2() and
 *   obs_transition_video_render_direct(). Due once t >= 1 while the video
 *   is transitioning (a NaN t is never due); then the video ends, and the
 *   transition stops unless its audio runs on. The direct render ends the
 *   audio too for a transition that is not active; render2 passes active as
 *   true.
 * - obs_source_transition_render_draw(): what obs_transition_video_render2()
 *   draws: both children through the callback while the video transitions
 *   with the textures locked and a callback set; else source B while the
 *   audio transitions; else source A.
 * - obs_source_transition_render_direct_index(): the child
 *   obs_transition_video_render_direct() draws: B only for target B while
 *   transitioning, else A.
 * - obs_source_transition_render_audio_stop(): the stop step of
 *   obs_transition_audio_render(). With a timestamp, at t >= 1 while the
 *   audio transitions; without one, only once the video has ended too (so a
 *   child whose audio starts late still joins the transition). The audio
 *   ends, and the transition stops unless its video runs on.
 * - obs_source_transition_render_audio_mix(): mix both children (each that
 *   is there) while the audio transitions, else copy source A's audio.
 * - obs_source_transition_render_audio_valid() and _audio_place(): the
 *   checks of transition_process_audio(): a child there, its audio not
 *   pending and not duplicated; a timestamp that is not 0; and the frame its
 *   audio starts at (ns_to_audio_frames() of ts - min_ts, in uint64_t), not
 *   past AUDIO_OUTPUT_FRAMES (a start at AUDIO_OUTPUT_FRAMES mixes 0 frames).
 * - obs_source_transition_render_offset(): the scaled child size of
 *   recalculate_transition_matrix() and the offsets it passes, cast to int,
 *   to add_alignment().
 *
 * obs-source-transition.c keeps the locks, get_video_time() and calc_time(),
 * copy_transition_state() and the refs, obs_transition_stop(), handle_stop()
 * and the signals, obs_source_active(), obs_source_removed() and the
 * timestamp reads, the texture trylock, render_child(), the callbacks,
 * mix_child() and get_sample_time(), the mixer loop and the memcpy(), the
 * offsets' (int) casts, add_alignment() and the matrix calls, each where and
 * when it was called before; it asks these functions in between and passes
 * in what it read.
 *
 * None of them takes a lock, calls into libobs or keeps state.
 *
 * The target is enum obs_transition_target passed as int, so this header
 * needs neither obs.h nor a graphics header; obs-source-transition.c checks
 * the values and the sizes.
 *
 * Hidden in libobs: not EXPORT. */

/* obs-source-transition.c checks that they are OBS_TRANSITION_SOURCE_B and
 * AUDIO_OUTPUT_FRAMES */
#define OBS_SOURCE_TRANSITION_RENDER_TARGET_B 1
#define OBS_SOURCE_TRANSITION_RENDER_AUDIO_FRAMES 1024

/* What a due video stop step does. */
enum obs_source_transition_render_video_step {
	/* transitioning_video ends; the audio runs on */
	OBS_SOURCE_TRANSITION_RENDER_VIDEO_END,
	/* transitioning_video ends, and obs_transition_stop() */
	OBS_SOURCE_TRANSITION_RENDER_VIDEO_STOP,
	/* transitioning_video and transitioning_audio end, and
	 * obs_transition_stop() (the direct render of a transition that is not
	 * active) */
	OBS_SOURCE_TRANSITION_RENDER_VIDEO_STOP_AUDIO,
};

/* What obs_transition_video_render2() draws. */
enum obs_source_transition_render_draw_step {
	/* both children into the textures, blended by the callback */
	OBS_SOURCE_TRANSITION_RENDER_DRAW_CALLBACK,
	/* source B with its matrix */
	OBS_SOURCE_TRANSITION_RENDER_DRAW_B,
	/* source A with its matrix */
	OBS_SOURCE_TRANSITION_RENDER_DRAW_A,
};

/* What the audio stop step of obs_transition_audio_render() does. */
enum obs_source_transition_render_audio_step {
	/* nothing */
	OBS_SOURCE_TRANSITION_RENDER_AUDIO_KEEP,
	/* transitioning_audio ends; the video runs on */
	OBS_SOURCE_TRANSITION_RENDER_AUDIO_END,
	/* transitioning_audio ends, and obs_transition_stop() */
	OBS_SOURCE_TRANSITION_RENDER_AUDIO_STOP,
};

/* The bits of obs_source_transition_render_audio_mix(). */
#define OBS_SOURCE_TRANSITION_RENDER_MIX_A (1u << 0)
#define OBS_SOURCE_TRANSITION_RENDER_MIX_B (1u << 1)
#define OBS_SOURCE_TRANSITION_RENDER_COPY_A (1u << 2)

/* Where a child's audio goes in transition_process_audio(). */
enum obs_source_transition_render_audio_place {
	/* a timestamp of 0: nothing more is read */
	OBS_SOURCE_TRANSITION_RENDER_PLACE_NONE,
	/* the child's mix is read, but it starts past the tick: nothing mixed */
	OBS_SOURCE_TRANSITION_RENDER_PLACE_LATE,
	/* mixed from *pos on */
	OBS_SOURCE_TRANSITION_RENDER_PLACE_MIX,
};

/* The offsets of recalculate_transition_matrix(), before the (int) casts. */
struct obs_source_transition_render_offset {
	float x;
	float y;
};

/* Whether the video stop step is due: t is the video time (get_video_time()),
 * transitioning_video the transition's flag. */
bool obs_source_transition_render_video_due(float t, bool transitioning_video);

/* The due video stop step: transitioning_audio is the transition's flag;
 * active is obs_source_active() of the transition for the direct render
 * (read only once the step is due), true for render2. */
enum obs_source_transition_render_video_step obs_source_transition_render_video_stop(bool transitioning_audio,
										     bool active);

/* What obs_transition_video_render2() draws, from the copied state's flags,
 * whether the texture trylock succeeded (false when it was not tried) and
 * whether a callback is set. */
enum obs_source_transition_render_draw_step obs_source_transition_render_draw(bool transitioning_video,
									      bool textures_locked, bool has_callback,
									      bool transitioning_audio);

/* The child obs_transition_video_render_direct() draws (0 or 1):
 * transitioning is the copied state's audio or video flag, target the
 * enum obs_transition_target asked for. */
int obs_source_transition_render_direct_index(bool transitioning, int target);

/* The audio stop step: has_min_ts is whether the children gave a timestamp,
 * t calc_time() at it (read only when has_min_ts), and the transition's two
 * flags. */
enum obs_source_transition_render_audio_step
obs_source_transition_render_audio_stop(bool has_min_ts, float t, bool transitioning_video, bool transitioning_audio);

/* Which children obs_transition_audio_render() mixes or copies, from the
 * copied state's audio flag and the children it holds. */
uint32_t obs_source_transition_render_audio_mix(bool transitioning_audio, bool has_a, bool has_b);

/* Whether transition_process_audio() reads a child's audio: present is
 * whether there is a child, pending and duplicated its audio_pending and
 * audio_is_duplicated. */
bool obs_source_transition_render_audio_valid(bool present, bool pending, bool duplicated);

/* Where the child's audio goes: ts is its audio timestamp, min_ts the
 * transition's, sample_rate the output's. *pos is written for LATE and
 * MIX. */
enum obs_source_transition_render_audio_place obs_source_transition_render_audio_place(uint64_t ts, uint64_t min_ts,
										       size_t sample_rate, size_t *pos);

/* The offsets of a child of source_cx by source_cy with the fit's scale
 * in a transition of tr_cx by tr_cy (all as recalculate_transition_matrix()
 * has them): tr_cx - source_cx * scale_x and tr_cy - source_cy * scale_y,
 * each product rounded to float first. */
struct obs_source_transition_render_offset obs_source_transition_render_offset(float tr_cx, float tr_cy,
									       float source_cx, float source_cy,
									       float scale_x, float scale_y);
