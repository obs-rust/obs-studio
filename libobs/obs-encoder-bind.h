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

/* The encoder media binding and shutdown checks of obs-encoder.c, on plain
 * values (obs-encoder-bind.c). One function per decision, each asked where
 * the C made it, from values the caller reads at that point:
 * - obs_encoder_set_audio() and obs_encoder_audio(): obs_encoder_bind_not_audio()
 *   (the type refusal). The video setter, obs_encoder_video(),
 *   obs_encoder_parent_video() and obs_encoder_unbind_video() ask
 *   obs_encoder_audio_start_not_video() (obs-encoder-audio-start.h), the
 *   type refusal obs_encoder_set_gpu_scale_type() and
 *   obs_encoder_set_frame_rate_divisor() already ask;
 * - encoder_set_video(): obs_encoder_bind_video_timebase() (the timebase
 *   from the video's frame rate, or none) and _divisor_override() (whether
 *   the frame rate divisor video is created);
 * - obs_encoder_set_audio(): obs_encoder_bind_audio_timebase() (the
 *   timebase from the sample rate, or none);
 * - obs_encoder_shutdown_unstarted(): obs_encoder_bind_locked() (each
 *   pthread_mutex_trylock() result) and _shutdown_unstarted() (shut down an
 *   encoder that is initialized and that nothing needs).
 *
 * obs-encoder.c keeps obs_encoder_valid(), every warning, the active flag
 * (an atomic, loaded where it always was) and the initialized, media,
 * fps_override and context.data tests (each a single field, tested where it
 * always was, in its && order), init_mutex and outputs_mutex with both
 * pthread_mutex_trylock() calls and their unlock order, the walk over videos
 * (it computes bound and stops at the first match) and the walk over
 * encoder->outputs (it computes needed and stops at the first output that
 * needs the encoder), obs_encoder_shutdown() with info.destroy, the resets,
 * maybe_clear_encoder_core_video_mix() and the paired-encoder weak releases,
 * obs_encoder_set_last_error(), video_output_get_info(),
 * audio_output_get_sample_rate(), video_output_free_frame_rate_divisor() and
 * video_output_create_with_frame_rate_divisor(), and every write to the
 * encoder. It passes their values here. Nothing here takes a lock, calls into
 * libobs or keeps state, and nothing divides.
 *
 * The constant is copied below, so this header needs no core header;
 * obs-encoder.c checks it against obs-encoder.h.
 *
 * Hidden in libobs: not EXPORT. */

/* OBS_ENCODER_AUDIO (enum obs_encoder_type) */
#define OBS_ENCODER_BIND_TYPE_AUDIO 0

/* An encoder's timebase_num and timebase_den. */
struct obs_encoder_bind_timebase {
	uint32_t num;
	uint32_t den;
};

/* obs_encoder_set_audio() and obs_encoder_audio() after obs_encoder_valid():
 * type (info.type, an enum obs_encoder_type value) is not
 * OBS_ENCODER_BIND_TYPE_AUDIO, so the caller logs and refuses. The setter's
 * active refusal follows in obs-encoder.c; it has no initialized refusal.
 * Negated, it is the connection route of add_connection() and
 * remove_connection() (obs-encoder-lifecycle.h): the audio mix, else the GPU
 * encode or the raw video. */
bool obs_encoder_bind_not_audio(int type);

/* encoder_set_video(): has_video is whether a video is set, fps_num and
 * fps_den its video_output_get_info() frame rate (read only when has_video,
 * 0 otherwise). With a video, the timebase is the frame period, fps_den over
 * fps_num; without one, 0 over 0. */
struct obs_encoder_bind_timebase obs_encoder_bind_video_timebase(bool has_video, uint32_t fps_num, uint32_t fps_den);

/* obs_encoder_set_audio() after the refusals: has_audio is whether an audio
 * is set, sample_rate its audio_output_get_sample_rate() (read only when
 * has_audio, 0 otherwise). With an audio, the timebase is 1 over the sample
 * rate; without one, 0 over 0. */
struct obs_encoder_bind_timebase obs_encoder_bind_audio_timebase(bool has_audio, uint32_t sample_rate);

/* encoder_set_video() with a video: the encoder's frame_rate_divisor is not
 * 0, so the video with that divisor (fps_override) is created. */
bool obs_encoder_bind_divisor_override(uint32_t frame_rate_divisor);

/* obs_encoder_shutdown_unstarted(): trylock_result, a
 * pthread_mutex_trylock() result, is 0, so the mutex is held. Asked for
 * outputs_mutex, then for init_mutex only when outputs_mutex is held. */
bool obs_encoder_bind_locked(int trylock_result);

/* obs_encoder_shutdown_unstarted() with both mutexes held: initialized is
 * the encoder's initialized field, needed whether it is active or any of its
 * outputs needs its encoders (the caller's walk, which stops at the first).
 * Shut it down when it is initialized and not needed. */
bool obs_encoder_bind_shutdown_unstarted(bool initialized, bool needed);
