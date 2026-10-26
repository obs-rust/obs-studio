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

/* The per-frame and per-encoder gates of the GPU encode thread
 * (obs-video-gpu-encode.c), on plain values (obs-encoder-gpu-gates.c). One
 * function per decision, each asked where the C made it, from values the
 * caller reads at that point:
 * - gpu_encode_thread(), for each frame: obs_encoder_gpu_gates_warming() and
 *   _warm_next() (the frames skipped while the thread starts);
 * - for each encoder: _divisor_inc(), _divisor_wrap() and _divisor_skip()
 *   (the frame rate divisor counter), _next_key() (the keyed mutex key the
 *   encoder hands on) and _record_timing() (the frame timing entry);
 * - after the walk: _count_next(), _requeue() and _requeue_ts() (the frame
 *   goes back to the front of the queue one interval later, or to the
 *   available queue);
 * - init_gpu_encoding(): _p010() (P010 or NV12 textures).
 *
 * The group start, the paired audio wait and the pts step are the gates
 * receive_video() asks, and gpu_encode_thread() asks them too:
 * obs_encoder_audio_start_group_ready(), _paired_late() and _video_pts()
 * (obs-encoder-audio-start.h).
 *
 * obs-video-gpu-encode.c keeps os_sem_wait(), the stop flag, every lock
 * (gpu_encoder_mutex and the group mutex) and every read of the video, the
 * frame, the encoders and their groups and paired encoders, in its order and
 * under its conditions: the failed flag (an atomic load), the group and
 * first_received tests, the walk over the weak paired encoders (it stops at
 * the first one not ready, as before), video_pause_check(), the group
 * reconfigure and info.update, start_ts, the NULL test of encode_texture2,
 * both encode calls, os_gettime_ns(), the timing entry, the packet send, the
 * deques, the references, the profiler, the texture creates and
 * obs_host_texture_shared_handle(). It passes their values here. Nothing here
 * takes a lock, calls into libobs or keeps state.
 *
 * The constants are copied below, so this header needs no core header;
 * obs-video-gpu-encode.c checks them against obs-internal.h and video-io.h.
 *
 * Hidden in libobs: not EXPORT. */

/* NUM_ENCODE_TEXTURE_FRAMES_TO_WAIT */
#define OBS_ENCODER_GPU_GATES_WAIT_FRAMES 1
/* VIDEO_FORMAT_P010 (enum video_format) */
#define OBS_ENCODER_GPU_GATES_FORMAT_P010 18

/* gpu_encode_thread(): frames are still to be skipped while the thread
 * starts (wait_frames is not 0), so this one is. */
bool obs_encoder_gpu_gates_warming(int wait_frames);

/* gpu_encode_thread(), for a skipped frame: the frames left to skip,
 * wait_frames - 1. */
int obs_encoder_gpu_gates_warm_next(int wait_frames);

/* gpu_encode_thread(), for each encoder that reaches the divisor: the
 * counter after this frame, counter + 1 (a uint32_t sum). The caller keeps
 * the old value as skip. */
uint32_t obs_encoder_gpu_gates_divisor_inc(uint32_t counter);

/* gpu_encode_thread(): the counter (read again after the increment) has
 * reached frame_rate_divisor, so it goes back to 0. */
bool obs_encoder_gpu_gates_divisor_wrap(uint32_t counter, uint32_t frame_rate_divisor);

/* gpu_encode_thread(): the counter before the increment was not 0, so the
 * encoder skips this frame. */
bool obs_encoder_gpu_gates_divisor_skip(uint32_t skip);

/* gpu_encode_thread(), for each encoder that encodes the frame: lock_count
 * (already incremented) is the number of encoders taken, so the last one
 * hands on key 0; any other hands on next_key + 1 (a uint64_t sum). */
uint64_t obs_encoder_gpu_gates_next_key(size_t lock_count, size_t num_encoders, uint64_t next_key);

/* gpu_encode_thread() after an encode: the frame has a timestamp (not 0),
 * so its timing entry is recorded. */
bool obs_encoder_gpu_gates_record_timing(uint64_t frame_ts);

/* gpu_encode_thread() after the walk: the frame's count after this pass,
 * count - 1. */
int obs_encoder_gpu_gates_count_next(int count);

/* gpu_encode_thread(): the count (after count_next()) is not 0, so the frame
 * goes back to the front of the queue. */
bool obs_encoder_gpu_gates_requeue(int count);

/* gpu_encode_thread(), for a requeued frame: its timestamp plus the frame
 * interval (a uint64_t sum). */
uint64_t obs_encoder_gpu_gates_requeue_ts(uint64_t timestamp, uint64_t interval);

/* init_gpu_encoding(): the video format is VIDEO_FORMAT_P010, so the encode
 * textures are P010; any other format gives NV12. */
bool obs_encoder_gpu_gates_p010(int format);
