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

#pragma once

#include "obs-internal.h"

/* Private adapter, not a new public API: the seam between the transition
 * code and the transition fields of struct obs_source
 * (obs-source-transition-seam.c).
 *
 * - transition_valid(), transition_active() and valid_scale_type(): the
 *   checks every transition file of libobs.core-source asks, moved from
 *   obs-source-transition.c unchanged.
 * - One get and one set per transition field of struct obs_source, each a
 *   single read or write of the field (idx is the slot, 0 or 1, of the
 *   two-slot fields), and lock, unlock and trylock of transition_mutex and
 *   transition_tex_mutex, each a single pthread_mutex_*() call returning its
 *   result. None checks its arguments or takes a lock of its own: a caller
 *   takes the same locks around them as the code that reads the field today.
 *
 * Hidden in libobs: not EXPORT. */

bool transition_valid(const obs_source_t *transition, const char *func);
bool transition_active(obs_source_t *transition);
enum obs_transition_scale_type valid_scale_type(int64_t type);

obs_source_t *obs_source_transition_seam_get_sources(const obs_source_t *transition, size_t idx);
void obs_source_transition_seam_set_sources(obs_source_t *transition, size_t idx, obs_source_t *value);
bool obs_source_transition_seam_get_source_active(const obs_source_t *transition, size_t idx);
void obs_source_transition_seam_set_source_active(obs_source_t *transition, size_t idx, bool value);
bool obs_source_transition_seam_get_transitioning_video(const obs_source_t *transition);
void obs_source_transition_seam_set_transitioning_video(obs_source_t *transition, bool value);
bool obs_source_transition_seam_get_transitioning_audio(const obs_source_t *transition);
void obs_source_transition_seam_set_transitioning_audio(obs_source_t *transition, bool value);
enum obs_transition_mode obs_source_transition_seam_get_mode(const obs_source_t *transition);
void obs_source_transition_seam_set_mode(obs_source_t *transition, enum obs_transition_mode value);
float obs_source_transition_seam_get_manual_val(const obs_source_t *transition);
void obs_source_transition_seam_set_manual_val(obs_source_t *transition, float value);
float obs_source_transition_seam_get_manual_target(const obs_source_t *transition);
void obs_source_transition_seam_set_manual_target(obs_source_t *transition, float value);
float obs_source_transition_seam_get_manual_torque(const obs_source_t *transition);
void obs_source_transition_seam_set_manual_torque(obs_source_t *transition, float value);
float obs_source_transition_seam_get_manual_clamp(const obs_source_t *transition);
void obs_source_transition_seam_set_manual_clamp(obs_source_t *transition, float value);
uint64_t obs_source_transition_seam_get_start_time(const obs_source_t *transition);
void obs_source_transition_seam_set_start_time(obs_source_t *transition, uint64_t value);
uint64_t obs_source_transition_seam_get_duration(const obs_source_t *transition);
void obs_source_transition_seam_set_duration(obs_source_t *transition, uint64_t value);
uint32_t obs_source_transition_seam_get_cx(const obs_source_t *transition);
void obs_source_transition_seam_set_cx(obs_source_t *transition, uint32_t value);
uint32_t obs_source_transition_seam_get_cy(const obs_source_t *transition);
void obs_source_transition_seam_set_cy(obs_source_t *transition, uint32_t value);
uint32_t obs_source_transition_seam_get_actual_cx(const obs_source_t *transition);
void obs_source_transition_seam_set_actual_cx(obs_source_t *transition, uint32_t value);
uint32_t obs_source_transition_seam_get_actual_cy(const obs_source_t *transition);
void obs_source_transition_seam_set_actual_cy(obs_source_t *transition, uint32_t value);
uint32_t obs_source_transition_seam_get_alignment(const obs_source_t *transition);
void obs_source_transition_seam_set_alignment(obs_source_t *transition, uint32_t value);
enum obs_transition_scale_type obs_source_transition_seam_get_scale_type(const obs_source_t *transition);
void obs_source_transition_seam_set_scale_type(obs_source_t *transition, enum obs_transition_scale_type value);
bool obs_source_transition_seam_get_use_fixed_duration(const obs_source_t *transition);
void obs_source_transition_seam_set_use_fixed_duration(obs_source_t *transition, bool value);
uint32_t obs_source_transition_seam_get_fixed_duration(const obs_source_t *transition);
void obs_source_transition_seam_set_fixed_duration(obs_source_t *transition, uint32_t value);
gs_texrender_t *obs_source_transition_seam_get_texrender(const obs_source_t *transition, size_t idx);
void obs_source_transition_seam_set_texrender(obs_source_t *transition, size_t idx, gs_texrender_t *value);
void obs_source_transition_seam_get_matrix(const obs_source_t *transition, size_t idx, struct matrix4 *out);
void obs_source_transition_seam_set_matrix(obs_source_t *transition, size_t idx, const struct matrix4 *value);
int obs_source_transition_seam_lock(obs_source_t *transition);
int obs_source_transition_seam_unlock(obs_source_t *transition);
int obs_source_transition_seam_trylock(obs_source_t *transition);
int obs_source_transition_seam_lock_textures(obs_source_t *transition);
int obs_source_transition_seam_unlock_textures(obs_source_t *transition);
int obs_source_transition_seam_trylock_textures(obs_source_t *transition);
