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

static float calc_time(obs_source_t *transition, uint64_t ts)
{
	return obs_source_transition_clock_time((int)transition->transition_mode, transition->transition_manual_val,
						transition->transition_start_time, transition->transition_duration, ts);
}

static inline float get_video_time(obs_source_t *transition)
{
	uint64_t ts = obs_video_stats_frame_time(&obs->video.stats);
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

struct transition_state {
	obs_source_t *s[2];
	bool transitioning_video;
	bool transitioning_audio;

	/* The clock, copied under the lock for an audio block (#3499): the mix
	 * runs after the lock is dropped, and a write then must not split the
	 * block between two positions. */
	enum obs_transition_mode mode;
	float manual_val;
	uint64_t start_time;
	uint64_t duration;
};

static inline void copy_transition_state(obs_source_t *transition, struct transition_state *state)
{
	state->s[0] = obs_source_get_ref(transition->transition_sources[0]);
	state->s[1] = obs_source_get_ref(transition->transition_sources[1]);

	state->transitioning_video = os_atomic_load_bool(&transition->transitioning_video);
	state->transitioning_audio = os_atomic_load_bool(&transition->transitioning_audio);
}

static inline void render_child(obs_source_t *transition, obs_source_t *child, size_t idx, const struct matrix4 *matrix,
				enum gs_color_space space)
{
	uint32_t cx = transition_get_cx(transition);
	uint32_t cy = transition_get_cy(transition);
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
		gs_matrix_mul(matrix);
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

	if (obs_source_transition_render_video_due(t, os_atomic_load_bool(&transition->transitioning_video))) {
		enum obs_source_transition_render_video_step step;

		os_atomic_store_bool(&transition->transitioning_video, false);
		video_stopped = true;

		/* no inactive rule here: active is true */
		step = obs_source_transition_render_video_stop(os_atomic_load_bool(&transition->transitioning_audio),
							       true);
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
				render_child(transition, state.s[i], i, &matrices[i], source_space);
				tex[i] = get_texture(transition, (enum obs_transition_target)i);
				if (!tex[i])
					tex[i] = placeholder_texture;
			} else {
				tex[i] = placeholder_texture;
			}
		}

		cx = transition_get_cx(transition);
		cy = transition_get_cy(transition);
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

	if (obs_source_transition_render_video_due(t, os_atomic_load_bool(&transition->transitioning_video))) {
		enum obs_source_transition_render_video_step step;

		os_atomic_store_bool(&transition->transitioning_video, false);
		video_stopped = true;

		step = obs_source_transition_render_video_stop(os_atomic_load_bool(&transition->transitioning_audio),
							       obs_source_active(transition));
		if (step == OBS_SOURCE_TRANSITION_RENDER_VIDEO_STOP_AUDIO)
			os_atomic_store_bool(&transition->transitioning_audio, false);

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

static inline void mix_child(obs_source_t *transition, const struct transition_state *state, float *out, float *in,
			     size_t count, size_t sample_rate, uint64_t ts, obs_transition_audio_mix_callback_t mix)
{
	void *context_data = transition->context.data;

	for (size_t i = 0; i < count; i++) {
		float t = obs_source_transition_sample_time((int)state->mode, state->manual_val, state->start_time,
							    state->duration, ts, sample_rate, i);
		out[i] += in[i] * mix(context_data, t);
	}
}

static void transition_process_audio(obs_source_t *transition, const struct transition_state *state,
				     obs_source_t *child, struct obs_source_audio_mix *audio, uint64_t min_ts,
				     uint32_t mixers, size_t channels, size_t sample_rate,
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

			mix_child(transition, state, out + pos, in, AUDIO_OUTPUT_FRAMES - pos, sample_rate, ts, mix);
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

	step = obs_source_transition_render_audio_stop(min_ts != 0, t,
						       os_atomic_load_bool(&transition->transitioning_video),
						       os_atomic_load_bool(&transition->transitioning_audio));
	if (step != OBS_SOURCE_TRANSITION_RENDER_AUDIO_KEEP) {
		os_atomic_store_bool(&transition->transitioning_audio, false);
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
			state.transitioning_video = os_atomic_load_bool(&transition->transitioning_video);
			state.transitioning_audio = os_atomic_load_bool(&transition->transitioning_audio);
			state.mode = transition->transition_mode;
			state.manual_val = transition->transition_manual_val;
			state.start_time = transition->transition_start_time;
			state.duration = transition->transition_duration;
		}
	}

	unlock_transition(transition);

	if (min_ts) {
		mix = obs_source_transition_render_audio_mix(state.transitioning_audio, state.s[0] != NULL,
							     state.s[1] != NULL);
		if (mix & OBS_SOURCE_TRANSITION_RENDER_MIX_A)
			transition_process_audio(transition, &state, state.s[0], audio, min_ts, mixers, channels,
						 sample_rate, mix_a);
		if (mix & OBS_SOURCE_TRANSITION_RENDER_MIX_B)
			transition_process_audio(transition, &state, state.s[1], audio, min_ts, mixers, channels,
						 sample_rate, mix_b);
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

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef lock_transition
#undef unlock_transition
#undef trylock_textures
#undef lock_textures
#undef unlock_textures
