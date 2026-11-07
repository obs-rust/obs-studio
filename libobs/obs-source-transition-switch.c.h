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

/* A start's clock, decided from the state its destination admission replaces
 * (after any plugin re-entry) and written only once that admission succeeds. */
struct transition_start_clock {
	uint64_t duration;
	bool same_as_source;
	bool same_as_dest;
	bool restart;
};

static inline void write_start_clock(obs_source_t *transition, const struct transition_start_clock *clock)
{
	if (clock->restart) {
		transition->transition_start_time = os_gettime_ns();
		transition->transition_duration = clock->duration;
	}
}

static bool set_source(obs_source_t *transition, enum obs_transition_target target, obs_source_t *new_child,
		       bool (*callback)(obs_source_t *t, size_t idx, obs_source_t *c),
		       struct transition_start_clock *clock)
{
	size_t idx = (size_t)target;
	obs_source_t *old_child;
	bool add_success = true;
	bool already_active;

	if (new_child)
		new_child = obs_source_get_ref(new_child);

	lock_transition(transition);

	old_child = transition->transition_sources[idx];

	clock->same_as_source = new_child == transition->transition_sources[0];
	clock->same_as_dest = new_child == old_child;
	clock->restart =
		obs_source_transition_start_restart(os_atomic_load_bool(&transition->transitioning_audio) ||
							    os_atomic_load_bool(&transition->transitioning_video),
						    clock->same_as_dest, clock->same_as_source);

	if (new_child == old_child) {
		write_start_clock(transition, clock);
		unlock_transition(transition);
		obs_source_release(new_child);
		return true;
	}

	already_active = transition->transition_source_active[idx];

	if (obs_source_transition_start_adds(already_active, new_child != NULL))
		add_success = obs_source_add_active_child(transition, new_child);
	if (obs_source_transition_start_removes(already_active, old_child != NULL, add_success))
		obs_source_remove_active_child(transition, old_child);

	if (obs_source_transition_start_calls_back(callback != NULL, add_success))
		add_success = callback(transition, idx, new_child);

	if (add_success) {
		write_start_clock(transition, clock);
		transition->transition_sources[idx] = new_child;
	}

	unlock_transition(transition);

	if (!add_success) {
		obs_source_release(new_child);
		return false;
	}

	if (obs_source_transition_start_unsized((uint32_t)os_atomic_load_long(&transition->transition_cx)) ||
	    obs_source_transition_start_unsized((uint32_t)os_atomic_load_long(&transition->transition_cy))) {
		transition_recalculate_size(transition);
		transition_recalculate_matrices(transition);
	}

	obs_source_release(old_child);
	return true;
}

static bool activate_transition(obs_source_t *transition, size_t idx, obs_source_t *child)
{
	if (obs_source_transition_start_activates(transition->transition_source_active[idx])) {
		if (!obs_source_add_active_child(transition, child))
			return false;

		transition->transition_source_active[idx] = true;
	}

	os_atomic_store_bool(&transition->transitioning_video, true);
	os_atomic_store_bool(&transition->transitioning_audio, true);
	return true;
}

struct transition_recursion_check {
	obs_source_t *transition;
	bool recursive;
};

static void check_transition_recursion(obs_source_t *parent, obs_source_t *child, void *param)
{
	struct transition_recursion_check *check = param;
	if (parent == check->transition || child == check->transition)
		check->recursive = true;
}

bool obs_transition_start(obs_source_t *transition, enum obs_transition_mode mode, uint32_t duration_ms,
			  obs_source_t *dest)
{
	bool active;
	bool same_as_source;
	bool same_as_dest;
	bool same_mode;

	if (!transition_valid(transition, "obs_transition_start"))
		return false;

	lock_transition(transition);
	same_as_source = dest == transition->transition_sources[0];
	active = transition_active(transition);
	unlock_transition(transition);
	if (obs_source_transition_start_refused(same_as_source, active))
		return false;

	/* The destination walk can visit this transition. Check before child
	 * activation holds its mutex, and before changing state or calling plugins. */
	struct transition_recursion_check check = {transition, dest == transition};
	if (obs_source_transition_start_walks(dest != NULL, check.recursive))
		obs_source_enum_full_tree(dest, check_transition_recursion, &check);
	if (check.recursive) {
		blog(LOG_WARNING, "obs_transition_start: transition '%s' has recursive destination '%s'",
		     obs_source_get_name(transition), obs_source_get_name(dest));
		return false;
	}

	if (transition_active(transition)) {
		obs_transition_set(transition, transition->transition_sources[1]);
	}

	lock_transition(transition);
	same_as_source = dest == transition->transition_sources[0];
	same_as_dest = dest == transition->transition_sources[1];
	same_mode = mode == transition->transition_mode;
	active = transition_active(transition);
	unlock_transition(transition);

	switch (obs_source_transition_start_admit(same_as_source, same_as_dest, same_mode, (int)mode, active)) {
	case OBS_SOURCE_TRANSITION_START_REFUSE:
		return false;
	case OBS_SOURCE_TRANSITION_START_DONE:
		return true;
	case OBS_SOURCE_TRANSITION_START_GO:
		break;
	}

	/* Written before the plugin callback: a start the callback makes on this
	 * transition replaces the mode, and it stays even if this start then
	 * completes (unchanged upstream behaviour). */
	lock_transition(transition);
	transition->transition_mode = mode;
	transition->transition_manual_val = 0.0f;
	transition->transition_manual_target = 0.0f;
	unlock_transition(transition);

	if (transition->info.transition_start)
		transition->info.transition_start(transition->context.data);

	if (transition->transition_use_fixed_duration)
		duration_ms = transition->transition_fixed_duration;

	/* Plugin re-entry may have changed the destination since the preflight,
	 * or started this transition itself. Child admission checks again without
	 * descending into our held mutex, and the clock is decided from the state
	 * being replaced, so a rejected start leaves a nested start's clock intact. */
	struct transition_start_clock clock = {obs_source_transition_start_duration_ns(duration_ms)};
	if (!set_source(transition, OBS_TRANSITION_SOURCE_B, dest, activate_transition, &clock)) {
		blog(LOG_WARNING, "obs_transition_start: transition '%s' has recursive destination '%s'",
		     obs_source_get_name(transition), obs_source_get_name(dest));
		return false;
	}
	if (obs_source_transition_start_null_dest(dest != NULL, clock.same_as_dest, clock.same_as_source)) {
		os_atomic_store_bool(&transition->transitioning_video, true);
		os_atomic_store_bool(&transition->transitioning_audio, true);
	}

	obs_source_dosignal(transition, "source_transition_start", "transition_start");

	transition_recalculate_size(transition);
	transition_recalculate_matrices(transition);

	return true;
}

void obs_transition_set(obs_source_t *transition, obs_source_t *source)
{
	obs_source_t *s[2];
	bool active[2];

	if (!transition_valid(transition, "obs_transition_set"))
		return;

	/* Refuse a source that contains the transition, or the transition
	 * itself, as obs_transition_load() and obs_transition_start() do, before
	 * changing anything: a walk of its active tree would enter the transition
	 * again and lock its mutex twice. */
	struct transition_recursion_check check = {transition, source == transition};
	if (obs_source_transition_start_walks(source != NULL, check.recursive))
		obs_source_enum_full_tree(source, check_transition_recursion, &check);
	if (check.recursive) {
		blog(LOG_WARNING,
		     "obs_transition_set: cannot set transition '%s' to source '%s' due to infinite recursion",
		     obs_source_get_name(transition), obs_source_get_name(source));
		return;
	}

	source = obs_source_get_ref(source);

	if (transition_active(transition)) {
		obs_source_dosignal(transition, "source_transition_stop", "transition_stop");
	}

	lock_transition(transition);
	for (size_t i = 0; i < 2; i++) {
		s[i] = transition->transition_sources[i];
		active[i] = transition->transition_source_active[i];
		transition->transition_sources[i] = NULL;
		transition->transition_source_active[i] = false;
	}
	transition->transition_source_active[0] = true;
	transition->transition_sources[0] = source;
	os_atomic_store_bool(&transition->transitioning_video, false);
	os_atomic_store_bool(&transition->transitioning_audio, false);
	transition->transition_manual_val = 0.0f;
	transition->transition_manual_target = 0.0f;
	unlock_transition(transition);

	for (size_t i = 0; i < 2; i++) {
		if (obs_source_transition_start_child_active(s[i] != NULL, active[i]))
			obs_source_remove_active_child(transition, s[i]);
		obs_source_release(s[i]);
	}

	/* The add fails for a source that contains the transition: then slot A
	 * is not active, so the next set does not remove a reference that
	 * another parent holds. */
	if (source && !obs_source_add_active_child(transition, source)) {
		lock_transition(transition);
		if (transition->transition_sources[0] == source)
			transition->transition_source_active[0] = false;
		unlock_transition(transition);
	}
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef lock_transition
#undef unlock_transition
