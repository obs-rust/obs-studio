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

/* The audio input steps of process_audio() and reset_resampler() in
 * obs-source.c, on plain values (obs-source-audio-input.c):
 * - obs_source_audio_input_discontinuous(): a packet more than
 *   OBS_SOURCE_AUDIO_INPUT_SMOOTHING_THRESHOLD from where the previous one
 *   ended starts a new stream, so the resampler drops what it holds back
 *   (#546). Only with a resampler and an end set.
 * - obs_source_audio_input_reset_needed(): the resampler is reset when the
 *   packet's rate, format or speakers differ from the source's, when the
 *   clock drift compensation flag changed, or on a discontinuity.
 * - obs_source_audio_input_resampler_needed(): reset_resampler() creates a
 *   resampler unless the source's rate, format and speakers are the
 *   output's and the compensation is off (the clock drift compensation
 *   needs one even when the formats match).
 * - obs_source_audio_input_balance_needed() and
 *   obs_source_audio_input_mono_needed(): the balance gate (a stereo
 *   source, an output that is not mono, a balance outside 0.49 to 0.51)
 *   and the Force Mono gate (an output that is not mono and the flag set).
 * - obs_source_audio_input_balance(): the sine, square and linear laws over
 *   the left and right planes; any other law changes nothing.
 * - obs_source_audio_input_downmix(): the planes' mean into every plane.
 *
 * process_audio() keeps its order: the steps, the reset_resampler() call,
 * next_resample_in_ts (computed in obs-source.c, which passes it in), the
 * resampler, copy_audio_data() and its allocation. reset_resampler() keeps
 * the stores, the resampler's creation and destruction and the log line.
 * The clock drift filter and audio_resampler_set_compensation_error() stay
 * in obs-source.c; the steps read only the compensation flag.
 *
 * None of them takes a lock, calls into libobs, allocates or keeps state.
 * The two loops write only the planes they are given, and only the first
 * frames of each.
 *
 * Hidden in libobs: not EXPORT. */

/* in nanoseconds; obs-source.c checks that it is its TS_SMOOTHING_THRESHOLD */
#define OBS_SOURCE_AUDIO_INPUT_SMOOTHING_THRESHOLD 70000000ULL

/* obs-source.c checks that they are SPEAKERS_STEREO,
 * OBS_SOURCE_FLAG_FORCE_MONO and enum obs_balance_type's values */
#define OBS_SOURCE_AUDIO_INPUT_SPEAKERS_STEREO 2
#define OBS_SOURCE_AUDIO_INPUT_FORCE_MONO (1u << 1)
#define OBS_SOURCE_AUDIO_INPUT_SINE_LAW 0
#define OBS_SOURCE_AUDIO_INPUT_SQUARE_LAW 1
#define OBS_SOURCE_AUDIO_INPUT_LINEAR 2

/* An audio format: struct resample_info's fields. The enums cross as int;
 * obs-source.c checks their sizes. */
struct obs_source_audio_input_format {
	/* enum audio_format */
	int format;
	uint32_t samples_per_sec;
	/* enum speaker_layout */
	int speakers;
};

/* Whether the packet at timestamp starts a new stream: has_resampler
 * (source->resampler != NULL), next_resample_in_ts set (not 0), and the
 * packet more than the threshold away from it, either way. */
bool obs_source_audio_input_discontinuous(bool has_resampler, uint64_t next_resample_in_ts, uint64_t timestamp);

/* Whether process_audio() calls reset_resampler(): current is
 * source->sample_info, packet the packet's format, last_compensation
 * source->last_async_compensation and compensation the loaded
 * source->async_compensation. */
bool obs_source_audio_input_reset_needed(struct obs_source_audio_input_format current,
					 struct obs_source_audio_input_format packet, bool last_compensation,
					 bool compensation, bool discontinuous);

/* Whether reset_resampler() creates a resampler: source is the new
 * source->sample_info, output the audio output's format and compensation
 * source->last_async_compensation. */
bool obs_source_audio_input_resampler_needed(struct obs_source_audio_input_format source,
					     struct obs_source_audio_input_format output, bool compensation);

/* Whether process_audio() balances: mono_output (the output has one
 * channel), speakers (source->sample_info.speakers) and source->balance. */
bool obs_source_audio_input_balance_needed(bool mono_output, int speakers, float balance);

/* Whether process_audio() downmixes: mono_output and source->flags. */
bool obs_source_audio_input_mono_needed(bool mono_output, uint32_t flags);

/* Applies law to the first frames of data[0] (left) and data[1] (right).
 * Any other law reads and writes nothing. */
void obs_source_audio_input_balance(float **data, uint32_t frames, float balance, int law);

/* Sums the first frames of data[1] .. data[channels - 1] into data[0],
 * scales it by 1 / channels and copies it back to every other plane. */
void obs_source_audio_input_downmix(float **data, size_t channels, uint32_t frames);
