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

#include "libobs/build/libobs-ported-source-transition-load.pch.h"

/* The load, scale type and swap checks of the transitions, moved from
 * obs-source-transition.c: the type check of transition_valid(), the scale
 * type rule of obs_transition_set_scale_type() and obs_transition_load(),
 * the source steps of obs_transition_load(), the recursion refusal of
 * copy_source_state() and the same-transition gate of
 * obs_transition_swap_begin() and obs_transition_swap_end(), on plain
 * values. obs-source-transition.c keeps obs_ptr_valid(), every read of the
 * transition's fields at its point and in its lock section, the locks, the
 * pointer comparisons, the lookup, the active child calls, the references
 * and the logs. Declared in obs-source-transition-load.h without EXPORT. In
 * hybrid libobs the Rust port (rust/obs-source, transition_load) replaces
 * this file. */

bool obs_source_transition_load_is_transition(int type)
{
	return type == OBS_SOURCE_TRANSITION_LOAD_TYPE_TRANSITION;
}

/* A scale type outside the enum (from a scene collection or a caller) is the
 * default, Max Only. */
int obs_source_transition_load_scale_type(int64_t type)
{
	switch (type) {
	case OBS_SOURCE_TRANSITION_LOAD_SCALE_MAX_ONLY:
	case OBS_SOURCE_TRANSITION_LOAD_SCALE_ASPECT:
	case OBS_SOURCE_TRANSITION_LOAD_SCALE_STRETCH:
		return (int)type;
	default:
		return OBS_SOURCE_TRANSITION_LOAD_SCALE_MAX_ONLY;
	}
}

bool obs_source_transition_load_looks_up(bool has_name)
{
	return has_name;
}

enum obs_source_transition_load_outcome obs_source_transition_load_source(bool found, bool add_ok)
{
	if (!found)
		return OBS_SOURCE_TRANSITION_LOAD_NOT_FOUND;
	if (!add_ok)
		return OBS_SOURCE_TRANSITION_LOAD_REFUSE;
	return OBS_SOURCE_TRANSITION_LOAD_KEEP;
}

bool obs_source_transition_load_swaps(bool same)
{
	return !same;
}

bool obs_source_transition_load_copy_stores(bool adds, bool add_ok)
{
	return !adds || add_ok;
}
