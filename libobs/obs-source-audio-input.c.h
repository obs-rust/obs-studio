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

#include "libobs/build/libobs-ported-source-audio-input.pch.h"

/* The audio input steps of process_audio() and reset_resampler(), moved
 * from obs-source.c: the decisions and the two loops, on plain values.
 * obs-source.c reads the source's fields, calls the steps and keeps the
 * stores, next_resample_in_ts, the resampler, copy_audio_data() and the
 * clock drift compensation. Declared in obs-source-audio-input.h without
 * EXPORT. In hybrid libobs the Rust port (rust/obs-source) replaces this
 * file. */

/* The sine law's M_PI must be <math.h>'s double, not graphics/math-defs.h's
 * float fallback. */
_Static_assert(sizeof(M_PI) == sizeof(double), "M_PI is a double");

static inline uint64_t audio_input_diff(uint64_t ts1, uint64_t ts2)
{
	return (ts1 < ts2) ? (ts2 - ts1) : (ts1 - ts2);
}

bool obs_source_audio_input_discontinuous(bool has_resampler, uint64_t next_resample_in_ts, uint64_t timestamp)
{
	/* The resampler holds back the last samples of every packet for its
	 * filter. A packet that does not continue the previous one (a media
	 * file started again after it ended) begins a new stream: drop what is
	 * held back instead of playing it in front of the new audio. */
	return has_resampler && next_resample_in_ts &&
	       audio_input_diff(timestamp, next_resample_in_ts) >= OBS_SOURCE_AUDIO_INPUT_SMOOTHING_THRESHOLD;
}

bool obs_source_audio_input_reset_needed(struct obs_source_audio_input_format current,
					 struct obs_source_audio_input_format packet, bool last_compensation,
					 bool compensation, bool discontinuous)
{
	return current.samples_per_sec != packet.samples_per_sec || current.format != packet.format ||
	       current.speakers != packet.speakers || last_compensation != compensation || discontinuous;
}

bool obs_source_audio_input_resampler_needed(struct obs_source_audio_input_format source,
					     struct obs_source_audio_input_format output, bool compensation)
{
	/* Clock drift compensation needs a resampler even when the formats
	 * match; it passes the audio through untouched until it is needed. */
	if (source.samples_per_sec == output.samples_per_sec && source.format == output.format &&
	    source.speakers == output.speakers && !compensation)
		return false;

	return true;
}

bool obs_source_audio_input_balance_needed(bool mono_output, int speakers, float balance)
{
	return !mono_output && speakers == OBS_SOURCE_AUDIO_INPUT_SPEAKERS_STEREO &&
	       (balance > 0.51f || balance < 0.49f);
}

bool obs_source_audio_input_mono_needed(bool mono_output, uint32_t flags)
{
	return !mono_output && (flags & OBS_SOURCE_AUDIO_INPUT_FORCE_MONO) != 0;
}

void obs_source_audio_input_balance(float **data, uint32_t frames, float balance, int law)
{
	switch (law) {
	case OBS_SOURCE_AUDIO_INPUT_SINE_LAW:
		for (uint32_t frame = 0; frame < frames; frame++) {
			data[0][frame] = data[0][frame] * sinf((float)((double)(1.0f - balance) * (M_PI / 2.0)));
			data[1][frame] = data[1][frame] * sinf((float)((double)balance * (M_PI / 2.0)));
		}
		break;
	case OBS_SOURCE_AUDIO_INPUT_SQUARE_LAW:
		for (uint32_t frame = 0; frame < frames; frame++) {
			data[0][frame] = data[0][frame] * sqrtf(1.0f - balance);
			data[1][frame] = data[1][frame] * sqrtf(balance);
		}
		break;
	case OBS_SOURCE_AUDIO_INPUT_LINEAR:
		for (uint32_t frame = 0; frame < frames; frame++) {
			data[0][frame] = data[0][frame] * (1.0f - balance);
			data[1][frame] = data[1][frame] * balance;
		}
		break;
	default:
		break;
	}
}

/* TODO: SSE optimization */
void obs_source_audio_input_downmix(float **data, size_t channels, uint32_t frames)
{
	const float channels_i = 1.0f / (float)channels;

	for (size_t channel = 1; channel < channels; channel++) {
		for (uint32_t frame = 0; frame < frames; frame++)
			data[0][frame] += data[channel][frame];
	}

	for (uint32_t frame = 0; frame < frames; frame++)
		data[0][frame] *= channels_i;

	for (size_t channel = 1; channel < channels; channel++) {
		for (uint32_t frame = 0; frame < frames; frame++)
			data[channel][frame] = data[0][frame];
	}
}
