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

/* The two sample-rate conversions of prepare_audio() in obs-output.c, on
 * plain values (obs-output-raw-audio-trim.c). For a raw-audio packet that
 * starts before the output's first video frame:
 * - obs_output_raw_audio_trim_duration(): the packet's length in
 *   nanoseconds, util_mul_div64(frames, 1000000000, sample_rate);
 * - obs_output_raw_audio_trim_frames(): the cut in frames,
 *   util_mul_div64(cut_ns, sample_rate, 1000000000), for a packet that is
 *   kept.
 *
 * obs-output.c keeps the passthrough, the video start taken from the pause
 * data under the pause mutex and the wait, the copy of the packet, the
 * before-video test, the drop test on the packet's length
 * (obs_output_raw_audio_ends_before(), reading the timestamp and the video
 * start after the conversion), the cut in nanoseconds and the new
 * timestamp, each plane's pointer advance (audio_size read for each plane)
 * and the new frame count (obs-output-raw-audio.h), each where it was. It
 * reads sample_rate once for each call here, as it did for each conversion.
 * Nothing here takes a lock, calls into libobs or keeps state.
 *
 * Hidden in libobs: not EXPORT. */

/* prepare_audio(), for a packet that starts before the video: its length in
 * nanoseconds, frames at sample_rate, rounded down. */
uint64_t obs_output_raw_audio_trim_duration(uint32_t frames, size_t sample_rate);

/* prepare_audio(), for a packet that is kept: the frames in cut_ns
 * nanoseconds at sample_rate, rounded down. */
uint64_t obs_output_raw_audio_trim_frames(uint64_t cut_ns, size_t sample_rate);
