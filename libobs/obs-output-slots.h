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

/* The encoder slot rules and setter gates of obs-output.c, on plain values
 * (obs-output-slots.c). One function per decision, each asked where the C
 * made it, from values the caller reads at that point:
 * - get_first_video_encoder_index() and get_first_audio_encoder_index()
 *   (which obs_output_audio() now calls): obs_output_slots_first_video()
 *   and _first_audio(), the lowest occupied slot;
 * - obs_output_set_mixer(): obs_output_slots_mixer_in_range() and
 *   _mixer_mask();
 * - obs_output_remove_encoder_internal(): obs_output_slots_video_type_ok(),
 *   then _audio_type_ok(), the slots an encoder of that type can be in;
 * - obs_output_set_video_encoder2() and obs_output_set_audio_encoder():
 *   obs_output_slots_video_type_ok() / _audio_type_ok() (the type gate),
 *   _video_index_ok() / _audio_index_ok() (the index rule, after the active
 *   gate), and for video at the default index, _default_index() and
 *   _scaled_axis_set() (the preferred size step);
 * - obs_output_get_video_encoder2(), obs_output_get_audio_encoder() and
 *   obs_output_set_preferred_size2(): obs_output_slots_video_slot() /
 *   _audio_slot(), the index bounds; obs_output_set_preferred_size2() also
 *   asks _default_index() for the raw size write.
 *
 * obs-output.c keeps obs_output_valid(), the log_flag_* checks (they log as
 * they test, so it calls them in their order and returns on their result),
 * flag_encoded(), active() (an atomic, loaded where it always was), every
 * read of the output and of the encoder, the occupancy masks it builds from
 * the slots, every pointer comparison (the slot already holding the
 * encoder, the slots that hold it), every obs_encoder_get_ref() and
 * obs_encoder_release(), obs_encoder_add_output() and _remove_output(),
 * destroy_caption_track() and create_caption_track() (one for any encoder,
 * none for NULL), obs_encoder_set_scaled_size(), obs_encoder_audio(),
 * obs_output_set_mixers(), obs_output_remove_encoder(),
 * obs_output_set_service() and every log. Nothing here takes a lock, calls
 * into libobs or keeps state.
 *
 * The constants are copied below, so this header needs no core header;
 * obs-output.c checks them against obs-output.h, obs-encoder.h and
 * audio-io.h.
 *
 * Hidden in libobs: not EXPORT. */

/* OBS_OUTPUT_MULTI_TRACK_AUDIO (OBS_OUTPUT_MULTI_TRACK) and
 * OBS_OUTPUT_MULTI_TRACK_VIDEO */
#define OBS_OUTPUT_SLOTS_MULTI_TRACK_AUDIO (1 << 4)
#define OBS_OUTPUT_SLOTS_MULTI_TRACK_VIDEO (1 << 6)
/* MAX_OUTPUT_VIDEO_ENCODERS, MAX_OUTPUT_AUDIO_ENCODERS, MAX_AUDIO_MIXES */
#define OBS_OUTPUT_SLOTS_MAX_VIDEO_ENCODERS 10
#define OBS_OUTPUT_SLOTS_MAX_AUDIO_ENCODERS 6
#define OBS_OUTPUT_SLOTS_MAX_AUDIO_MIXES 6
/* OBS_ENCODER_AUDIO, OBS_ENCODER_VIDEO */
#define OBS_OUTPUT_SLOTS_ENCODER_AUDIO 0
#define OBS_OUTPUT_SLOTS_ENCODER_VIDEO 1

/* get_first_video_encoder_index(): the lowest i below
 * OBS_OUTPUT_SLOTS_MAX_VIDEO_ENCODERS whose bit is set in occupied (bit i:
 * video_encoders[i] is set), written to *index; false, and *index not
 * written, when none is. Bits from OBS_OUTPUT_SLOTS_MAX_VIDEO_ENCODERS up are
 * ignored. */
bool obs_output_slots_first_video(uint32_t occupied, size_t *index);

/* get_first_audio_encoder_index(): the same over audio_encoders, below
 * OBS_OUTPUT_SLOTS_MAX_AUDIO_ENCODERS. */
bool obs_output_slots_first_audio(uint32_t occupied, size_t *index);

/* obs_output_set_mixer(): mixer_idx is below
 * OBS_OUTPUT_SLOTS_MAX_AUDIO_MIXES; otherwise the caller logs and ignores
 * it. */
bool obs_output_slots_mixer_in_range(size_t mixer_idx);

/* obs_output_set_mixer(): the mixer mask of mixer_idx alone,
 * (size_t)1 << mixer_idx. Asked only in range. */
size_t obs_output_slots_mixer_mask(size_t mixer_idx);

/* obs_output_set_video_encoder2(), for an encoder that is not NULL: its type
 * (an enum obs_encoder_type value) is OBS_ENCODER_VIDEO; otherwise the caller
 * logs and refuses it. obs_output_remove_encoder_internal(): the encoder can
 * be in video_encoders, which it then clears where they hold it. */
bool obs_output_slots_video_type_ok(int encoder_type);

/* obs_output_set_audio_encoder(), for an encoder that is not NULL: its type
 * is OBS_ENCODER_AUDIO; otherwise the caller logs and refuses it.
 * obs_output_remove_encoder_internal(), for an encoder that is not a video
 * one: it can be in audio_encoders. A type that is neither is in no slot. */
bool obs_output_slots_audio_type_ok(int encoder_type);

/* obs_output_set_video_encoder2(), after the active gate: with
 * OBS_OUTPUT_MULTI_TRACK_VIDEO in flags, idx is below
 * OBS_OUTPUT_SLOTS_MAX_VIDEO_ENCODERS; without it, idx is 0. Otherwise the
 * caller returns. */
bool obs_output_slots_video_index_ok(uint32_t flags, size_t idx);

/* obs_output_set_audio_encoder(), after the active gate: with
 * OBS_OUTPUT_MULTI_TRACK_AUDIO in flags, idx is below
 * OBS_OUTPUT_SLOTS_MAX_AUDIO_ENCODERS; without it, idx is 0. */
bool obs_output_slots_audio_index_ok(uint32_t flags, size_t idx);

/* obs_output_set_video_encoder2() and obs_output_set_preferred_size2(): idx
 * is the default index 0, the one that keeps the single-encoder behaviour
 * (the encoder takes the preferred size; a raw output's scaled size). */
bool obs_output_slots_default_index(size_t idx);

/* obs_output_set_video_encoder2() at the default index: one axis of the
 * output's scaled size is set (not 0). Asked for scaled_width, then for
 * scaled_height only when scaled_width is set; both set, the encoder takes
 * the size. */
bool obs_output_slots_scaled_axis_set(uint32_t size);

/* obs_output_get_video_encoder2() and obs_output_set_preferred_size2(): idx
 * is a video slot, below OBS_OUTPUT_SLOTS_MAX_VIDEO_ENCODERS, whatever the
 * flags. */
bool obs_output_slots_video_slot(size_t idx);

/* obs_output_get_audio_encoder(): idx is an audio slot, below
 * OBS_OUTPUT_SLOTS_MAX_AUDIO_ENCODERS, whatever the flags. */
bool obs_output_slots_audio_slot(size_t idx);
