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

/* The push-to-mute and push-to-talk gate of source_muted() in obs-source.c,
 * on plain values (obs-source-ptm-gate.c):
 * - obs_source_ptm_gate_stop_time(): when a key held at os_time stops
 *   holding once it is let go: delay_ms milliseconds later, saturated at
 *   UINT64_MAX instead of wrapping (#2051). get_source_volume() uses it
 *   too.
 * - obs_source_ptm_gate_held(): whether a key that is no longer pressed
 *   still holds at os_time: strictly before its stop time.
 * - obs_source_ptm_gate_mutes() and obs_source_ptm_gate_talk_mutes():
 *   whether the push-to-mute gate (enabled and active) or the push-to-talk
 *   gate (enabled and not active) mutes the source.
 *
 * source_muted() keeps every read and write of the source's fields, in the
 * same order, and passes the values it read. The pressed flags are written
 * by the hotkey thread without atomics, and the stop times are also written
 * by get_source_volume() on the audio mix thread, so obs-source.c reads each
 * flag where it read it before (a key pressed between the two reads is
 * seen by the second), stores a stop time only while the key is held, and
 * reads enabled, user_muted and the enabled flags only where the C
 * short-circuit reached them. None of the functions takes a lock, calls
 * into libobs or keeps state.
 *
 * Hidden in libobs: not EXPORT. */

/* Nanoseconds per millisecond: the delays are milliseconds, os_time and the
 * stop times nanoseconds (os_gettime_ns()). */
#define OBS_SOURCE_PTM_GATE_NS_PER_MS 1000000ULL

/* os_time + delay_ms * OBS_SOURCE_PTM_GATE_NS_PER_MS, or UINT64_MAX where
 * the product or the sum would wrap. */
uint64_t obs_source_ptm_gate_stop_time(uint64_t os_time, uint64_t delay_ms);

/* os_time < stop_time: a key let go still holds until its stop time. */
bool obs_source_ptm_gate_held(uint64_t os_time, uint64_t stop_time);

/* Whether push-to-mute mutes: enabled is source->push_to_mute_enabled and
 * active whether its key is pressed or held. */
bool obs_source_ptm_gate_mutes(bool enabled, bool active);

/* Whether push-to-talk mutes: enabled is source->push_to_talk_enabled and
 * active whether its key is pressed or held. */
bool obs_source_ptm_gate_talk_mutes(bool enabled, bool active);
