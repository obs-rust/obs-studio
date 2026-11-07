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

#include "libobs/build/libobs-ported-source-transition-access.pch.h"

#define lock_transition(transition) pthread_mutex_lock(&transition->transition_mutex);
#define unlock_transition(transition) pthread_mutex_unlock(&transition->transition_mutex);

obs_source_t *obs_transition_get_source(obs_source_t *transition, enum obs_transition_target target)
{
	size_t idx = (size_t)target;
	obs_source_t *ret;

	if (!transition_valid(transition, "obs_transition_get_source"))
		return NULL;

	lock_transition(transition);
	ret = transition->transition_sources[idx];
	ret = obs_source_get_ref(ret);
	unlock_transition(transition);

	return ret;
}

obs_source_t *obs_transition_get_active_source(obs_source_t *transition)
{
	obs_source_t *ret;
	bool active;

	if (!transition_valid(transition, "obs_transition_get_source"))
		return NULL;

	lock_transition(transition);
	active = os_atomic_load_bool(&transition->transitioning_audio) ||
		 os_atomic_load_bool(&transition->transitioning_video);
	ret = transition->transition_sources[obs_source_transition_start_active_index(active)];
	ret = obs_source_get_ref(ret);
	unlock_transition(transition);

	return ret;
}

void obs_transition_set_manual_torque(obs_source_t *transition, float torque, float clamp)
{
	lock_transition(transition);
	transition->transition_manual_torque = torque;
	transition->transition_manual_clamp = clamp;
	unlock_transition(transition);
}

void obs_transition_set_manual_time(obs_source_t *transition, float t)
{
	enum obs_transition_mode mode;

	lock_transition(transition);
	transition->transition_manual_target = t;
	mode = transition->transition_mode;
	unlock_transition(transition);

	if (obs_source_transition_start_manual_reset((int)mode, t)) {
		obs_transition_set(transition, transition->transition_sources[0]);
	}
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef lock_transition
#undef unlock_transition
