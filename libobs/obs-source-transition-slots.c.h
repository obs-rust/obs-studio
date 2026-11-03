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
#define lock_textures(transition) pthread_mutex_lock(&transition->transition_tex_mutex)
#define unlock_textures(transition) pthread_mutex_unlock(&transition->transition_tex_mutex)

void obs_transition_clear(obs_source_t *transition)
{
	obs_source_t *s[2];
	bool active[2];

	if (!transition_valid(transition, "obs_transition_clear"))
		return;

	lock_transition(transition);
	for (size_t i = 0; i < 2; i++) {
		s[i] = transition->transition_sources[i];
		active[i] = transition->transition_source_active[i];
		transition->transition_sources[i] = NULL;
		transition->transition_source_active[i] = false;
	}
	transition->transitioning_video = false;
	transition->transitioning_audio = false;
	unlock_transition(transition);

	for (size_t i = 0; i < 2; i++) {
		if (s[i] && active[i])
			obs_source_remove_active_child(transition, s[i]);
		obs_source_release(s[i]);
	}
}

void obs_transition_enum_sources(obs_source_t *transition, obs_source_enum_proc_t cb, void *param)
{
	lock_transition(transition);
	for (size_t i = 0; i < 2; i++) {
		if (transition->transition_sources[i])
			cb(transition, transition->transition_sources[i], param);
	}
	unlock_transition(transition);
}

static inline obs_source_t *copy_source_state(obs_source_t *tr_dest, obs_source_t *tr_source, size_t idx)
{
	obs_source_t *old_child = tr_dest->transition_sources[idx];
	obs_source_t *new_child = obs_source_get_ref(tr_source->transition_sources[idx]);
	bool active = tr_source->transition_source_active[idx];

	if (old_child && tr_dest->transition_source_active[idx])
		obs_source_remove_active_child(tr_dest, old_child);

	/* The add fails for a source that contains tr_dest, or for tr_dest itself.
	 * Store nothing then, as obs_transition_set() refuses such a source: a
	 * walk of the slot's tree would enter tr_dest again and lock its mutex
	 * twice. tr_source still holds its own reference until the swap ends. */
	bool adds = obs_source_transition_start_child_active(new_child != NULL, active);

	if (!obs_source_transition_load_copy_stores(adds, adds && obs_source_add_active_child(tr_dest, new_child))) {
		blog(LOG_WARNING,
		     "obs_transition_swap_begin: cannot give transition '%s' source '%s' due to infinite recursion",
		     obs_source_get_name(tr_dest), obs_source_get_name(new_child));
		obs_source_release(new_child);
		new_child = NULL;
		active = false;
	}

	tr_dest->transition_sources[idx] = new_child;
	tr_dest->transition_source_active[idx] = active;

	return old_child;
}

void obs_transition_swap_begin(obs_source_t *tr_dest, obs_source_t *tr_source)
{
	obs_source_t *old_children[2];

	if (!obs_source_transition_load_swaps(tr_dest == tr_source))
		return;

	lock_textures(tr_source);
	lock_textures(tr_dest);

	lock_transition(tr_source);
	lock_transition(tr_dest);

	for (size_t i = 0; i < 2; i++)
		old_children[i] = copy_source_state(tr_dest, tr_source, i);

	/* Hand over a running transition too, not only its sources: a
	 * transition that holds both sources without transitioning never
	 * finishes, and starting it towards either source does nothing. */
	tr_dest->transitioning_video = tr_source->transitioning_video;
	tr_dest->transitioning_audio = tr_source->transitioning_audio;
	tr_dest->transition_start_time = tr_source->transition_start_time;
	tr_dest->transition_duration = tr_source->transition_duration;
	tr_dest->transition_mode = tr_source->transition_mode;
	tr_dest->transition_manual_val = tr_source->transition_manual_val;
	tr_dest->transition_manual_target = tr_source->transition_manual_target;

	unlock_transition(tr_dest);
	unlock_transition(tr_source);

	for (size_t i = 0; i < 2; i++)
		obs_source_release(old_children[i]);
}

void obs_transition_swap_end(obs_source_t *tr_dest, obs_source_t *tr_source)
{
	if (!obs_source_transition_load_swaps(tr_dest == tr_source))
		return;

	obs_transition_clear(tr_source);

	for (size_t i = 0; i < 2; i++) {
		gs_texrender_t *dest = tr_dest->transition_texrender[i];
		gs_texrender_t *source = tr_source->transition_texrender[i];

		tr_dest->transition_texrender[i] = source;
		tr_source->transition_texrender[i] = dest;
	}

	unlock_textures(tr_dest);
	unlock_textures(tr_source);
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef lock_transition
#undef unlock_transition
#undef lock_textures
#undef unlock_textures
