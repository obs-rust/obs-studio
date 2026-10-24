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

/* The volume decisions of get_source_volume() and apply_audio_volume() in
 * obs-source.c, on plain values (obs-source-volume.c):
 * - obs_source_volume_snap(): the volume a mix applies, 0 when the gate
 *   mutes the source or the volume is within 0.0001 of 0, 1 when it is
 *   within 0.0001 of 1, else the volume itself;
 * - obs_source_volume_action_due(): whether the first queued audio action
 *   falls inside this tick, action_ts < audio_ts + duration;
 * - obs_source_volume_step(): what apply_audio_volume() does with the
 *   volume: nothing at exactly 1, clear every mix at exactly 0 (or -0) or
 *   with no mixers, else multiply;
 * - obs_source_volume_mixes(): whether the multiply touches a mix, which
 *   must be in both the source's mixers and the tick's.
 *
 * get_source_volume()'s gate (muted, disabled, push-to-mute and
 * push-to-talk active) goes through obs-source-ptm-gate.h's helpers.
 * obs-source.c keeps every read and write of the source's fields, in the
 * same order, and passes the values it read: the pressed flags, the mute
 * and the volume come from the queued audio actions, and enabled and the
 * mixers from other threads, without atomics. It keeps the action queue
 * and its mutex, apply_audio_actions(), conv_frames_to_time() (it passes
 * the duration), the memset and the multiply loops. None of the functions
 * takes a lock, calls into libobs, touches memory or keeps state.
 *
 * Hidden in libobs: not EXPORT. */

/* What apply_audio_volume() does with the volume. */
enum obs_source_volume_op {
	/* exactly 1: leave the buffers as they are */
	OBS_SOURCE_VOLUME_SKIP = 0,
	/* exactly 0 (or -0), or no mixers: clear every mix */
	OBS_SOURCE_VOLUME_CLEAR = 1,
	/* anything else, NaN included: multiply the mixes in both masks */
	OBS_SOURCE_VOLUME_MULTIPLY = 2,
};

/* 0.0f when muted or close_float(volume, 0.0f, 0.0001f), 1.0f when
 * close_float(volume, 1.0f, 0.0001f), else volume. */
float obs_source_volume_snap(bool muted, float volume);

/* action_ts < audio_ts + duration, the sum wrapping as uint64_t. */
bool obs_source_volume_action_due(uint64_t action_ts, uint64_t audio_ts, uint64_t duration);

/* SKIP when vol == 1.0f, CLEAR when vol == 0.0f or mixers == 0, else
 * MULTIPLY. */
enum obs_source_volume_op obs_source_volume_step(float vol, uint32_t mixers);

/* Whether mix is in audio_mixers and in mixers. mix is below
 * MAX_AUDIO_MIXES (the C shifts an int by it). */
bool obs_source_volume_mixes(uint32_t audio_mixers, uint32_t mixers, size_t mix);
