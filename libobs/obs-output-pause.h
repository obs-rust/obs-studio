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

/* The pause plan of obs-output.c, on plain values (obs-output-pause.c). One
 * function per decision, each asked where the C made it, from values the
 * caller reads at that point:
 * - obs_output_pause(): obs_output_pause_allowed() (the output can pause),
 *   then, once the output is active, obs_output_pause_already() (it is
 *   already in the wanted state, which succeeds without doing anything);
 * - obs_encoded_output_pause(): obs_output_pause_lock_set() (whose pause
 *   records are locked, checked and updated, and in which order);
 * - get_closest_v_ts(): obs_output_pause_closest_ts() (the pause time, the
 *   clock snapped to the video frame grid);
 * - pause_can_start(), pause_can_stop() and end_pause():
 *   obs_output_pause_started() and obs_output_pause_ended(), one test per
 *   field of the record, which the caller joins with ! and && in the order
 *   it always read them (the end is read only once the start test passed);
 * - end_pause(): obs_output_pause_offset() (the offset after a pause ends).
 *
 * obs-output.c keeps obs_output_valid(), every atomic load and store
 * (active, paused, the encoders' paused), each loaded where it always was,
 * every pause.mutex lock and unlock (in the slots' order, released in
 * reverse), the read of the frame interval and os_gettime_ns() in their
 * order, get_first_video_encoder_index(), the stores to each record, the
 * pause and unpause signals and the log. Nothing here takes a lock, calls
 * into libobs or keeps state.
 *
 * The constants are copied below, so this header needs no core header;
 * obs-output.c checks them against obs-output.h.
 *
 * Hidden in libobs: not EXPORT. */

/* OBS_OUTPUT_CAN_PAUSE */
#define OBS_OUTPUT_PAUSE_CAN_PAUSE (1 << 5)

/* MAX_OUTPUT_VIDEO_ENCODERS, MAX_OUTPUT_AUDIO_ENCODERS */
#define OBS_OUTPUT_PAUSE_MAX_VIDEO_ENCODERS 10
#define OBS_OUTPUT_PAUSE_MAX_AUDIO_ENCODERS 6

/* obs_output_pause(): an output with flags (info.flags) can pause. */
bool obs_output_pause_allowed(uint32_t flags);

/* obs_output_pause(), for an active output: it is paused (paused) and the
 * call asks for pause, so it succeeds at once, with no signal and no log. */
bool obs_output_pause_already(bool paused, bool pause);

/* obs_encoded_output_pause(): the slots whose encoder's pause record is
 * locked, checked and updated, from each slot's encoder as an opaque id (0
 * for an empty slot): bit i of *video_lock for video[i], bit i of
 * *audio_lock for audio[i]. The caller locks the marked slots, video slots
 * first, each in slot order, and unlocks them in the reverse order. */
void obs_output_pause_lock_set(const uintptr_t video[OBS_OUTPUT_PAUSE_MAX_VIDEO_ENCODERS],
			       const uintptr_t audio[OBS_OUTPUT_PAUSE_MAX_AUDIO_ENCODERS], uint32_t *video_lock,
			       uint32_t *audio_lock);

/* get_closest_v_ts(): the pause time for a clock of now, in ns: the video
 * frame grid of interval ns (video_frame_interval_ns) through the record's
 * last video timestamp (last_video_ts), two frames ahead. */
uint64_t obs_output_pause_closest_ts(uint64_t last_video_ts, uint64_t interval, uint64_t now);

/* pause_can_start() and pause_can_stop(): a record whose start is ts_start
 * started a pause. */
bool obs_output_pause_started(uint64_t ts_start);

/* pause_can_start(), pause_can_stop() and end_pause(): a record whose end is
 * ts_end ended its pause (until the encoders clear it). */
bool obs_output_pause_ended(uint64_t ts_end);

/* end_pause(): the record's offset once a pause from ts_start to ts_end
 * ended, from its offset before (ts_offset). */
uint64_t obs_output_pause_offset(uint64_t ts_offset, uint64_t ts_start, uint64_t ts_end);
