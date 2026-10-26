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

#include "libobs/build/libobs-ported-source-transition-start.pch.h"

/* The start checks and child handover of the transitions, moved from
 * obs-source-transition.c: the admission steps of obs_transition_start(),
 * the clock rule and handover of set_source(), activate_transition(), the
 * active child of obs_transition_get_active_source() and
 * obs_transition_save(), the reset of obs_transition_set_manual_time() and
 * the active child rule of obs_transition_set() and copy_source_state(), on
 * plain values. obs-source-transition.c keeps every read of the
 * transition's fields at its point and in its lock section, the locks, the
 * pointer comparisons, the active child calls, the callbacks, the signals
 * and the logs. Declared in obs-source-transition-start.h without EXPORT. In
 * hybrid libobs the Rust port (rust/obs-source, transition_start) replaces
 * this file. */

bool obs_source_transition_start_refused(bool same_as_source, bool active)
{
	return same_as_source && !active;
}

bool obs_source_transition_start_walks(bool has_dest, bool recursive)
{
	return has_dest && !recursive;
}

enum obs_source_transition_start_admission obs_source_transition_start_admit(bool same_as_source, bool same_as_dest,
									     bool same_mode, int mode, bool active)
{
	if (same_as_source && !active)
		return OBS_SOURCE_TRANSITION_START_REFUSE;
	if (active && mode == OBS_SOURCE_TRANSITION_START_MODE_MANUAL && same_mode && same_as_dest)
		return OBS_SOURCE_TRANSITION_START_DONE;
	return OBS_SOURCE_TRANSITION_START_GO;
}

uint64_t obs_source_transition_start_duration_ns(uint32_t ms)
{
	return (uint64_t)ms * 1000000ULL;
}

bool obs_source_transition_start_restart(bool active, bool same_as_dest, bool same_as_source)
{
	return !active || (!same_as_dest && !same_as_source);
}

bool obs_source_transition_start_adds(bool already_active, bool has_new)
{
	return already_active && has_new;
}

bool obs_source_transition_start_removes(bool already_active, bool has_old, bool add_ok)
{
	return already_active && has_old && add_ok;
}

bool obs_source_transition_start_calls_back(bool has_callback, bool add_ok)
{
	return has_callback && add_ok;
}

bool obs_source_transition_start_unsized(uint32_t size)
{
	return size == 0;
}

bool obs_source_transition_start_null_dest(bool has_dest, bool same_as_dest, bool same_as_source)
{
	return !has_dest && same_as_dest && !same_as_source;
}

bool obs_source_transition_start_activates(bool slot_active)
{
	return !slot_active;
}

size_t obs_source_transition_start_active_index(bool active)
{
	return active ? 1 : 0;
}

bool obs_source_transition_start_manual_reset(int mode, float t)
{
	return mode == OBS_SOURCE_TRANSITION_START_MODE_MANUAL && t == 0.0f;
}

bool obs_source_transition_start_child_active(bool has_child, bool active)
{
	return has_child && active;
}
