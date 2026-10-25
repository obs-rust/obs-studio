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

#include "libobs/build/libobs-ported-audio-discard.pch.h"

/* The steps of the audio tick that drop audio from a source's input buffer,
 * on the source's fields by pointer, declared in obs-audio-discard.h.
 * obs-audio.c holds the source's audio_buf_mutex, passes the fields in and
 * logs the steps. Ported to rust/obs-core (audio_discard): the hybrid
 * libobs drops this file. */

static inline void obs_audio_discard_pop(const struct obs_audio_input *in, size_t channels, size_t size)
{
	for (size_t ch = 0; ch < channels; ch++)
		deque_pop_front(&in->buf[ch], NULL, size);
}

struct obs_audio_ignore_step obs_audio_discard_ignore(const struct obs_audio_input *in, size_t channels,
						      size_t sample_rate, uint64_t start_ts)
{
	struct obs_audio_ignore_step step = {0};
	size_t num_floats = in->buf[0].size / sizeof(float);

	step.buffered = num_floats;

	if (!*in->audio_ts && num_floats) {
		step.no_timestamp = true;
		/* channel 0's size, read again for each channel */
		for (size_t ch = 0; ch < channels; ch++)
			deque_pop_front(&in->buf[ch], NULL, in->buf[0].size);
		*in->last_size = 0;
		return step;
	}

	if (num_floats) {
		/* round up the number of samples to drop */
		size_t drop = (size_t)util_mul_div64(start_ts - *in->audio_ts - 1, sample_rate, 1000000000ULL) + 1;
		if (drop > num_floats)
			drop = num_floats;

		step.dropped = drop;
		obs_audio_discard_pop(in, channels, drop * sizeof(float));

		*in->last_size = 0;
		*in->audio_ts += util_mul_div64(drop, 1000000000ULL, sample_rate);
		step.lag = start_ts - *in->audio_ts;

		/* rounding error, adjust */
		if (*in->audio_ts == (start_ts - 1))
			*in->audio_ts = start_ts;

		/* source is back in sync */
		if (*in->audio_ts >= start_ts) {
			step.rerender = true;
			return step;
		}
	}

	if (!*in->pending || num_floats) {
		step.warn = true;
		step.warn_lag = start_ts - *in->audio_ts;
	}

	*in->pending = true;
	*in->audio_ts = 0;
	/* tell the timestamp adjustment code in source_output_audio_data to
	 * reset everything, and hopefully fix the timestamps */
	*in->timing_set = false;
	return step;
}

enum obs_audio_stop_check obs_audio_discard_if_stopped(const struct obs_audio_input *in, size_t channels)
{
	size_t last_size = *in->last_size;
	size_t size = in->buf[0].size;

	if (!size)
		return OBS_AUDIO_STOP_NONE;

	/* if perpetually pending data, it means the audio has stopped,
	 * so clear the audio data */
	if (last_size == size) {
		if (!*in->pending_stop) {
			*in->pending_stop = true;
			return OBS_AUDIO_STOP_PENDING;
		}

		for (size_t ch = 0; ch < channels; ch++)
			deque_pop_front(&in->buf[ch], NULL, in->buf[ch].size);

		*in->pending_stop = false;
		*in->audio_ts = 0;
		*in->last_size = 0;
		return OBS_AUDIO_STOP_CLEARED;
	} else {
		*in->last_size = size;
		return OBS_AUDIO_STOP_NONE;
	}
}

#define OBS_AUDIO_DISCARD_MAX_SIZE (AUDIO_OUTPUT_FRAMES * sizeof(float))

struct obs_audio_discard_step obs_audio_discard_tick(const struct obs_audio_input *in, bool audio_render,
						     size_t channels, size_t sample_rate, const struct ts_info *ts)
{
	struct obs_audio_discard_step step = {OBS_AUDIO_DISCARD_DONE, OBS_AUDIO_STOP_NONE};
	size_t total_floats = AUDIO_OUTPUT_FRAMES;
	size_t size;

	if (audio_render) {
		*in->audio_ts = 0;
		step.path = OBS_AUDIO_DISCARD_RENDER_SOURCE;
		return step;
	}

	if (ts->end <= *in->audio_ts) {
		step.path = OBS_AUDIO_DISCARD_AHEAD;
		return step;
	}

	if (*in->audio_ts < (ts->start - 1)) {
		step.path = OBS_AUDIO_DISCARD_BEHIND;
		if (*in->pending && in->buf[0].size < OBS_AUDIO_DISCARD_MAX_SIZE)
			step.stop = obs_audio_discard_if_stopped(in, channels);
		return step;
	}

	if (*in->audio_ts != ts->start && *in->audio_ts != (ts->start - 1)) {
		size_t start_point = (size_t)util_mul_div64(*in->audio_ts - ts->start, sample_rate, 1000000000ULL);
		if (start_point == AUDIO_OUTPUT_FRAMES) {
			step.path = OBS_AUDIO_DISCARD_START_AT_END;
			return step;
		}

		total_floats -= start_point;
	}

	size = total_floats * sizeof(float);

	if (in->buf[0].size < size) {
		step.path = OBS_AUDIO_DISCARD_PENDING;
		step.stop = obs_audio_discard_if_stopped(in, channels);
		if (step.stop != OBS_AUDIO_STOP_CLEARED)
			*in->audio_ts = ts->end;
		return step;
	}

	obs_audio_discard_pop(in, channels, size);

	*in->last_size = 0;
	*in->pending_stop = false;
	*in->audio_ts = ts->end;
	return step;
}

#undef OBS_AUDIO_DISCARD_MAX_SIZE
