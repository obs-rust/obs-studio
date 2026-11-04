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
#define trylock_textures(transition) pthread_mutex_trylock(&transition->transition_tex_mutex)
#define unlock_textures(transition) pthread_mutex_unlock(&transition->transition_tex_mutex)

void add_alignment(struct vec2 *v, uint32_t align, int cx, int cy);

uint32_t transition_get_cx(obs_source_t *tr)
{
	return obs_source_transition_clock_size(tr->transition_cx, tr->transition_actual_cx);
}

uint32_t transition_get_cy(obs_source_t *tr)
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
	float tr_cx = (float)transition_get_cx(tr);
	float tr_cy = (float)transition_get_cy(tr);
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

void transition_recalculate_matrices(obs_source_t *transition)
{
	recalculate_transition_matrix(transition, 0);
	recalculate_transition_matrix(transition, 1);
}

void transition_recalculate_size(obs_source_t *transition)
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
	transition_recalculate_size(transition);
	transition_recalculate_matrices(transition);

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

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef lock_transition
#undef unlock_transition
#undef trylock_textures
#undef unlock_textures
