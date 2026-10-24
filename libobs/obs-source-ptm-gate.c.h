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

#include "libobs/build/libobs-ported-source-ptm-gate.pch.h"

/* The push-to-mute and push-to-talk gate of obs-source.c, moved from
 * source_muted(), with push_to_stop_time(), which get_source_volume() uses
 * too: the decisions, on plain values. source_muted() keeps every field
 * read and write in the same order and passes the values it read. Declared
 * in obs-source-ptm-gate.h without EXPORT. In hybrid libobs the Rust port
 * (rust/obs-source) replaces this file. */

uint64_t obs_source_ptm_gate_stop_time(uint64_t os_time, uint64_t delay_ms)
{
	if (delay_ms > UINT64_MAX / OBS_SOURCE_PTM_GATE_NS_PER_MS)
		return UINT64_MAX;

	uint64_t delay_ns = delay_ms * OBS_SOURCE_PTM_GATE_NS_PER_MS;
	return delay_ns > UINT64_MAX - os_time ? UINT64_MAX : os_time + delay_ns;
}

bool obs_source_ptm_gate_held(uint64_t os_time, uint64_t stop_time)
{
	return os_time < stop_time;
}

bool obs_source_ptm_gate_mutes(bool enabled, bool active)
{
	return enabled && active;
}

bool obs_source_ptm_gate_talk_mutes(bool enabled, bool active)
{
	return enabled && !active;
}
