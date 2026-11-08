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

float transition_calc_time(obs_source_t *transition, uint64_t ts)
{
	return obs_source_transition_clock_time((int)transition->transition_mode, transition->transition_manual_val,
						transition->transition_start_time, transition->transition_duration, ts);
}

float transition_get_video_time(obs_source_t *transition)
{
	uint64_t ts = obs_video_stats_frame_time(&obs->video.stats);
	return transition_calc_time(transition, ts);
}

float obs_transition_get_time(obs_source_t *transition)
{
	/* Read under the lock that publishes a start's clock, as the render
	 * functions do: a caller choosing what to draw from this value, then
	 * rendering, must not pair a new start with the previous clock. */
	lock_transition(transition);
	float t = transition_get_video_time(transition);
	unlock_transition(transition);
	return t;
}

void transition_stop(obs_source_t *transition)
{
	obs_source_t *old_child = transition->transition_sources[0];

	if (old_child && transition->transition_source_active[0])
		obs_source_remove_active_child(transition, old_child);
	obs_source_release(old_child);

	transition->transition_source_active[0] = true;
	transition->transition_source_active[1] = false;
	transition->transition_sources[0] = transition->transition_sources[1];
	transition->transition_sources[1] = NULL;
}

void transition_handle_stop(obs_source_t *transition)
{
	if (transition->info.transition_stop)
		transition->info.transition_stop(transition->context.data);
	obs_source_dosignal(transition, "source_transition_stop", "transition_stop");
}

void obs_transition_force_stop(obs_source_t *transition)
{
	transition_handle_stop(transition);
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef lock_transition
#undef unlock_transition
