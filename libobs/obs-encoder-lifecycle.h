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

/* The encoder callback and connection lifecycle decisions of obs-encoder.c, on
 * plain values (obs-encoder-lifecycle.c). One function per decision, each
 * asked where the C made it, from values the caller reads at that point:
 * - obs_encoder_start_internal(), under callbacks_mutex:
 *   obs_encoder_lifecycle_first() (no callback before this one, so the
 *   encoder connects) and _found() (the callback is already there, so it is
 *   not added); after the mutex, the keyframe request for a video encoder
 *   that gains a callback while running asks
 *   obs_encoder_audio_start_not_video() (obs-encoder-audio-start.h), the
 *   video type test the setters already ask;
 * - obs_encoder_stop(), under callbacks_mutex: _found() (the callback is
 *   erased) and _last() (none is left, so the encoder disconnects); after
 *   the disconnect, under the group mutex, _group_idle() (with
 *   destroy_on_stop, the group is destroyed);
 * - add_connection() and remove_connection(): the route (the audio mix, else
 *   the GPU encode or the raw video) asks obs_encoder_bind_not_audio()
 *   (obs-encoder-bind.h), the audio type refusal of the audio setter, negated;
 *   under the group mutex, add_connection() asks _lcm_unset() (the divisor LCM
 *   starts at 1), _started_inc() and _group_ready() (every encoder of the
 *   group has started), and remove_connection() _started_dec() and
 *   _group_idle() (the last started encoder stopped, so the group is reset);
 * - gpu_encode_available(), which full_stop() also asks: _pass_texture() (the
 *   texture cap), after the mix test and before its texture flags;
 * - send_first_video_packet(): _prepend_sei() (the SEI goes in front of the
 *   first keyframe); send_packet() asks obs_encoder_audio_start_not_video()
 *   for the first packet route.
 *
 * obs-encoder.c keeps init_mutex, callbacks_mutex, outputs_mutex, the group
 * mutex and every section they guard, the darrays (the callback push and
 * erase, the SEI copy), get_callback_idx() (a walk; its result is passed),
 * the context.data, media, request_keyframe, destroy_on_stop, keyframe,
 * sent_first_packet, mix and texture flag tests (each a single field, tested
 * where it always was, in its && or || order), lcm(), audio_output_connect()
 * and _disconnect(), start_gpu_encode() and stop_gpu_encode(),
 * start_raw_video() and stop_raw_video(), get_audio_info(), get_video_info(),
 * get_mix_for_video(), add_ready_encoder_group(), the group destroy,
 * obs_encoder_shutdown(), set_encoder_active(), pause_reset(),
 * request_keyframe(), get_sei(), obs_output_active(), obs_output_force_stop()
 * and the NULL packet, the callbacks and the profiler, and every write to
 * the encoder, its group and its callbacks. It passes their values here.
 * Nothing here takes a lock, calls into libobs or keeps state, and nothing
 * divides.
 *
 * The constants are copied below, so this header needs no core header;
 * obs-encoder.c checks them against obs-encoder.h and util/darray.h.
 *
 * Hidden in libobs: not EXPORT. */

/* OBS_ENCODER_CAP_PASS_TEXTURE */
#define OBS_ENCODER_LIFECYCLE_CAP_PASS_TEXTURE (1 << 1)
/* DARRAY_INVALID, get_callback_idx()'s "not found" */
#define OBS_ENCODER_LIFECYCLE_NOT_FOUND ((size_t)-1)

/* obs_encoder_start_internal() under callbacks_mutex, before the add:
 * num_callbacks (callbacks.num) is 0, so this is the encoder's first callback
 * and it connects once the mutex is released. */
bool obs_encoder_lifecycle_first(size_t num_callbacks);

/* obs_encoder_start_internal() and obs_encoder_stop() under callbacks_mutex:
 * idx, get_callback_idx()'s result, is not OBS_ENCODER_LIFECYCLE_NOT_FOUND.
 * The start adds the callback only when it is not found; the stop erases it
 * only when it is. */
bool obs_encoder_lifecycle_found(size_t idx);

/* obs_encoder_stop() under callbacks_mutex, after the erase: num_callbacks
 * (callbacks.num) is 0, so that was the last callback and the encoder
 * disconnects. */
bool obs_encoder_lifecycle_last(size_t num_callbacks);

/* gpu_encode_available() with a mix: caps (info.caps) has
 * OBS_ENCODER_LIFECYCLE_CAP_PASS_TEXTURE. Only then does the caller test the
 * mix's P010 and NV12 texture flags. */
bool obs_encoder_lifecycle_pass_texture(uint32_t caps);

/* add_connection() under the group mutex: lcm (frame_rate_divisors_lcm) is 0,
 * so the caller sets it to 1 before taking the LCM with the encoder's
 * divisor (lcm() stays in C). */
bool obs_encoder_lifecycle_lcm_unset(uint32_t lcm);

/* add_connection() under the group mutex: num_encoders_started plus one,
 * wrapping as the C's uint32_t does. */
uint32_t obs_encoder_lifecycle_started_inc(uint32_t started);

/* add_connection() under the group mutex, after the count: started (the new
 * num_encoders_started) is at least num_encoders (encoders.num), so the group
 * is ready; the caller adds it after releasing the mutex. */
bool obs_encoder_lifecycle_group_ready(uint32_t started, size_t num_encoders);

/* remove_connection() under the group mutex: num_encoders_started minus one,
 * wrapping as the C's uint32_t does. */
uint32_t obs_encoder_lifecycle_started_dec(uint32_t started);

/* remove_connection() under the group mutex, after the count: started (the new
 * num_encoders_started) is 0, so the group's start and reconfigure state is
 * reset. obs_encoder_stop() asks the same of the group's count, after
 * destroy_on_stop, to destroy the group. */
bool obs_encoder_lifecycle_group_idle(uint32_t started);

/* send_first_video_packet() on the first keyframe: got is get_sei()'s result,
 * has_sei whether the SEI pointer is set and size its size (NULL and 0 when
 * get_sei() set neither). The SEI is copied in front of the packet only when
 * all three hold; otherwise the packet is sent as it is. */
bool obs_encoder_lifecycle_prepend_sei(bool got, bool has_sei, size_t size);
