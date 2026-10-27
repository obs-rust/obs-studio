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

#define trylock_textures(transition) pthread_mutex_trylock(&transition->transition_tex_mutex)
#define lock_textures(transition) pthread_mutex_lock(&transition->transition_tex_mutex)
#define unlock_textures(transition) pthread_mutex_unlock(&transition->transition_tex_mutex)

_Static_assert(OBS_SOURCE_TRANSITION_CLOCK_MODE_MANUAL == OBS_TRANSITION_MODE_MANUAL, "OBS_TRANSITION_MODE_MANUAL");
_Static_assert(OBS_SOURCE_TRANSITION_CLOCK_SCALE_MAX_ONLY == OBS_TRANSITION_SCALE_MAX_ONLY,
	       "OBS_TRANSITION_SCALE_MAX_ONLY");
_Static_assert(OBS_SOURCE_TRANSITION_CLOCK_SCALE_ASPECT == OBS_TRANSITION_SCALE_ASPECT, "OBS_TRANSITION_SCALE_ASPECT");
_Static_assert(OBS_SOURCE_TRANSITION_CLOCK_SCALE_STRETCH == OBS_TRANSITION_SCALE_STRETCH,
	       "OBS_TRANSITION_SCALE_STRETCH");
_Static_assert(OBS_SOURCE_TRANSITION_CLOCK_CS_SRGB == GS_CS_SRGB, "GS_CS_SRGB");
_Static_assert(OBS_SOURCE_TRANSITION_CLOCK_CS_SRGB_16F == GS_CS_SRGB_16F, "GS_CS_SRGB_16F");
_Static_assert(OBS_SOURCE_TRANSITION_CLOCK_CS_709_EXTENDED == GS_CS_709_EXTENDED, "GS_CS_709_EXTENDED");
_Static_assert(OBS_SOURCE_TRANSITION_CLOCK_CS_709_SCRGB == GS_CS_709_SCRGB, "GS_CS_709_SCRGB");
_Static_assert(sizeof(enum obs_transition_mode) == sizeof(int), "enum obs_transition_mode is passed as int");
_Static_assert(sizeof(enum obs_transition_scale_type) == sizeof(int),
	       "enum obs_transition_scale_type is passed as int");
_Static_assert(sizeof(enum gs_color_space) == sizeof(int), "enum gs_color_space is passed as int");
_Static_assert(OBS_SOURCE_TRANSITION_RENDER_TARGET_B == OBS_TRANSITION_SOURCE_B, "OBS_TRANSITION_SOURCE_B");
_Static_assert(OBS_SOURCE_TRANSITION_RENDER_AUDIO_FRAMES == AUDIO_OUTPUT_FRAMES, "AUDIO_OUTPUT_FRAMES");
_Static_assert(sizeof(enum obs_transition_target) == sizeof(int), "enum obs_transition_target is passed as int");
_Static_assert(OBS_SOURCE_TRANSITION_START_MODE_MANUAL == OBS_TRANSITION_MODE_MANUAL, "OBS_TRANSITION_MODE_MANUAL");
_Static_assert(OBS_SOURCE_TRANSITION_LOAD_TYPE_TRANSITION == OBS_SOURCE_TYPE_TRANSITION, "OBS_SOURCE_TYPE_TRANSITION");
_Static_assert(sizeof(enum obs_source_type) == sizeof(int), "enum obs_source_type is passed as int");
_Static_assert(OBS_SOURCE_TRANSITION_LOAD_SCALE_MAX_ONLY == OBS_TRANSITION_SCALE_MAX_ONLY,
	       "OBS_TRANSITION_SCALE_MAX_ONLY");
_Static_assert(OBS_SOURCE_TRANSITION_LOAD_SCALE_ASPECT == OBS_TRANSITION_SCALE_ASPECT, "OBS_TRANSITION_SCALE_ASPECT");
_Static_assert(OBS_SOURCE_TRANSITION_LOAD_SCALE_STRETCH == OBS_TRANSITION_SCALE_STRETCH,
	       "OBS_TRANSITION_SCALE_STRETCH");

static inline bool transition_valid(const obs_source_t *transition, const char *func)
{
	if (!obs_ptr_valid(transition, func))
		return false;
	else if (!obs_source_transition_load_is_transition((int)transition->info.type))
		return false;

	return true;
}

bool obs_transition_init(obs_source_t *transition)
{
	pthread_mutex_init_value(&transition->transition_mutex);
	pthread_mutex_init_value(&transition->transition_tex_mutex);
	if (pthread_mutex_init(&transition->transition_mutex, NULL) != 0)
		return false;
	if (pthread_mutex_init(&transition->transition_tex_mutex, NULL) != 0)
		return false;

	transition->transition_alignment = OBS_ALIGN_LEFT | OBS_ALIGN_TOP;
	transition->transition_texrender[0] = gs_texrender_create(GS_RGBA, GS_ZS_NONE);
	transition->transition_texrender[1] = gs_texrender_create(GS_RGBA, GS_ZS_NONE);
	transition->transition_source_active[0] = true;

	return transition->transition_texrender[0] != NULL && transition->transition_texrender[1] != NULL;
}

void obs_transition_free(obs_source_t *transition)
{
	pthread_mutex_destroy(&transition->transition_mutex);
	pthread_mutex_destroy(&transition->transition_tex_mutex);

	gs_enter_context(obs->video.graphics);
	gs_texrender_destroy(transition->transition_texrender[0]);
	gs_texrender_destroy(transition->transition_texrender[1]);
	gs_leave_context();
}

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

void add_alignment(struct vec2 *v, uint32_t align, int cx, int cy);

static inline uint32_t get_cx(obs_source_t *tr)
{
	return obs_source_transition_clock_size(tr->transition_cx, tr->transition_actual_cx);
}

static inline uint32_t get_cy(obs_source_t *tr)
{
	return obs_source_transition_clock_size(tr->transition_cy, tr->transition_actual_cy);
}

static void recalculate_transition_matrix(obs_source_t *tr, size_t idx)
{
	obs_source_t *child;
	struct matrix4 mat;
	struct vec2 pos;
	struct vec2 scale = {.x = 1.0f, .y = 1.0f};
	struct obs_source_transition_render_offset offset;
	float tr_cx = (float)get_cx(tr);
	float tr_cy = (float)get_cy(tr);
	float source_cx;
	float source_cy;
	enum obs_transition_scale_type scale_type = tr->transition_scale_type;

	lock_transition(tr);

	child = tr->transition_sources[idx];
	if (!child) {
		unlock_transition(tr);
		return;
	}

	source_cx = (float)obs_source_get_width(child);
	source_cy = (float)obs_source_get_height(child);
	unlock_transition(tr);

	if (!obs_source_transition_clock_fit(tr_cx, tr_cy, source_cx, source_cy, (int)scale_type, &scale.x, &scale.y))
		return;

	offset = obs_source_transition_render_offset(tr_cx, tr_cy, source_cx, source_cy, scale.x, scale.y);

	vec2_zero(&pos);
	add_alignment(&pos, tr->transition_alignment, (int)offset.x, (int)offset.y);

	matrix4_identity(&mat);
	matrix4_scale3f(&mat, &mat, scale.x, scale.y, 1.0f);
	matrix4_translate3f(&mat, &mat, pos.x, pos.y, 0.0f);
	matrix4_copy(&tr->transition_matrices[idx], &mat);
}

static inline void recalculate_transition_matrices(obs_source_t *transition)
{
	recalculate_transition_matrix(transition, 0);
	recalculate_transition_matrix(transition, 1);
}

static void recalculate_transition_size(obs_source_t *transition)
{
	uint32_t cx = 0, cy = 0;
	obs_source_t *child;

	lock_transition(transition);

	for (size_t i = 0; i < 2; i++) {
		child = transition->transition_sources[i];
		if (child) {
			uint32_t new_cx = obs_source_get_width(child);
			uint32_t new_cy = obs_source_get_height(child);
			cx = obs_source_transition_clock_fold(cx, new_cx);
			cy = obs_source_transition_clock_fold(cy, new_cy);
		}
	}

	unlock_transition(transition);

	transition->transition_actual_cx = cx;
	transition->transition_actual_cy = cy;
}

void obs_transition_tick(obs_source_t *transition, float t)
{
	recalculate_transition_size(transition);
	recalculate_transition_matrices(transition);

	switch (obs_source_transition_clock_manual((int)transition->transition_mode,
						   transition->transition_manual_torque)) {
	case OBS_SOURCE_TRANSITION_CLOCK_SNAP:
		transition->transition_manual_val = transition->transition_manual_target;
		break;
	case OBS_SOURCE_TRANSITION_CLOCK_STEP:
		transition->transition_manual_val =
			calc_torquef(transition->transition_manual_val, transition->transition_manual_target,
				     transition->transition_manual_torque, transition->transition_manual_clamp, t);
		break;
	case OBS_SOURCE_TRANSITION_CLOCK_HOLD:
		break;
	}

	if (trylock_textures(transition) == 0) {
		gs_texrender_reset(transition->transition_texrender[0]);
		gs_texrender_reset(transition->transition_texrender[1]);
		unlock_textures(transition);
	}
}

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
		obs_source_transition_start_restart(transition->transitioning_audio || transition->transitioning_video,
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

	if (obs_source_transition_start_unsized(transition->transition_cx) ||
	    obs_source_transition_start_unsized(transition->transition_cy)) {
		recalculate_transition_size(transition);
		recalculate_transition_matrices(transition);
	}

	obs_source_release(old_child);
	return true;
}

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
	active = transition->transitioning_audio || transition->transitioning_video;
	ret = transition->transition_sources[obs_source_transition_start_active_index(active)];
	ret = obs_source_get_ref(ret);
	unlock_transition(transition);

	return ret;
}

static bool activate_transition(obs_source_t *transition, size_t idx, obs_source_t *child)
{
	if (obs_source_transition_start_activates(transition->transition_source_active[idx])) {
		if (!obs_source_add_active_child(transition, child))
			return false;

		transition->transition_source_active[idx] = true;
	}

	transition->transitioning_video = true;
	transition->transitioning_audio = true;
	return true;
}

static inline bool transition_active(obs_source_t *transition)
{
	return transition->transitioning_audio || transition->transitioning_video;
}

bool obs_transition_is_active(obs_source_t *transition)
{
	return transition_active(transition);
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
		transition->transitioning_video = true;
		transition->transitioning_audio = true;
	}

	obs_source_dosignal(transition, "source_transition_start", "transition_start");

	recalculate_transition_size(transition);
	recalculate_transition_matrices(transition);

	return true;
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
	transition->transitioning_video = false;
	transition->transitioning_audio = false;
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

static float calc_time(obs_source_t *transition, uint64_t ts)
{
	return obs_source_transition_clock_time((int)transition->transition_mode, transition->transition_manual_val,
						transition->transition_start_time, transition->transition_duration, ts);
}

static inline float get_video_time(obs_source_t *transition)
{
	uint64_t ts = obs->video.video_time;
	return calc_time(transition, ts);
}

float obs_transition_get_time(obs_source_t *transition)
{
	/* Read under the lock that publishes a start's clock, as the render
	 * functions do: a caller choosing what to draw from this value, then
	 * rendering, must not pair a new start with the previous clock. */
	lock_transition(transition);
	float t = get_video_time(transition);
	unlock_transition(transition);
	return t;
}

static inline gs_texture_t *get_texture(obs_source_t *transition, enum obs_transition_target target)
{
	size_t idx = (size_t)target;
	return gs_texrender_get_texture(transition->transition_texrender[idx]);
}

static inline enum obs_transition_scale_type valid_scale_type(int64_t type)
{
	return (enum obs_transition_scale_type)obs_source_transition_load_scale_type(type);
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

	recalculate_transition_size(tr);
	recalculate_transition_matrices(tr);
}

struct transition_state {
	obs_source_t *s[2];
	bool transitioning_video;
	bool transitioning_audio;
};

static inline void copy_transition_state(obs_source_t *transition, struct transition_state *state)
{
	state->s[0] = obs_source_get_ref(transition->transition_sources[0]);
	state->s[1] = obs_source_get_ref(transition->transition_sources[1]);

	state->transitioning_video = transition->transitioning_video;
	state->transitioning_audio = transition->transitioning_audio;
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

static inline void render_child(obs_source_t *transition, obs_source_t *child, size_t idx, enum gs_color_space space)
{
	uint32_t cx = get_cx(transition);
	uint32_t cy = get_cy(transition);
	struct vec4 blank;
	if (!child)
		return;

	enum gs_color_format format = gs_get_format_from_space(space);
	if (gs_texrender_get_format(transition->transition_texrender[idx]) != format) {
		gs_texrender_destroy(transition->transition_texrender[idx]);
		transition->transition_texrender[idx] = gs_texrender_create(format, GS_ZS_NONE);
	}

	if (gs_texrender_begin_with_color_space(transition->transition_texrender[idx], cx, cy, space)) {
		vec4_zero(&blank);
		gs_clear(GS_CLEAR_COLOR, &blank, 0.0f, 0);
		gs_ortho(0.0f, (float)cx, 0.0f, (float)cy, -100.0f, 100.0f);

		gs_matrix_push();
		gs_matrix_mul(&transition->transition_matrices[idx]);
		obs_source_video_render(child);
		gs_matrix_pop();

		gs_texrender_end(transition->transition_texrender[idx]);
	}
}

static void obs_transition_stop(obs_source_t *transition)
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

static inline void handle_stop(obs_source_t *transition)
{
	if (transition->info.transition_stop)
		transition->info.transition_stop(transition->context.data);
	obs_source_dosignal(transition, "source_transition_stop", "transition_stop");
}

void obs_transition_force_stop(obs_source_t *transition)
{
	handle_stop(transition);
}

void obs_transition_video_render(obs_source_t *transition, obs_transition_video_render_callback_t callback)
{
	obs_transition_video_render2(transition, callback, obs->video.transparent_texture);
}

void obs_transition_video_render2(obs_source_t *transition, obs_transition_video_render_callback_t callback,
				  gs_texture_t *placeholder_texture)
{
	struct transition_state state;
	struct matrix4 matrices[2];
	enum obs_source_transition_render_draw_step draw;
	bool locked = false;
	bool stopped = false;
	bool video_stopped = false;
	float t;

	if (!transition_valid(transition, "obs_transition_video_render"))
		return;

	/* Starts write the clock under this lock with the transitioning flags;
	 * reading it before the lock could pair a new start with an old clock. */
	lock_transition(transition);

	t = get_video_time(transition);

	if (obs_source_transition_render_video_due(t, transition->transitioning_video)) {
		enum obs_source_transition_render_video_step step;

		transition->transitioning_video = false;
		video_stopped = true;

		/* no inactive rule here: active is true */
		step = obs_source_transition_render_video_stop(transition->transitioning_audio, true);
		if (step != OBS_SOURCE_TRANSITION_RENDER_VIDEO_END) {
			obs_transition_stop(transition);
			stopped = true;
		}
	}
	copy_transition_state(transition, &state);
	matrices[0] = transition->transition_matrices[0];
	matrices[1] = transition->transition_matrices[1];

	unlock_transition(transition);

	if (state.transitioning_video)
		locked = trylock_textures(transition) == 0;

	draw = obs_source_transition_render_draw(state.transitioning_video, locked, callback != NULL,
						 state.transitioning_audio);

	if (draw == OBS_SOURCE_TRANSITION_RENDER_DRAW_CALLBACK) {
		gs_texture_t *tex[2];
		uint32_t cx;
		uint32_t cy;

		const enum gs_color_space current_space = gs_get_color_space();
		const enum gs_color_space source_space = obs_source_get_color_space(transition, 1, &current_space);
		for (size_t i = 0; i < 2; i++) {
			if (state.s[i]) {
				render_child(transition, state.s[i], i, source_space);
				tex[i] = get_texture(transition, i);
				if (!tex[i])
					tex[i] = placeholder_texture;
			} else {
				tex[i] = placeholder_texture;
			}
		}

		cx = get_cx(transition);
		cy = get_cy(transition);
		if (cx && cy) {
			gs_blend_state_push();
			gs_blend_function(GS_BLEND_ONE, GS_BLEND_INVSRCALPHA);

			callback(transition->context.data, tex[0], tex[1], t, cx, cy);

			gs_blend_state_pop();
		}

	} else if (draw == OBS_SOURCE_TRANSITION_RENDER_DRAW_B) {
		if (state.s[1]) {
			gs_matrix_push();
			gs_matrix_mul(&matrices[1]);
			obs_source_video_render(state.s[1]);
			gs_matrix_pop();
		}
	} else {
		if (state.s[0]) {
			gs_matrix_push();
			gs_matrix_mul(&matrices[0]);
			obs_source_video_render(state.s[0]);
			gs_matrix_pop();
		}
	}

	if (locked)
		unlock_textures(transition);

	obs_source_release(state.s[0]);
	obs_source_release(state.s[1]);

	if (video_stopped)
		obs_source_dosignal(transition, "source_transition_video_stop", "transition_video_stop");
	if (stopped)
		handle_stop(transition);
}

static enum gs_color_space mix_spaces(enum gs_color_space a, enum gs_color_space b)
{
	return (enum gs_color_space)obs_source_transition_clock_mix_spaces((int)a, (int)b);
}

enum gs_color_space obs_transition_video_get_color_space(obs_source_t *transition)
{
	obs_source_t *source0 = transition->transition_sources[0];
	obs_source_t *source1 = transition->transition_sources[1];

	const enum gs_color_space preferred_spaces[] = {
		GS_CS_SRGB,
		GS_CS_SRGB_16F,
		GS_CS_709_EXTENDED,
	};

	enum gs_color_space space = GS_CS_SRGB;

	if (source0) {
		space = mix_spaces(space, obs_source_get_color_space(source0, OBS_COUNTOF(preferred_spaces),
								     preferred_spaces));
	}

	if (source1) {
		space = mix_spaces(space, obs_source_get_color_space(source1, OBS_COUNTOF(preferred_spaces),
								     preferred_spaces));
	}

	return space;
}

bool obs_transition_video_render_direct(obs_source_t *transition, enum obs_transition_target target)
{
	struct transition_state state;
	struct matrix4 matrices[2];
	bool stopped = false;
	bool video_stopped = false;
	bool transitioning;
	float t;

	if (!transition_valid(transition, "obs_transition_video_render"))
		return false;

	/* Starts write the clock under this lock with the transitioning flags;
	 * reading it before the lock could pair a new start with an old clock. */
	lock_transition(transition);

	t = get_video_time(transition);

	if (obs_source_transition_render_video_due(t, transition->transitioning_video)) {
		enum obs_source_transition_render_video_step step;

		transition->transitioning_video = false;
		video_stopped = true;

		step = obs_source_transition_render_video_stop(transition->transitioning_audio,
							       obs_source_active(transition));
		if (step == OBS_SOURCE_TRANSITION_RENDER_VIDEO_STOP_AUDIO)
			transition->transitioning_audio = false;

		if (step != OBS_SOURCE_TRANSITION_RENDER_VIDEO_END) {
			obs_transition_stop(transition);
			stopped = true;
		}
	}
	copy_transition_state(transition, &state);
	transitioning = state.transitioning_audio || state.transitioning_video;
	matrices[0] = transition->transition_matrices[0];
	matrices[1] = transition->transition_matrices[1];

	unlock_transition(transition);

	int idx = obs_source_transition_render_direct_index(transitioning, (int)target);
	if (state.s[idx]) {
		gs_matrix_push();
		gs_matrix_mul(&matrices[idx]);
		obs_source_video_render(state.s[idx]);
		gs_matrix_pop();
	}

	obs_source_release(state.s[0]);
	obs_source_release(state.s[1]);

	if (video_stopped)
		obs_source_dosignal(transition, "source_transition_video_stop", "transition_video_stop");
	if (stopped)
		handle_stop(transition);

	return transitioning;
}

static inline float get_sample_time(obs_source_t *transition, size_t sample_rate, size_t sample, uint64_t ts)
{
	uint64_t sample_ts_offset = util_mul_div64(sample, 1000000000ULL, sample_rate);
	uint64_t i_ts = ts + sample_ts_offset;
	return calc_time(transition, i_ts);
}

static inline void mix_child(obs_source_t *transition, float *out, float *in, size_t count, size_t sample_rate,
			     uint64_t ts, obs_transition_audio_mix_callback_t mix)
{
	void *context_data = transition->context.data;

	for (size_t i = 0; i < count; i++) {
		float t = get_sample_time(transition, sample_rate, i, ts);
		out[i] += in[i] * mix(context_data, t);
	}
}

static void transition_process_audio(obs_source_t *transition, obs_source_t *child, struct obs_source_audio_mix *audio,
				     uint64_t min_ts, uint32_t mixers, size_t channels, size_t sample_rate,
				     obs_transition_audio_mix_callback_t mix)
{
	bool valid = obs_source_transition_render_audio_valid(child != NULL, child && child->audio_pending,
							      child && child->audio_is_duplicated);
	enum obs_source_transition_render_audio_place place;
	struct obs_source_audio_mix child_audio;
	uint64_t ts;
	size_t pos = 0;

	if (!valid)
		return;

	ts = obs_source_get_audio_timestamp(child);
	place = obs_source_transition_render_audio_place(ts, min_ts, sample_rate, &pos);
	if (place == OBS_SOURCE_TRANSITION_RENDER_PLACE_NONE)
		return;
	obs_source_get_audio_mix(child, &child_audio);

	if (place == OBS_SOURCE_TRANSITION_RENDER_PLACE_LATE)
		return;

	for (size_t mix_idx = 0; mix_idx < MAX_AUDIO_MIXES; mix_idx++) {
		struct audio_output_data *output = &audio->output[mix_idx];
		struct audio_output_data *input = &child_audio.output[mix_idx];

		if ((mixers & (1 << mix_idx)) == 0)
			continue;

		for (size_t ch = 0; ch < channels; ch++) {
			float *out = output->data[ch];
			float *in = input->data[ch];

			mix_child(transition, out + pos, in, AUDIO_OUTPUT_FRAMES - pos, sample_rate, ts, mix);
		}
	}
}

/* The transition's children, leaving out any that was removed. */
static inline void transition_live_sources(obs_source_t *transition, obs_source_t *sources[2])
{
	for (size_t i = 0; i < 2; i++) {
		sources[i] = transition->transition_sources[i];
		if (sources[i] && obs_source_removed(sources[i]))
			sources[i] = NULL;
	}
}

static inline uint64_t transition_calc_min_ts(obs_source_t *sources[2])
{
	uint64_t timestamps[2] = {0, 0};

	for (size_t i = 0; i < 2; i++) {
		if (sources[i] && !sources[i]->audio_pending)
			timestamps[i] = obs_source_get_audio_timestamp(sources[i]);
	}

	return obs_source_transition_clock_min_ts(timestamps[0], timestamps[1]);
}

bool obs_transition_audio_render(obs_source_t *transition, uint64_t *ts_out, struct obs_source_audio_mix *audio,
				 uint32_t mixers, size_t channels, size_t sample_rate,
				 obs_transition_audio_mix_callback_t mix_a, obs_transition_audio_mix_callback_t mix_b)
{
	obs_source_t *sources[2];
	struct transition_state state = {0};
	enum obs_source_transition_render_audio_step step;
	bool stopped = false;
	uint64_t min_ts;
	uint32_t mix;
	float t = 0.0f;

	if (!transition_valid(transition, "obs_transition_audio_render"))
		return false;

	lock_transition(transition);

	transition_live_sources(transition, sources);
	min_ts = transition_calc_min_ts(sources);

	if (min_ts)
		t = calc_time(transition, min_ts);

	step = obs_source_transition_render_audio_stop(min_ts != 0, t, transition->transitioning_video,
						       transition->transitioning_audio);
	if (step != OBS_SOURCE_TRANSITION_RENDER_AUDIO_KEEP) {
		transition->transitioning_audio = false;
		if (step == OBS_SOURCE_TRANSITION_RENDER_AUDIO_STOP) {
			obs_transition_stop(transition);
			stopped = true;
		}
	}

	if (min_ts) {
		/* The audio stop can make a removed child source A (#2185). */
		transition_live_sources(transition, sources);
		min_ts = transition_calc_min_ts(sources);
		if (min_ts) {
			state.s[0] = obs_source_get_ref(sources[0]);
			state.s[1] = obs_source_get_ref(sources[1]);
			state.transitioning_video = transition->transitioning_video;
			state.transitioning_audio = transition->transitioning_audio;
		}
	}

	unlock_transition(transition);

	if (min_ts) {
		mix = obs_source_transition_render_audio_mix(state.transitioning_audio, state.s[0] != NULL,
							     state.s[1] != NULL);
		if (mix & OBS_SOURCE_TRANSITION_RENDER_MIX_A)
			transition_process_audio(transition, state.s[0], audio, min_ts, mixers, channels, sample_rate,
						 mix_a);
		if (mix & OBS_SOURCE_TRANSITION_RENDER_MIX_B)
			transition_process_audio(transition, state.s[1], audio, min_ts, mixers, channels, sample_rate,
						 mix_b);
		if (mix & OBS_SOURCE_TRANSITION_RENDER_COPY_A)
			memcpy(audio->output[0].data[0], state.s[0]->audio_output_buf[0][0], TOTAL_AUDIO_SIZE);

		obs_source_release(state.s[0]);
		obs_source_release(state.s[1]);
	}

	if (stopped)
		handle_stop(transition);

	*ts_out = min_ts;
	return !!min_ts;
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
#undef trylock_textures
#undef lock_textures
#undef unlock_textures
