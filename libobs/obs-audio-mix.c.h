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

#include "libobs/build/libobs-ported-audio-mix.pch.h"

/* The arithmetic of the audio tick on plain values, declared in
 * obs-audio-mix.h. obs-audio.c passes the source's and the core's fields in
 * and logs the steps. Ported to rust/obs-core (audio_mix): the hybrid libobs
 * drops this file. */

size_t obs_audio_mix_time_to_frames(size_t sample_rate, uint64_t t)
{
	return (size_t)util_mul_div64(t, sample_rate, 1000000000ULL);
}

void obs_audio_mix_add(struct audio_output_data *mixes, float *(*buf)[MAX_AUDIO_CHANNELS], uint64_t audio_ts,
		       const struct ts_info *ts, size_t channels, size_t sample_rate)
{
	size_t total_floats = AUDIO_OUTPUT_FRAMES;
	size_t start_point = 0;

	if (audio_ts < ts->start || ts->end <= audio_ts)
		return;

	if (audio_ts != ts->start) {
		start_point = obs_audio_mix_time_to_frames(sample_rate, audio_ts - ts->start);
		if (start_point == AUDIO_OUTPUT_FRAMES)
			return;

		total_floats -= start_point;
	}

	for (size_t mix_idx = 0; mix_idx < MAX_AUDIO_MIXES; mix_idx++) {
		for (size_t ch = 0; ch < channels; ch++) {
			register float *mix = mixes[mix_idx].data[ch];
			register float *aud = buf[mix_idx][ch];
			register float *end;

			mix += start_point;
			end = aud + total_floats;

			while (aud < end)
				*(mix++) += *(aud++);
		}
	}
}

bool obs_audio_buffering_maxed(const struct obs_audio_buffering *buffering)
{
	return *buffering->total_buffering_ticks == buffering->max_buffering_ticks;
}

/* Pushes `ticks` windows before the waiting ones and sets *ts to the
 * earliest. With no tick, ts->start still becomes the start of the waiting
 * ticks and ts->end stays. */
static void push_buffering_ticks(const struct obs_audio_buffering *buffering, size_t sample_rate, struct ts_info *ts,
				 int ticks)
{
	struct ts_info new_ts = *ts;

	new_ts.start = *buffering->buffered_ts -
		       audio_frames_to_ns(sample_rate, *buffering->buffering_wait_ticks * AUDIO_OUTPUT_FRAMES);

	while (ticks--) {
		const uint64_t cur_ticks = ++*buffering->buffering_wait_ticks;

		new_ts.end = new_ts.start;
		new_ts.start =
			*buffering->buffered_ts - audio_frames_to_ns(sample_rate, cur_ticks * AUDIO_OUTPUT_FRAMES);

		deque_push_front(buffering->buffered_timestamps, &new_ts, sizeof(new_ts));
	}

	*ts = new_ts;
}

struct obs_audio_buffering_step obs_audio_buffering_fixed(const struct obs_audio_buffering *buffering,
							  size_t sample_rate, struct ts_info *ts)
{
	struct obs_audio_buffering_step step = {0};

	if (obs_audio_buffering_maxed(buffering))
		return step;

	if (!*buffering->buffering_wait_ticks)
		*buffering->buffered_ts = ts->start;

	step.added = true;
	step.ticks = buffering->max_buffering_ticks - *buffering->total_buffering_ticks;
	*buffering->total_buffering_ticks += step.ticks;

	step.ms = (size_t)(step.ticks * AUDIO_OUTPUT_FRAMES * 1000) / sample_rate;
	step.total_ms = (size_t)(*buffering->total_buffering_ticks * AUDIO_OUTPUT_FRAMES * 1000) / sample_rate;

	push_buffering_ticks(buffering, sample_rate, ts, step.ticks);
	return step;
}

struct obs_audio_buffering_step obs_audio_buffering_add(const struct obs_audio_buffering *buffering, size_t sample_rate,
							struct ts_info *ts, uint64_t min_ts)
{
	struct obs_audio_buffering_step step = {0};
	uint64_t offset;
	uint64_t frames;

	if (obs_audio_buffering_maxed(buffering))
		return step;

	if (!*buffering->buffering_wait_ticks)
		*buffering->buffered_ts = ts->start;

	offset = ts->start - min_ts;
	frames = ns_to_audio_frames(sample_rate, offset);

	step.added = true;
	step.ticks = (int)((frames + AUDIO_OUTPUT_FRAMES - 1) / AUDIO_OUTPUT_FRAMES);

	*buffering->total_buffering_ticks += step.ticks;

	if (*buffering->total_buffering_ticks >= buffering->max_buffering_ticks) {
		step.ticks -= *buffering->total_buffering_ticks - buffering->max_buffering_ticks;
		*buffering->total_buffering_ticks = buffering->max_buffering_ticks;
		step.max_reached = true;
	}

	step.ms = (size_t)(step.ticks * AUDIO_OUTPUT_FRAMES * 1000) / sample_rate;
	step.total_ms = (size_t)(*buffering->total_buffering_ticks * AUDIO_OUTPUT_FRAMES * 1000) / sample_rate;

	push_buffering_ticks(buffering, sample_rate, ts, step.ticks);
	return step;
}
