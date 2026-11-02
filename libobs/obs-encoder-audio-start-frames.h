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

#include <stdint.h>

/* The two sample-rate conversions of the encoder's audio start in
 * obs-encoder.c, on plain values (obs-encoder-audio-start-frames.c):
 * - obs_encoder_audio_start_frames_offset(): calc_offset_size()'s offset
 *   from the audio's start to the video's in frames,
 *   util_mul_div64(offset_ns, samplerate, 1000000000), which it passes to
 *   obs_encoder_audio_start_offset_size();
 * - obs_encoder_audio_start_frames_duration(): buffer_audio()'s packet
 *   length in nanoseconds, util_mul_div64(frames, 1000000000, samplerate),
 *   which it passes to obs_encoder_audio_start_packet().
 *
 * obs-encoder.c keeps every read of the encoder (samplerate, blocksize),
 * the paired encoder lookup, the deques and every rule call
 * (obs-encoder-audio-start.h), each where it was and under its condition.
 * It reads samplerate once for each call here, as it did for each
 * conversion. Nothing here takes a lock, calls into libobs or keeps state.
 *
 * Hidden in libobs: not EXPORT. */

/* calc_offset_size(): the frames in offset_ns nanoseconds at samplerate,
 * rounded down. */
uint64_t obs_encoder_audio_start_frames_offset(uint64_t offset_ns, uint32_t samplerate);

/* buffer_audio(), once the paired video has started: the packet's length in
 * nanoseconds, frames at samplerate, rounded down. */
uint64_t obs_encoder_audio_start_frames_duration(uint32_t frames, uint32_t samplerate);
