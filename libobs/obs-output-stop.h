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

/* The output stop and end-capture steps of obs-output.c, on plain values
 * (obs-output-stop.c). One function per decision, each asked where the C
 * made it, from values the caller reads at that point:
 * - obs_output_stop(): obs_output_stop_encoded() and _delay_on(), the
 *   delayed stop (an encoded output with a delay running);
 * - end_data_capture_thread(): obs_output_stop_has_video() and _has_audio()
 *   (which encoders or raw connections stop), _encoded() (the encoders, or
 *   the raw video and audio), _delay_on() (the callback is process_delay(),
 *   and the delay is cleaned up) and _service() (the service is
 *   deactivated);
 * - obs_output_end_data_capture_internal(): obs_output_stop_encoded() (the
 *   unstarted encoders are shut down), _end_delay() (the delay ends, or its
 *   restart references keep it), _has_video() (the frame log) and
 *   _thread_failed() (the inline fallback, for both threads).
 * end_data_capture_thread() asks obs_output_raw_audio_interleave() whether
 * an undelayed encoded output's callback is interleave_packets(), and
 * stop_raw_audio() asks obs_output_raw_audio_mix_on() for each mix with
 * raw_audio2, the rules hook_data_capture() and start_raw_audio() ask
 * (obs-output-raw-audio.h).
 *
 * obs-output.c keeps obs_output_valid(), the context.data and raw_audio2
 * tests, every atomic load and store (active, data_active, reconnecting,
 * starting, delay_active, delay_capturing, delay_restart_refs and the end
 * capture thread's flags) and the stopping event test, each loaded where it
 * always was and tested in its && order, the signal and force tests, the
 * unpause, the event resets and signals, the reconnect thread's join,
 * info.stop, obs_output_end_data_capture(), obs_output_delay_stop(),
 * obs_encoder_stop(), obs_encoder_shutdown_unstarted(), stop_raw_video(),
 * audio_output_disconnect(), get_first_mixer(), obs_service_deactivate(),
 * obs_output_cleanup_delay(), log_frame_info(), the caption frees under
 * their mutex, the keyframe tracking clear, the joins and pthread_create(),
 * the stop parameters copied before a thread starts, every signal and every
 * log. Nothing here takes a lock, calls into libobs or keeps state.
 *
 * The constants are copied below, so this header needs no core header;
 * obs-output.c checks them against obs-output.h.
 *
 * Hidden in libobs: not EXPORT. */

/* OBS_OUTPUT_VIDEO, OBS_OUTPUT_AUDIO, OBS_OUTPUT_ENCODED, OBS_OUTPUT_SERVICE */
#define OBS_OUTPUT_STOP_VIDEO (1 << 0)
#define OBS_OUTPUT_STOP_AUDIO (1 << 1)
#define OBS_OUTPUT_STOP_ENCODED (1 << 2)
#define OBS_OUTPUT_STOP_SERVICE (1 << 3)

/* obs_output_stop(), for an active or reconnecting output that is not
 * reconnecting any more, and end_data_capture_thread(): with
 * OBS_OUTPUT_ENCODED in flags (info.flags), the stop goes through the delay
 * (obs_output_stop()) or the encoders are stopped, not the raw video and
 * audio (end_data_capture_thread()). obs_output_end_data_capture_internal(),
 * for an output that is not active: the encoders it initialized are shut
 * down on a thread. */
bool obs_output_stop_encoded(uint32_t flags);

/* obs_output_stop(), for an encoded output, and end_data_capture_thread():
 * active_delay_ns is not 0, so a delay is running: the stop goes through
 * obs_output_delay_stop(), the encoders are stopped with process_delay() and
 * the delay is cleaned up. */
bool obs_output_stop_delay_on(uint64_t active_delay_ns);

/* end_data_capture_thread(): with OBS_OUTPUT_VIDEO in flags, the video
 * encoders (or the raw video) are stopped. obs_output_end_data_capture_internal():
 * the frame counts are logged. */
bool obs_output_stop_has_video(uint32_t flags);

/* end_data_capture_thread(): with OBS_OUTPUT_AUDIO in flags, the audio
 * encoders (or the raw audio) are stopped. */
bool obs_output_stop_has_audio(uint32_t flags);

/* end_data_capture_thread(), after the encoders: with OBS_OUTPUT_SERVICE in
 * flags, the service is deactivated. */
bool obs_output_stop_service(uint32_t flags);

/* obs_output_end_data_capture_internal(), for an active output with its
 * data active and a delay active, after delay_capturing is cleared:
 * delay_restart_refs (loaded) is 0, so the delay ends (delay_active is
 * cleared and the capture ends); otherwise a restart keeps it, and the
 * caller signals the stopping event and returns. */
bool obs_output_stop_end_delay(long delay_restart_refs);

/* obs_output_end_data_capture_internal(): pthread_create() returned ret, not
 * 0, so the thread did not start: the unstarted encoders' shutdown is left
 * (end_data_capture_thread_active is cleared), or end_data_capture_thread()
 * runs inline after a warning. */
bool obs_output_stop_thread_failed(int ret);
