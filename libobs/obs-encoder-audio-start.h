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

/* The encoder audio start alignment and the setter gates of obs-encoder.c,
 * on plain values (obs-encoder-audio-start.c). One function per decision,
 * each asked where the C made it, from values the caller reads at that point:
 * - receive_video(): obs_encoder_audio_start_group_ready() (the group's start
 *   timestamp, read under the group mutex, against the frame's),
 *   obs_encoder_audio_start_paired_late() (asked for each paired encoder the
 *   walk reaches, once it has received audio) and
 *   obs_encoder_audio_start_video_pts() (the pts step after an encode);
 * - buffer_audio(): obs_encoder_audio_start_size(),
 *   obs_encoder_audio_start_paired() and _unpaired() (the two branches),
 *   obs_encoder_audio_start_no_video() (the paired video has not started) and
 *   obs_encoder_audio_start_packet() (wait, truncate, clear, start from the
 *   buffer);
 * - calc_offset_size(): obs_encoder_audio_start_offset_ns() and
 *   obs_encoder_audio_start_offset_size(), around the conversion to frames;
 * - start_from_buffer(): obs_encoder_audio_start_buffer_offset();
 * - push_back_audio(): obs_encoder_audio_start_push_size();
 * - receive_audio(): obs_encoder_audio_start_can_send() (the send loop) and
 *   send_audio_data(): obs_encoder_audio_start_audio_pts() (the pts step);
 * - obs_encoder_set_gpu_scale_type() and obs_encoder_set_frame_rate_divisor():
 *   obs_encoder_audio_start_not_video() (the first refusal) and
 *   obs_encoder_audio_start_divisor_ok().
 *
 * obs-encoder.c keeps every read of the encoder, its group and its paired
 * encoders, in its order and under its conditions (the walk over the paired
 * encoders stops at the first one not ready, as before), the group mutex,
 * obs_weak_encoder_get_encoder() and obs_encoder_release(), the deques and
 * bfree(), do_encode(), the pause checks, the group reconfigure, the active
 * flag and initialized checks, the fps_override free and create, every log,
 * and both util_mul_div64() conversions between nanoseconds and frames (the
 * division by the sample rate traps on a zero rate on MSVC x64). It passes
 * their results here. Nothing here takes a lock, calls into libobs or keeps
 * state.
 *
 * The constant is copied below, so this header needs no core header;
 * obs-encoder.c checks it against obs-encoder.h.
 *
 * Hidden in libobs: not EXPORT. */

/* OBS_ENCODER_VIDEO (enum obs_encoder_type) */
#define OBS_ENCODER_AUDIO_START_TYPE_VIDEO 1

/* What buffer_audio() does with a packet once the paired video encoder has
 * started (its start timestamp is not 0). */
struct obs_encoder_audio_start_plan {
	/* the packet ends at or before the video's start: keep it buffered,
	 * do not start the audio */
	bool wait;
	/* the packet starts before the video: drop its frames before the
	 * video's start (calc_offset_size()) */
	bool truncate;
	/* the packet starts at or before the video: clear the buffered audio */
	bool clear;
	/* the video started before the packet: start from the buffered audio
	 * (start_from_buffer()) */
	bool from_buffer;
};

/* receive_video(), for an encoder in a group that has not started: the
 * group's start timestamp (read under its mutex) equals the frame's. */
bool obs_encoder_audio_start_group_ready(uint64_t group_start, uint64_t timestamp);

/* receive_video(), for each paired encoder the walk reaches that has
 * received audio: its first raw timestamp is after the frame's, so the video
 * waits. */
bool obs_encoder_audio_start_paired_late(uint64_t first_raw_ts, uint64_t timestamp);

/* receive_video() after a successful encode: cur_pts plus timebase_num times
 * frame_rate_divisor (a uint32_t product). */
int64_t obs_encoder_audio_start_video_pts(int64_t cur_pts, uint32_t timebase_num, uint32_t frame_rate_divisor);

/* buffer_audio(): the bytes of one plane of the packet, frames times
 * blocksize (a size_t product). */
size_t obs_encoder_audio_start_size(uint32_t frames, size_t blocksize);

/* buffer_audio(): the audio has not started (start_ts 0) and has a paired
 * video encoder. */
bool obs_encoder_audio_start_paired(uint64_t start_ts, bool has_paired);

/* buffer_audio(), when the first branch was not taken: the audio has not
 * started and has no paired video encoder, so it starts at this packet. */
bool obs_encoder_audio_start_unpaired(uint64_t start_ts, bool has_paired);

/* buffer_audio(): the paired video encoder's start timestamp is 0, so the
 * audio does not start yet. */
bool obs_encoder_audio_start_no_video(uint64_t v_start_ts);

/* buffer_audio() once the video has started: timestamp is the packet's,
 * duration_ns its length (util_mul_div64(frames, 1000000000, samplerate)),
 * v_start_ts the video's start. The packet ends at timestamp + duration_ns. */
struct obs_encoder_audio_start_plan obs_encoder_audio_start_packet(uint64_t timestamp, uint64_t duration_ns,
								   uint64_t v_start_ts);

/* calc_offset_size(): the nanoseconds from the audio's start to the
 * video's, v_start_ts - a_start_ts. */
uint64_t obs_encoder_audio_start_offset_ns(uint64_t v_start_ts, uint64_t a_start_ts);

/* calc_offset_size(): offset_frames (the offset converted with
 * util_mul_div64(offset, samplerate, 1000000000)) as a size_t, times
 * blocksize. */
size_t obs_encoder_audio_start_offset_size(uint64_t offset_frames, size_t blocksize);

/* start_from_buffer(): the buffered audio's first raw timestamp is before
 * the video's start, so its front is dropped. */
bool obs_encoder_audio_start_buffer_offset(uint64_t first_raw_ts, uint64_t v_start_ts);

/* push_back_audio(): the bytes to push of size after dropping offset_size
 * from the front; 0 (nothing pushed) when the offset covers it. */
size_t obs_encoder_audio_start_push_size(size_t size, size_t offset_size);

/* receive_audio(): the buffered bytes of plane 0 hold a whole frame. */
bool obs_encoder_audio_start_can_send(size_t buffered, size_t framesize_bytes);

/* send_audio_data() after a successful encode: cur_pts plus framesize. */
int64_t obs_encoder_audio_start_audio_pts(int64_t cur_pts, size_t framesize);

/* obs_encoder_set_gpu_scale_type() and obs_encoder_set_frame_rate_divisor()
 * after obs_encoder_valid(): type (info.type) is not
 * OBS_ENCODER_AUDIO_START_TYPE_VIDEO, the first refusal. The active and
 * initialized refusals follow in obs-encoder.c. */
bool obs_encoder_audio_start_not_video(int type);

/* obs_encoder_set_frame_rate_divisor() after the refusals: the divisor is
 * not 0. */
bool obs_encoder_audio_start_divisor_ok(uint32_t frame_rate_divisor);
