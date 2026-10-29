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

/* The begin-capture plan of obs-output.c, on plain values
 * (obs-output-begin.c). obs_output_begin_data_capture_internal() runs it on
 * every start of an encoded output:
 * - obs_output_begin_video_interval() and obs_output_begin_audio_interval():
 *   calculate_batch_size(), the packet interval of each encoder in ns;
 * - obs_output_begin_batch_size(): calculate_batch_size(), the interleaver's
 *   maximum batch size (interleaver_max_batch_size) from those intervals;
 * - obs_output_begin_pair_video() and obs_output_begin_pair_audio():
 *   pair_encoders(), whether the first video encoder and each audio encoder
 *   take part in the pairing.
 *
 * The arithmetic is C's: fps_den * frame_rate_divisor is computed in
 * uint32_t and wraps, and util_mul_div64() is the one of util_uint64.h. An
 * fps_num or sample_rate of 0 and an interval of 0 (an audio encoder whose
 * frame size is 0) divide by 0 and are undefined in C, and so is a quotient
 * above 64 bits on MSVC x64 (_udiv128 faults).
 *
 * obs-output.c keeps obs_get_video_info(), the encoder getters (frame rate
 * divisor, sample rate, frame size), the walk over the encoder slots and the
 * array of intervals, get_first_video_encoder_index(), the init_mutex locks,
 * the weak references and the paired_encoders pushes, the store to
 * interleaver_max_batch_size and the log. Nothing here takes a lock, calls
 * into libobs or keeps state.
 *
 * Hidden in libobs: not EXPORT. */

/* calculate_batch_size(): a video encoder's packet interval in ns,
 * util_mul_div64(1000000000, fps_den * frame_rate_divisor, fps_num), the
 * product in uint32_t. */
uint64_t obs_output_begin_video_interval(uint32_t fps_num, uint32_t fps_den, uint32_t frame_rate_divisor);

/* calculate_batch_size(): an audio encoder's packet interval in ns,
 * util_mul_div64(1000000000, frame_size, sample_rate). */
uint64_t obs_output_begin_audio_interval(uint32_t sample_rate, size_t frame_size);

/* calculate_batch_size(): the sum over the num intervals of
 * (largest * 2) / interval, largest being the largest of them. intervals may
 * be NULL when num is 0 (then 0). */
size_t obs_output_begin_batch_size(const uint64_t *intervals, size_t num);

/* pair_encoders(): true when the first video encoder takes part, that is
 * when it is not active yet (video_active, read under its init_mutex). */
bool obs_output_begin_pair_video(bool video_active);

/* pair_encoders(): true when this audio encoder is paired with the video
 * encoder: it is not active and has no paired encoder yet (paired_num, its
 * paired_encoders.num, read under its init_mutex). */
bool obs_output_begin_pair_audio(bool audio_active, size_t paired_num);
