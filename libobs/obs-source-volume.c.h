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

#include "libobs/build/libobs-ported-source-volume.pch.h"

/* The volume decisions of obs-source.c, moved from get_source_volume() (the
 * 0/1 snap) and apply_audio_volume() (whether the first queued action is
 * due, and skip, clear or multiply over the mix masks): the decisions, on
 * plain values. obs-source.c keeps every field read and write in the same
 * order and passes the values it read. Declared in obs-source-volume.h
 * without EXPORT. In hybrid libobs the Rust port (rust/obs-source,
 * volume_apply) replaces this file. */

float obs_source_volume_snap(bool muted, float volume)
{
	if (muted || close_float(volume, 0.0f, 0.0001f))
		return 0.0f;
	if (close_float(volume, 1.0f, 0.0001f))
		return 1.0f;

	return volume;
}

bool obs_source_volume_action_due(uint64_t action_ts, uint64_t audio_ts, uint64_t duration)
{
	return action_ts < (audio_ts + duration);
}

enum obs_source_volume_op obs_source_volume_step(float vol, uint32_t mixers)
{
	if (vol == 1.0f)
		return OBS_SOURCE_VOLUME_SKIP;

	if (vol == 0.0f || mixers == 0)
		return OBS_SOURCE_VOLUME_CLEAR;

	return OBS_SOURCE_VOLUME_MULTIPLY;
}

bool obs_source_volume_mixes(uint32_t audio_mixers, uint32_t mixers, size_t mix)
{
	uint32_t mix_and_val = (uint32_t)(1 << mix);
	return (audio_mixers & mix_and_val) != 0 && (mixers & mix_and_val) != 0;
}
