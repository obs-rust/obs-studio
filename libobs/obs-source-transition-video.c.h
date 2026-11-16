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

#include "libobs/build/libobs-ported-source-transition-video.pch.h"

#define lock_transition(transition) pthread_mutex_lock(&transition->transition_mutex);
#define unlock_transition(transition) pthread_mutex_unlock(&transition->transition_mutex);

#define trylock_textures(transition) pthread_mutex_trylock(&transition->transition_tex_mutex)
#define lock_textures(transition) pthread_mutex_lock(&transition->transition_tex_mutex)
#define unlock_textures(transition) pthread_mutex_unlock(&transition->transition_tex_mutex)

static inline gs_texture_t *get_texture(obs_source_t *transition, enum obs_transition_target target)
{
	size_t idx = (size_t)target;
	return gs_texrender_get_texture(transition->transition_texrender[idx]);
}

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

	t = transition_get_video_time(transition);

	if (obs_source_transition_render_video_due(t, os_atomic_load_bool(&transition->transitioning_video))) {
		enum obs_source_transition_render_video_step step;

		os_atomic_store_bool(&transition->transitioning_video, false);
		video_stopped = true;

		/* no inactive rule here: active is true */
		step = obs_source_transition_render_video_stop(os_atomic_load_bool(&transition->transitioning_audio),
							       true);
		if (step != OBS_SOURCE_TRANSITION_RENDER_VIDEO_END) {
			transition_stop(transition);
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
		transition_handle_stop(transition);
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

	t = transition_get_video_time(transition);

	if (obs_source_transition_render_video_due(t, os_atomic_load_bool(&transition->transitioning_video))) {
		enum obs_source_transition_render_video_step step;

		os_atomic_store_bool(&transition->transitioning_video, false);
		video_stopped = true;

		step = obs_source_transition_render_video_stop(os_atomic_load_bool(&transition->transitioning_audio),
							       obs_source_active(transition));
		if (step == OBS_SOURCE_TRANSITION_RENDER_VIDEO_STOP_AUDIO)
			os_atomic_store_bool(&transition->transitioning_audio, false);

		if (step != OBS_SOURCE_TRANSITION_RENDER_VIDEO_END) {
			transition_stop(transition);
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
		transition_handle_stop(transition);

	return transitioning;
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef lock_transition
#undef unlock_transition
#undef trylock_textures
#undef lock_textures
#undef unlock_textures
