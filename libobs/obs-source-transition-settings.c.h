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

#include "libobs/build/libobs.pch.h"

#define lock_transition(transition) pthread_mutex_lock(&transition->transition_mutex);
#define unlock_transition(transition) pthread_mutex_unlock(&transition->transition_mutex);

bool obs_transition_is_active(obs_source_t *transition)
{
	return transition_active(transition);
}

void obs_transition_set_scale_type(obs_source_t *transition, enum obs_transition_scale_type type)
{
	if (!transition_valid(transition, "obs_transition_set_scale_type"))
		return;

	transition->transition_scale_type = valid_scale_type((int64_t)type);
}

enum obs_transition_scale_type obs_transition_get_scale_type(const obs_source_t *transition)
{
	return transition_valid(transition, "obs_transition_get_scale_type") ? transition->transition_scale_type
									     : OBS_TRANSITION_SCALE_MAX_ONLY;
}

void obs_transition_set_alignment(obs_source_t *transition, uint32_t alignment)
{
	if (!transition_valid(transition, "obs_transition_set_alignment"))
		return;

	transition->transition_alignment = alignment;
}

uint32_t obs_transition_get_alignment(const obs_source_t *transition)
{
	return transition_valid(transition, "obs_transition_get_alignment") ? transition->transition_alignment : 0;
}

void obs_transition_set_size(obs_source_t *transition, uint32_t cx, uint32_t cy)
{
	if (!transition_valid(transition, "obs_transition_set_size"))
		return;

	transition->transition_cx = cx;
	transition->transition_cy = cy;
}

void obs_transition_get_size(const obs_source_t *transition, uint32_t *cx, uint32_t *cy)
{
	if (!transition_valid(transition, "obs_transition_set_size")) {
		*cx = 0;
		*cy = 0;
		return;
	}

	*cx = transition->transition_cx;
	*cy = transition->transition_cy;
}

void obs_transition_save(obs_source_t *tr, obs_data_t *data)
{
	obs_source_t *child;

	lock_transition(tr);
	child = tr->transition_sources[obs_source_transition_start_active_index(transition_active(tr))];

	obs_data_set_string(data, "transition_source_a", child ? child->context.name : "");
	obs_data_set_int(data, "transition_alignment", tr->transition_alignment);
	obs_data_set_int(data, "transition_mode", (int64_t)tr->transition_mode);
	obs_data_set_int(data, "transition_scale_type", (int64_t)tr->transition_scale_type);
	obs_data_set_int(data, "transition_cx", tr->transition_cx);
	obs_data_set_int(data, "transition_cy", tr->transition_cy);
	unlock_transition(tr);
}

void obs_transition_enable_fixed(obs_source_t *transition, bool enable, uint32_t duration)
{
	if (!transition_valid(transition, "obs_transition_enable_fixed"))
		return;

	transition->transition_use_fixed_duration = enable;
	transition->transition_fixed_duration = duration;
}

bool obs_transition_fixed(obs_source_t *transition)
{
	return transition_valid(transition, "obs_transition_fixed") ? transition->transition_use_fixed_duration : false;
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef lock_transition
#undef unlock_transition
