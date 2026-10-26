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

/* The source audio placement and action timeline of obs-source.c, on plain
 * values (obs-source-audio-place.c):
 * - source_output_audio_place(): whether the buffers are reset first (no
 *   audio_ts yet, or a packet older than it), the packet's size and byte
 *   placement, the drop past MAX_BUF_SIZE, and how much is popped off the
 *   back of each channel after the place;
 * - source_output_audio_push_back(): the drop past MAX_BUF_SIZE;
 * - the last step of source_output_audio_data(): nothing for a monitor-only
 *   source, else push back (with an audio_ts) or place;
 * - apply_audio_actions(): each queued action's timestamp clamped to
 *   audio_ts, its frame, the stop at AUDIO_OUTPUT_FRAMES, how far the
 *   previous volume fills, and the mixes multiplied.
 *
 * Each decision is its own function, asked at its own point, so
 * obs-source.c keeps every read where it was: audio_ts (read again after
 * reset_audio_data(), and only where the C's && and || reach it),
 * monitoring_type, the first channel's buffer size before a push back, each
 * channel's size after its place, and audio_mixers once per mix. It keeps
 * audio_buf_mutex and audio_actions_mutex, audio_output_get_channels() and
 * audio_output_get_sample_rate() (it passes the rate), reset_audio_data(),
 * deque_place(), deque_pop_back() and deque_push_back(), the DEBUG_AUDIO
 * log, the stores of last_audio_input_buf_size, da_erase(),
 * apply_audio_action(), get_source_volume() (called before the loop and
 * after each applied action, as before), the stores into the volume array
 * and multiply_vol_data(). None of these functions takes a lock, calls into
 * libobs, touches memory or keeps state.
 *
 * The monitoring type crosses as int, so this header needs no other libobs
 * header; obs-source.c checks that the constants below match the core's and
 * that enum obs_monitoring_type is int-sized.
 *
 * Hidden in libobs: not EXPORT. */

/* AUDIO_OUTPUT_FRAMES (media-io/audio-io.h). */
#define OBS_SOURCE_AUDIO_PLACE_FRAMES 1024
/* MAX_BUF_SIZE (obs-source.c): 1000 * AUDIO_OUTPUT_FRAMES * sizeof(float)
 * bytes per channel. */
#define OBS_SOURCE_AUDIO_PLACE_MAX_BUF_SIZE (1000 * OBS_SOURCE_AUDIO_PLACE_FRAMES * sizeof(float))
/* OBS_MONITORING_TYPE_MONITOR_ONLY (enum obs_monitoring_type, obs.h). */
#define OBS_SOURCE_AUDIO_PLACE_MONITOR_ONLY 1

/* Whether the source has an audio timestamp: audio_ts != 0. The place resets
 * without one, and a push back needs one. */
bool obs_source_audio_place_started(uint64_t audio_ts);

/* Whether a timestamp is older than audio_ts: timestamp < audio_ts. A packet
 * this old resets the buffers; an action this old is moved to audio_ts. */
bool obs_source_audio_place_older(uint64_t timestamp, uint64_t audio_ts);

/* A packet's size in bytes per channel: frames * sizeof(float), in
 * size_t. */
size_t obs_source_audio_place_size(uint32_t frames);

/* get_buf_placement(), in bytes: the frames from audio_ts to timestamp,
 * (size_t)util_mul_div64(timestamp - audio_ts, sample_rate, 1000000000) *
 * sizeof(float), the difference wrapping as uint64_t and the product as
 * size_t. */
size_t obs_source_audio_place_offset(uint64_t timestamp, uint64_t audio_ts, uint32_t sample_rate);

/* Whether size bytes from start keep a channel within MAX_BUF_SIZE:
 * start + size <= MAX_BUF_SIZE, the sum wrapping as size_t. start is the
 * placement for a place and the first channel's size for a push back. */
bool obs_source_audio_place_fits(size_t start, size_t size);

/* The bytes popped off the back of a channel after the place:
 * buf_size - (placement + size), wrapping as size_t. buf_size is the
 * channel's size after deque_place(). */
size_t obs_source_audio_place_pop_back(size_t buf_size, size_t placement, size_t size);

/* Whether the packet goes to the buffers at all: a monitoring type other
 * than OBS_MONITORING_TYPE_MONITOR_ONLY. */
bool obs_source_audio_place_outputs(int monitoring_type);

/* An action's frame in this tick: conv_time_to_frames(sample_rate,
 * timestamp - audio_ts), (size_t)util_mul_div64(timestamp - audio_ts,
 * sample_rate, 1000000000), the difference wrapping as uint64_t. The caller
 * has already clamped timestamp to audio_ts. */
size_t obs_source_audio_place_action_frame(uint64_t timestamp, uint64_t audio_ts, size_t sample_rate);

/* Whether the action loop stops at this frame, leaving the action queued:
 * frame >= AUDIO_OUTPUT_FRAMES. */
bool obs_source_audio_place_action_stops(size_t frame);

/* How far the previous volume fills before an action at new_frame_num:
 * new_frame_num when it is past frame_num, else frame_num (nothing is
 * filled). */
size_t obs_source_audio_place_fill_end(size_t frame_num, size_t new_frame_num);

/* Whether the volume array multiplies a mix: bit mix of audio_mixers. mix is
 * below MAX_AUDIO_MIXES (the C shifts an int by it). */
bool obs_source_audio_place_mix_on(uint32_t audio_mixers, size_t mix);
