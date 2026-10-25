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

/* The per-source decisions of the "get minimum audio timestamp" step of the
 * audio tick (obs-audio-min-ts.c), on plain values. The walk over the audio
 * sources, the locks, the two passes of calc_min_ts, the names and the
 * logging stay in obs-audio.c, which calls these for each source under its
 * audio_buf_mutex. Nothing here takes a lock, logs or calls into libobs.
 *
 * obs_audio_min_ts_earlier() is find_min_ts's test for one source: a source
 * that is not pending, has a timestamp and is strictly earlier than
 * *min_ts becomes the minimum (true).
 *
 * obs_audio_min_ts_insufficient() is audio_buffer_insufficient: whether a
 * source's input (buffered bytes in channel 0) is short for a tick that
 * starts at min_ts. Render sources, pending sources and sources without a
 * timestamp are not; neither is a source whose start point is at or past
 * AUDIO_OUTPUT_FRAMES. A short source is marked pending (*pending).
 *
 * obs_audio_min_ts_buffering() is the choice after the search: fixed
 * buffering when fixed_buffer is set and the buffering is not at the
 * maximum, added buffering when min_ts is before the tick start, or none.
 *
 * Hidden in libobs: not EXPORT. */

enum obs_audio_buffering_choice {
	OBS_AUDIO_BUFFERING_NONE,
	OBS_AUDIO_BUFFERING_FIXED,
	OBS_AUDIO_BUFFERING_ADD,
};

bool obs_audio_min_ts_earlier(bool pending, uint64_t audio_ts, uint64_t *min_ts);
bool obs_audio_min_ts_insufficient(bool audio_render, bool *pending, uint64_t audio_ts, size_t buffered,
				   size_t sample_rate, uint64_t min_ts);
enum obs_audio_buffering_choice obs_audio_min_ts_buffering(bool fixed_buffer, bool maxed, uint64_t min_ts,
							   uint64_t tick_start);
