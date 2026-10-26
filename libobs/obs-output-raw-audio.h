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

/* The raw output audio admission and the capture hookup of obs-output.c, on
 * plain values (obs-output-raw-audio.c). One function per decision, each
 * asked where the C made it, from values the caller reads at that point:
 * - prepare_audio(): obs_output_raw_audio_passthrough() (an output without
 *   video takes the packet as it is), obs_output_raw_audio_video_unset()
 *   (asked before the video start is taken from the pause data, and again
 *   after), obs_output_raw_audio_before_video(),
 *   obs_output_raw_audio_ends_before() (drop the packet), and the front cut:
 *   obs_output_raw_audio_cut_ns(), _cut_ts(), _cut_bytes() (asked for each
 *   plane) and _cut_frames();
 * - default_raw_audio_callback(): obs_output_raw_audio_start_unset() (the
 *   first packet sets the audio start), _block_bytes(), _push_bytes() (asked
 *   for each plane), _can_pop() (the block loop), _block_ts(), _paused_ts()
 *   (under the pause mutex) and _next_frames();
 * - start_raw_audio(): obs_output_raw_audio_mix_on() (asked for each mix
 *   with raw_audio2);
 * - hook_data_capture(): obs_output_raw_audio_interleave() (the encoded
 *   callback), obs_output_raw_audio_delay_on() and _delay_ns();
 *   obs_output_start() asks obs_output_raw_audio_delay_on() too, whether the
 *   start is delayed.
 *
 * obs-output.c keeps data_active(), every read of the output, the pause
 * mutex and its reads (last_video_ts only while video_start_ts is 0, and
 * ts_offset), audio_pause_check(), the deques and the pointer advance of
 * each plane, the raw_audio and raw_audio2 callbacks and the NULL test of
 * raw_audio2, get_first_mixer(), audio_output_connect(), start_raw_video(),
 * reset_packet_data() under interleaved_mutex, obs_encoder_start(), the
 * delay's fields, os_atomic_set_bool() and log, and every conversion that
 * divides by the sample rate: util_mul_div64(frames, 1000000000,
 * sample_rate), util_mul_div64(cutoff, sample_rate, 1000000000) and
 * audio_frames_to_ns() (the division traps on a zero rate on MSVC x64). It
 * passes their results here. Nothing here takes a lock, calls into libobs or
 * keeps state.
 *
 * The constants are copied below, so this header needs no core header;
 * obs-output.c checks them against obs-output.h and audio-io.h.
 *
 * Hidden in libobs: not EXPORT. */

/* OBS_OUTPUT_VIDEO */
#define OBS_OUTPUT_RAW_AUDIO_FLAG_VIDEO (1 << 0)
/* AUDIO_OUTPUT_FRAMES */
#define OBS_OUTPUT_RAW_AUDIO_FRAMES 1024

/* prepare_audio(): the output has no OBS_OUTPUT_VIDEO flag, so the packet
 * passes as it is. */
bool obs_output_raw_audio_passthrough(uint32_t flags);

/* prepare_audio(): video_start_ts is 0. Asked first to take it from the
 * pause data (under the pause mutex), then again: still 0, the packet
 * waits. */
bool obs_output_raw_audio_video_unset(uint64_t video_start_ts);

/* prepare_audio(): the packet starts before the video. */
bool obs_output_raw_audio_before_video(uint64_t timestamp, uint64_t video_start_ts);

/* prepare_audio(), for a packet that starts before the video: it ends at or
 * before the video's start, so it is dropped. duration_ns is
 * util_mul_div64(frames, 1000000000, sample_rate); the end is timestamp +
 * duration_ns (a uint64_t sum). */
bool obs_output_raw_audio_ends_before(uint64_t timestamp, uint64_t duration_ns, uint64_t video_start_ts);

/* prepare_audio(): the nanoseconds cut from the packet's front,
 * video_start_ts - timestamp. */
uint64_t obs_output_raw_audio_cut_ns(uint64_t video_start_ts, uint64_t timestamp);

/* prepare_audio(): the cut packet's timestamp, timestamp + cut_ns. */
uint64_t obs_output_raw_audio_cut_ts(uint64_t timestamp, uint64_t cut_ns);

/* prepare_audio(), for each plane: the bytes the plane's data advances,
 * audio_size times cut_frames (util_mul_div64(cut_ns, sample_rate,
 * 1000000000)) truncated to uint32_t. */
size_t obs_output_raw_audio_cut_bytes(size_t audio_size, uint64_t cut_frames);

/* prepare_audio(): the frames left, frames - (uint32_t)cut_frames. */
uint32_t obs_output_raw_audio_cut_frames(uint32_t frames, uint64_t cut_frames);

/* default_raw_audio_callback(): audio_start_ts is 0, so this packet's
 * timestamp becomes the audio's start. */
bool obs_output_raw_audio_start_unset(uint64_t audio_start_ts);

/* default_raw_audio_callback(): the bytes of one block of one plane,
 * OBS_OUTPUT_RAW_AUDIO_FRAMES times audio_size. */
size_t obs_output_raw_audio_block_bytes(size_t audio_size);

/* default_raw_audio_callback(), for each plane: the bytes pushed, frames
 * times audio_size. */
size_t obs_output_raw_audio_push_bytes(uint32_t frames, size_t audio_size);

/* default_raw_audio_callback(): the block loop pops a block while plane 0
 * holds more than one (buffered > block_bytes). */
bool obs_output_raw_audio_can_pop(size_t buffered, size_t block_bytes);

/* default_raw_audio_callback(): a block's timestamp before the pause offset,
 * audio_start_ts + frames_ns (audio_frames_to_ns(sample_rate,
 * total_audio_frames)). */
uint64_t obs_output_raw_audio_block_ts(uint64_t audio_start_ts, uint64_t frames_ns);

/* default_raw_audio_callback(), under the pause mutex: timestamp +
 * ts_offset. */
uint64_t obs_output_raw_audio_paused_ts(uint64_t timestamp, uint64_t ts_offset);

/* default_raw_audio_callback(): total_audio_frames after a block,
 * total_audio_frames + OBS_OUTPUT_RAW_AUDIO_FRAMES. */
size_t obs_output_raw_audio_next_frames(size_t total_audio_frames);

/* start_raw_audio() with raw_audio2, for each mix below MAX_AUDIO_MIXES:
 * bit idx of mixer_mask is set. */
bool obs_output_raw_audio_mix_on(size_t mixer_mask, size_t idx);

/* hook_data_capture(), for an encoded output: interleave_packets() when it
 * has both video and audio, else default_encoded_callback(). */
bool obs_output_raw_audio_interleave(bool has_video, bool has_audio);

/* hook_data_capture(): delay_sec is not 0, so process_delay() wraps the
 * callback. obs_output_start() asks it too: the start goes through
 * obs_output_delay_start() (obs-output-start.h). */
bool obs_output_raw_audio_delay_on(uint32_t delay_sec);

/* hook_data_capture(): active_delay_ns, delay_sec times 1000000000 (a
 * uint64_t product). */
uint64_t obs_output_raw_audio_delay_ns(uint32_t delay_sec);
