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

#include "libobs/build/libobs-ported-audio-min-ts.pch.h"

/* The per-source decisions of the minimum audio timestamp step, on plain
 * values, declared in obs-audio-min-ts.h. obs-audio.c walks the sources,
 * holds each one's audio_buf_mutex and passes the fields in. Ported to
 * rust/obs-core (audio_min_ts): the hybrid libobs drops this file. */

bool obs_audio_min_ts_earlier(bool pending, uint64_t audio_ts, uint64_t *min_ts)
{
	if (!pending && audio_ts && audio_ts < *min_ts) {
		*min_ts = audio_ts;
		return true;
	}
	return false;
}

bool obs_audio_min_ts_insufficient(bool audio_render, bool *pending, uint64_t audio_ts, size_t buffered,
				   size_t sample_rate, uint64_t min_ts)
{
	size_t total_floats = AUDIO_OUTPUT_FRAMES;
	size_t size;

	if (audio_render || *pending || !audio_ts) {
		return false;
	}

	if (audio_ts != min_ts && audio_ts != (min_ts - 1)) {
		size_t start_point = (size_t)util_mul_div64(audio_ts - min_ts, sample_rate, 1000000000ULL);
		if (start_point >= AUDIO_OUTPUT_FRAMES)
			return false;

		total_floats -= start_point;
	}

	size = total_floats * sizeof(float);

	if (buffered < size) {
		*pending = true;
		return true;
	}

	return false;
}

enum obs_audio_buffering_choice obs_audio_min_ts_buffering(bool fixed_buffer, bool maxed, uint64_t min_ts,
							   uint64_t tick_start)
{
	if (fixed_buffer)
		return maxed ? OBS_AUDIO_BUFFERING_NONE : OBS_AUDIO_BUFFERING_FIXED;
	return min_ts < tick_start ? OBS_AUDIO_BUFFERING_ADD : OBS_AUDIO_BUFFERING_NONE;
}
