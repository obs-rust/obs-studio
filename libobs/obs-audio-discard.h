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

#include "obs-audio-mix.h"
#include "util/deque.h"

/* The steps of the audio tick that drop audio from a source's input buffer
 * (obs-audio-discard.c), on the source's fields by pointer. The source side
 * (the audio_buf_mutex, which the caller holds, the name and the logging)
 * stays in obs-audio.c, which logs from the step the functions return.
 * Nothing here takes a lock, logs or calls into libobs.
 *
 * struct obs_audio_input points to the fields of struct obs_source the steps
 * read and change (timing_set is volatile there). buf is audio_input_buf
 * (`channels` deques). The sample
 * count is always channel 0's, and every channel pops that size.
 *
 * obs_audio_discard_ignore() is ignore_audio: a source behind start_ts at
 * maximum buffering. Without a timestamp, channel 0's buffered audio is
 * popped from each channel, with channel 0's size read again for each one:
 * once channel 0 is empty, the other channels pop nothing. With
 * audio, the samples behind start_ts are popped (rounded up, cut at what is
 * buffered) and audio_ts moves on (to start_ts when it lands on
 * start_ts - 1); when that reaches start_ts, rerender is set. Otherwise the
 * source's audio restarts: pending is set, audio_ts and timing_set are
 * cleared, and warn says whether obs-audio.c prints the lag warning (not
 * when it was already pending with nothing buffered). audio_ts must be below
 * start_ts.
 *
 * obs_audio_discard_if_stopped() is the "pending stop" check: when the
 * buffered size equals last_size twice in a row, the buffer is cleared and
 * audio_ts reset.
 *
 * obs_audio_discard_tick() is discard_audio: after the mix, pop the frames of
 * the tick window ts from the buffer and set audio_ts to ts->end. The step
 * says which path it took.
 *
 * Hidden in libobs: not EXPORT. */

struct obs_audio_input {
	struct deque *buf;
	uint64_t *audio_ts;
	size_t *last_size;
	bool *pending;
	bool *pending_stop;
	volatile bool *timing_set;
};

struct obs_audio_ignore_step {
	/* the source is back in sync: render it again */
	bool rerender;
	/* audio without a timestamp: all of it was popped */
	bool no_timestamp;
	/* channel 0's sample count before, and the samples popped */
	size_t buffered;
	size_t dropped;
	/* with dropped samples: start_ts - audio_ts after the drop */
	uint64_t lag;
	/* the source's audio restarts and obs-audio.c prints the warning, with
	 * warn_lag = start_ts - audio_ts before the restart */
	bool warn;
	uint64_t warn_lag;
};

enum obs_audio_stop_check {
	OBS_AUDIO_STOP_NONE,
	/* first unchanged tick: pending_stop is now set */
	OBS_AUDIO_STOP_PENDING,
	/* second unchanged tick: the buffer was cleared */
	OBS_AUDIO_STOP_CLEARED,
};

enum obs_audio_discard_path {
	/* a source with audio_render: audio_ts reset, nothing popped */
	OBS_AUDIO_DISCARD_RENDER_SOURCE,
	/* audio_ts at or after ts->end */
	OBS_AUDIO_DISCARD_AHEAD,
	/* audio_ts before ts->start - 1 */
	OBS_AUDIO_DISCARD_BEHIND,
	/* the start point is AUDIO_OUTPUT_FRAMES */
	OBS_AUDIO_DISCARD_START_AT_END,
	/* not enough audio for the tick: audio_ts set to ts->end, unless the
	 * stop check cleared the buffer */
	OBS_AUDIO_DISCARD_PENDING,
	/* the tick's frames were popped and audio_ts set to ts->end */
	OBS_AUDIO_DISCARD_DONE,
};

struct obs_audio_discard_step {
	enum obs_audio_discard_path path;
	enum obs_audio_stop_check stop;
};

struct obs_audio_ignore_step obs_audio_discard_ignore(const struct obs_audio_input *in, size_t channels,
						      size_t sample_rate, uint64_t start_ts);
enum obs_audio_stop_check obs_audio_discard_if_stopped(const struct obs_audio_input *in, size_t channels);
struct obs_audio_discard_step obs_audio_discard_tick(const struct obs_audio_input *in, bool audio_render,
						     size_t channels, size_t sample_rate, const struct ts_info *ts);
