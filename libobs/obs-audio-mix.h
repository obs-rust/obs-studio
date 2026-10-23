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

#include "media-io/audio-io.h"
#include "util/deque.h"

/* The arithmetic of the audio tick (obs-audio-mix.c), on plain values. The
 * source and core side (locks, the source fields, logging) stays in
 * obs-audio.c, which passes the values in. Nothing here takes a lock, logs or
 * calls into libobs.
 *
 * obs_audio_mix_time_to_frames() is floor(t * sample_rate / 1e9) through
 * util_mul_div64.
 *
 * obs_audio_mix_add() adds a root source's audio (buf, the source's
 * audio_output_buf, AUDIO_OUTPUT_FRAMES floats per mix and channel) to the
 * first `channels` channels of all MAX_AUDIO_MIXES mixes. The audio starts
 * at audio_ts. Audio outside [ts->start, ts->end) adds nothing; otherwise it
 * goes from frame obs_audio_mix_time_to_frames(audio_ts - ts->start) on,
 * and a start on frame AUDIO_OUTPUT_FRAMES adds nothing.
 *
 * obs_audio_buffering is the buffering state of struct obs_core_audio, by
 * pointer. obs_audio_buffering_fixed() adds the ticks up to the maximum
 * (fixed buffering), obs_audio_buffering_add() the ticks a source min_ts
 * behind ts->start needs (rounded up, cut at the maximum). Both do nothing
 * when the total is at the maximum. Otherwise they start buffered_ts at
 * ts->start when no tick is waiting, push one window per added tick to the
 * front of buffered_timestamps (contiguous, each one tick before the last),
 * and set *ts to the earliest one. The step they return gives the values
 * that obs-audio.c logs.
 *
 * Hidden in libobs: not EXPORT. */

struct ts_info {
	uint64_t start;
	uint64_t end;
};

struct obs_audio_buffering {
	uint64_t *buffered_ts;
	struct deque *buffered_timestamps;
	uint64_t *buffering_wait_ticks;
	int *total_buffering_ticks;
	int max_buffering_ticks;
};

struct obs_audio_buffering_step {
	/* false: the total was at the maximum and nothing changed */
	bool added;
	/* obs_audio_buffering_add() only: the ticks were cut at the maximum */
	bool max_reached;
	int ticks;
	size_t ms;
	size_t total_ms;
};

size_t obs_audio_mix_time_to_frames(size_t sample_rate, uint64_t t);
void obs_audio_mix_add(struct audio_output_data *mixes, float *(*buf)[MAX_AUDIO_CHANNELS], uint64_t audio_ts,
		       const struct ts_info *ts, size_t channels, size_t sample_rate);

bool obs_audio_buffering_maxed(const struct obs_audio_buffering *buffering);
struct obs_audio_buffering_step obs_audio_buffering_fixed(const struct obs_audio_buffering *buffering,
							  size_t sample_rate, struct ts_info *ts);
struct obs_audio_buffering_step obs_audio_buffering_add(const struct obs_audio_buffering *buffering, size_t sample_rate,
							struct ts_info *ts, uint64_t min_ts);
