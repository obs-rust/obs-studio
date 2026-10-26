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

/* The output start and begin-capture steps of obs-output.c, on plain values
 * (obs-output-start.c). One function per decision, each asked where the C
 * made it, from values the caller reads at that point:
 * - obs_output_start(): obs_output_start_service(), the service gate (the
 *   service is asked whether it can connect, then initialized);
 * - obs_output_actual_start(): obs_output_start_drop_restart_ref(), a
 *   delayed restart's reference is dropped;
 * - obs_output_initialize_encoders(): obs_output_start_init_video() and
 *   _init_audio(), which encoders are initialized;
 * - obs_output_begin_data_capture_internal(): obs_output_start_reset_raw()
 *   (a raw output's audio and timestamps are reset), _pair() (the video and
 *   audio encoders are paired) and _service() (the service is activated).
 * obs_output_start() asks obs_output_raw_audio_delay_on() whether the start
 * is delayed, the rule hook_data_capture() asks (obs-output-raw-audio.h),
 * and reset_raw_output() takes each audio conversion field that is set with
 * obs_encoder_size_audio_defaults(), the rule get_audio_info() fills an
 * encoder's audio with (obs-encoder-size.h).
 *
 * obs-output.c keeps obs_output_valid(), log_flag_encoded() (it logs as it
 * tests), the context.data test, every atomic load and store (starting,
 * delay_active, active, reconnecting, data_active, delay_capturing and
 * delay_restart_refs), each loaded where it always was and tested in its
 * && order, the success and audio_conversion_set tests, the stopping event
 * wait, info.start, obs_video_stats_frame_counts(), the caption track reset
 * under its mutex, obs_service_can_try_to_connect(), obs_service_initialize()
 * and obs_service_activate(), obs_output_delay_start(), the encoder
 * initialization and the last-error copy, can_begin_data_capture(),
 * pair_encoders() (its locks and weak references), hook_data_capture(),
 * calculate_batch_size(), clear_raw_audio_buffers(), audio_output_get_info(),
 * get_audio_planes(), get_audio_size(), pause_reset(), every signal and every
 * log. Nothing here takes a lock, calls into libobs or keeps state.
 *
 * The constants are copied below, so this header needs no core header;
 * obs-output.c checks them against obs-output.h.
 *
 * Hidden in libobs: not EXPORT. */

/* OBS_OUTPUT_VIDEO, OBS_OUTPUT_AUDIO, OBS_OUTPUT_ENCODED, OBS_OUTPUT_SERVICE */
#define OBS_OUTPUT_START_VIDEO (1 << 0)
#define OBS_OUTPUT_START_AUDIO (1 << 1)
#define OBS_OUTPUT_START_ENCODED (1 << 2)
#define OBS_OUTPUT_START_SERVICE (1 << 3)

/* obs_output_start(), after the "already starting" refusal: with
 * OBS_OUTPUT_SERVICE in flags (info.flags), the start needs the service,
 * which the caller then asks whether it can connect and initializes, in that
 * order, refusing when either fails. obs_output_begin_data_capture_internal(),
 * after the encoders are hooked up: the service is activated. */
bool obs_output_start_service(uint32_t flags);

/* obs_output_actual_start(), after info.start and the starting store:
 * delay_restart_refs (loaded) is not 0, so a delayed restart is under way
 * and the caller drops one reference. */
bool obs_output_start_drop_restart_ref(long delay_restart_refs);

/* obs_output_initialize_encoders(), for an encoded output that is not
 * active: with OBS_OUTPUT_VIDEO in flags, the video encoders are
 * initialized (a failure returns false before the audio ones). */
bool obs_output_start_init_video(uint32_t flags);

/* obs_output_initialize_encoders(), after the video encoders: with
 * OBS_OUTPUT_AUDIO in flags, the audio encoders are initialized. */
bool obs_output_start_init_audio(uint32_t flags);

/* obs_output_begin_data_capture_internal(), for an output that is neither
 * delayed nor active: without OBS_OUTPUT_ENCODED in flags (a raw output),
 * reset_raw_output() runs before the capture check. */
bool obs_output_start_reset_raw(uint32_t flags);

/* obs_output_begin_data_capture_internal(), after the capture check: with
 * both OBS_OUTPUT_VIDEO and OBS_OUTPUT_AUDIO in flags, the video and audio
 * encoders are paired. */
bool obs_output_start_pair(uint32_t flags);
