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

#include "libobs/build/libobs-ported-source-audio-render.pch.h"

/* The audio render mix plan of obs-source.c, moved from
 * obs_source_audio_render() (the route), custom_audio_render() (the clears
 * around the callback and its result) and process_audio_source_tick() (the
 * pending check, the copy or clear of each mix): the decisions, on plain
 * values. obs-source.c keeps every field read and write in the same order
 * and passes the values it read. Declared in obs-source-audio-render.h
 * without EXPORT. In hybrid libobs the Rust port (rust/obs-source,
 * audio_render_plan) replaces this file. */

enum obs_source_audio_render_route obs_source_audio_render_route(bool has_buffer, bool custom, bool has_data)
{
	if (!has_buffer)
		return OBS_SOURCE_AUDIO_RENDER_PENDING;

	if (custom) {
		if (!has_data)
			return OBS_SOURCE_AUDIO_RENDER_PENDING;
		return OBS_SOURCE_AUDIO_RENDER_CUSTOM;
	}

	return OBS_SOURCE_AUDIO_RENDER_TICK;
}

bool obs_source_audio_render_custom_clear_before(uint32_t audio_mixers, uint32_t mixers, size_t mix)
{
	return (audio_mixers & mixers & (1 << mix)) != 0;
}

uint64_t obs_source_audio_render_custom_ts(bool success, uint64_t ts)
{
	return success ? ts : 0;
}

bool obs_source_audio_render_custom_pending(bool success)
{
	return !success;
}

bool obs_source_audio_render_custom_unmixed(uint64_t audio_ts, uint32_t mixers)
{
	return !audio_ts || !mixers;
}

bool obs_source_audio_render_custom_in_tick(uint32_t mixers, size_t mix)
{
	uint32_t mix_bit = 1 << mix;

	return (mixers & mix_bit) != 0;
}

bool obs_source_audio_render_custom_clear_after(uint32_t audio_mixers, size_t mix)
{
	uint32_t mix_bit = 1 << mix;

	return (audio_mixers & mix_bit) == 0;
}

bool obs_source_audio_render_tick_unset(uint64_t audio_ts)
{
	return !audio_ts;
}

bool obs_source_audio_render_tick_short(size_t input_size, size_t size)
{
	return input_size < size;
}

bool obs_source_audio_render_tick_stop(bool submix, size_t mix)
{
	return submix && mix > 1;
}

bool obs_source_audio_render_tick_copy(bool submix, uint32_t audio_mixers, uint32_t mixers, size_t mix)
{
	uint32_t mix_and_val;

	/* A submix source fills mix 1 from bit 0 of its mixers. */
	if (submix) {
		mixers = 1;
		mix_and_val = 1;
	} else {
		mix_and_val = (1 << mix);
	}

	return !((audio_mixers & mix_and_val) == 0 || (mixers & mix_and_val) == 0);
}

bool obs_source_audio_render_tick_volume(bool submix)
{
	return !submix;
}

bool obs_source_audio_render_tick_clear_main(uint32_t audio_mixers, uint32_t mixers)
{
	return (audio_mixers & 1) == 0 || (mixers & 1) == 0;
}
