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

/* Private adapter, not a new public API (obs-source-transition-seam.h). */

bool transition_valid(const obs_source_t *transition, const char *func)
{
	if (!obs_ptr_valid(transition, func))
		return false;
	else if (!obs_source_transition_load_is_transition((int)transition->info.type))
		return false;

	return true;
}

bool transition_active(obs_source_t *transition)
{
	return transition->transitioning_audio || transition->transitioning_video;
}

enum obs_transition_scale_type valid_scale_type(int64_t type)
{
	return (enum obs_transition_scale_type)obs_source_transition_load_scale_type(type);
}

obs_source_t *obs_source_transition_seam_get_sources(const obs_source_t *transition, size_t idx)
{
	return transition->transition_sources[idx];
}

void obs_source_transition_seam_set_sources(obs_source_t *transition, size_t idx, obs_source_t *value)
{
	transition->transition_sources[idx] = value;
}

const char *obs_source_transition_seam_get_child_name(const obs_source_t *child)
{
	return child->context.name;
}

bool obs_source_transition_seam_get_source_active(const obs_source_t *transition, size_t idx)
{
	return transition->transition_source_active[idx];
}

void obs_source_transition_seam_set_source_active(obs_source_t *transition, size_t idx, bool value)
{
	transition->transition_source_active[idx] = value;
}

bool obs_source_transition_seam_get_transitioning_video(const obs_source_t *transition)
{
	return transition->transitioning_video;
}

void obs_source_transition_seam_set_transitioning_video(obs_source_t *transition, bool value)
{
	transition->transitioning_video = value;
}

bool obs_source_transition_seam_get_transitioning_audio(const obs_source_t *transition)
{
	return transition->transitioning_audio;
}

void obs_source_transition_seam_set_transitioning_audio(obs_source_t *transition, bool value)
{
	transition->transitioning_audio = value;
}

enum obs_transition_mode obs_source_transition_seam_get_mode(const obs_source_t *transition)
{
	return transition->transition_mode;
}

void obs_source_transition_seam_set_mode(obs_source_t *transition, enum obs_transition_mode value)
{
	transition->transition_mode = value;
}

float obs_source_transition_seam_get_manual_val(const obs_source_t *transition)
{
	return transition->transition_manual_val;
}

void obs_source_transition_seam_set_manual_val(obs_source_t *transition, float value)
{
	transition->transition_manual_val = value;
}

float obs_source_transition_seam_get_manual_target(const obs_source_t *transition)
{
	return transition->transition_manual_target;
}

void obs_source_transition_seam_set_manual_target(obs_source_t *transition, float value)
{
	transition->transition_manual_target = value;
}

float obs_source_transition_seam_get_manual_torque(const obs_source_t *transition)
{
	return transition->transition_manual_torque;
}

void obs_source_transition_seam_set_manual_torque(obs_source_t *transition, float value)
{
	transition->transition_manual_torque = value;
}

float obs_source_transition_seam_get_manual_clamp(const obs_source_t *transition)
{
	return transition->transition_manual_clamp;
}

void obs_source_transition_seam_set_manual_clamp(obs_source_t *transition, float value)
{
	transition->transition_manual_clamp = value;
}

uint64_t obs_source_transition_seam_get_start_time(const obs_source_t *transition)
{
	return transition->transition_start_time;
}

void obs_source_transition_seam_set_start_time(obs_source_t *transition, uint64_t value)
{
	transition->transition_start_time = value;
}

uint64_t obs_source_transition_seam_get_duration(const obs_source_t *transition)
{
	return transition->transition_duration;
}

void obs_source_transition_seam_set_duration(obs_source_t *transition, uint64_t value)
{
	transition->transition_duration = value;
}

uint32_t obs_source_transition_seam_get_cx(const obs_source_t *transition)
{
	return transition->transition_cx;
}

void obs_source_transition_seam_set_cx(obs_source_t *transition, uint32_t value)
{
	transition->transition_cx = value;
}

uint32_t obs_source_transition_seam_get_cy(const obs_source_t *transition)
{
	return transition->transition_cy;
}

void obs_source_transition_seam_set_cy(obs_source_t *transition, uint32_t value)
{
	transition->transition_cy = value;
}

uint32_t obs_source_transition_seam_get_actual_cx(const obs_source_t *transition)
{
	return transition->transition_actual_cx;
}

void obs_source_transition_seam_set_actual_cx(obs_source_t *transition, uint32_t value)
{
	transition->transition_actual_cx = value;
}

uint32_t obs_source_transition_seam_get_actual_cy(const obs_source_t *transition)
{
	return transition->transition_actual_cy;
}

void obs_source_transition_seam_set_actual_cy(obs_source_t *transition, uint32_t value)
{
	transition->transition_actual_cy = value;
}

uint32_t obs_source_transition_seam_get_alignment(const obs_source_t *transition)
{
	return transition->transition_alignment;
}

void obs_source_transition_seam_set_alignment(obs_source_t *transition, uint32_t value)
{
	transition->transition_alignment = value;
}

enum obs_transition_scale_type obs_source_transition_seam_get_scale_type(const obs_source_t *transition)
{
	return transition->transition_scale_type;
}

void obs_source_transition_seam_set_scale_type(obs_source_t *transition, enum obs_transition_scale_type value)
{
	transition->transition_scale_type = value;
}

bool obs_source_transition_seam_get_use_fixed_duration(const obs_source_t *transition)
{
	return transition->transition_use_fixed_duration;
}

void obs_source_transition_seam_set_use_fixed_duration(obs_source_t *transition, bool value)
{
	transition->transition_use_fixed_duration = value;
}

uint32_t obs_source_transition_seam_get_fixed_duration(const obs_source_t *transition)
{
	return transition->transition_fixed_duration;
}

void obs_source_transition_seam_set_fixed_duration(obs_source_t *transition, uint32_t value)
{
	transition->transition_fixed_duration = value;
}

gs_texrender_t *obs_source_transition_seam_get_texrender(const obs_source_t *transition, size_t idx)
{
	return transition->transition_texrender[idx];
}

void obs_source_transition_seam_set_texrender(obs_source_t *transition, size_t idx, gs_texrender_t *value)
{
	transition->transition_texrender[idx] = value;
}

void obs_source_transition_seam_get_matrix(const obs_source_t *transition, size_t idx, struct matrix4 *out)
{
	matrix4_copy(out, &transition->transition_matrices[idx]);
}

void obs_source_transition_seam_set_matrix(obs_source_t *transition, size_t idx, const struct matrix4 *value)
{
	matrix4_copy(&transition->transition_matrices[idx], value);
}

int obs_source_transition_seam_lock(obs_source_t *transition)
{
	return pthread_mutex_lock(&transition->transition_mutex);
}

int obs_source_transition_seam_unlock(obs_source_t *transition)
{
	return pthread_mutex_unlock(&transition->transition_mutex);
}

int obs_source_transition_seam_trylock(obs_source_t *transition)
{
	return pthread_mutex_trylock(&transition->transition_mutex);
}

int obs_source_transition_seam_lock_textures(obs_source_t *transition)
{
	return pthread_mutex_lock(&transition->transition_tex_mutex);
}

int obs_source_transition_seam_unlock_textures(obs_source_t *transition)
{
	return pthread_mutex_unlock(&transition->transition_tex_mutex);
}

int obs_source_transition_seam_trylock_textures(obs_source_t *transition)
{
	return pthread_mutex_trylock(&transition->transition_tex_mutex);
}
