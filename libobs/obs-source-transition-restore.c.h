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

#include "libobs/build/libobs-ported-source-transition-size.pch.h"

#define lock_transition(transition) pthread_mutex_lock(&transition->transition_mutex);
#define unlock_transition(transition) pthread_mutex_unlock(&transition->transition_mutex);

void obs_transition_load(obs_source_t *tr, obs_data_t *data)
{
	const char *name = obs_data_get_string(data, "transition_source_a");
	int64_t alignment = obs_data_get_int(data, "transition_alignment");
	int64_t mode = obs_data_get_int(data, "transition_mode");
	int64_t scale_type = obs_data_get_int(data, "transition_scale_type");
	int64_t cx = obs_data_get_int(data, "transition_cx");
	int64_t cy = obs_data_get_int(data, "transition_cy");
	obs_source_t *source = NULL;
	obs_source_t *old;
	bool old_active;

	if (obs_source_transition_load_looks_up(name != NULL)) {
		source = obs_get_source_by_name(name);
		/* The add only for a found source, as before. */
		bool found = source != NULL;
		bool add_ok = found && obs_source_add_active_child(tr, source);

		switch (obs_source_transition_load_source(found, add_ok)) {
		case OBS_SOURCE_TRANSITION_LOAD_REFUSE:
			blog(LOG_WARNING,
			     "Cannot set transition '%s' "
			     "to source '%s' due to "
			     "infinite recursion",
			     tr->context.name, name);
			obs_source_release(source);
			source = NULL;
			break;
		case OBS_SOURCE_TRANSITION_LOAD_NOT_FOUND:
			blog(LOG_WARNING,
			     "Failed to find source '%s' for "
			     "transition '%s'",
			     name, tr->context.name);
			break;
		case OBS_SOURCE_TRANSITION_LOAD_KEEP:
			break;
		}
	}

	lock_transition(tr);
	old = tr->transition_sources[0];
	old_active = tr->transition_source_active[0];
	tr->transition_sources[0] = source;
	tr->transition_source_active[0] = true;
	tr->transition_alignment = (uint32_t)alignment;
	tr->transition_mode = (enum obs_transition_mode)mode;
	tr->transition_scale_type = valid_scale_type(scale_type);
	tr->transition_cx = (uint32_t)cx;
	tr->transition_cy = (uint32_t)cy;
	unlock_transition(tr);

	/* Take back the source the slot held, as obs_transition_clear() does. */
	if (obs_source_transition_start_child_active(old != NULL, old_active))
		obs_source_remove_active_child(tr, old);
	obs_source_release(old);

	transition_recalculate_size(tr);
	transition_recalculate_matrices(tr);
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef lock_transition
#undef unlock_transition
