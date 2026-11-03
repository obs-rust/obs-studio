/******************************************************************************
    Copyright (C) 2014-2015 by Ruwen Hahn <palana@stunned.de>

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

/* Private adapter, not a new public API: the seam between the hotkey save and
 * load functions and the hotkey registry in obs-hotkey.c. Nothing here is
 * EXPORT. The functions hold the hotkey mutex through lock and unlock, as they
 * did before the seam. */

typedef bool (*obs_hotkey_binding_internal_enum_func)(void *data, size_t idx, obs_hotkey_binding_t *binding);
typedef bool (*obs_hotkey_internal_enum_func)(void *data, obs_hotkey_t *hotkey);

bool obs_hotkey_seam_lock(void);
void obs_hotkey_seam_unlock(void);

obs_hotkey_t *obs_hotkey_seam_find(obs_hotkey_id id);
obs_hotkey_pair_t *obs_hotkey_seam_find_pair(obs_hotkey_pair_id id);
obs_hotkey_id obs_hotkey_seam_pair_id(const obs_hotkey_pair_t *pair, size_t half);

void obs_hotkey_seam_add_binding(obs_hotkey_t *hotkey, obs_key_combination_t combo);
bool obs_hotkey_seam_remove_bindings(obs_hotkey_id id);
void obs_hotkey_seam_signal(const char *signal, obs_hotkey_t *hotkey);
void obs_hotkey_seam_enum_bindings(obs_hotkey_binding_internal_enum_func func, void *data);

void obs_hotkey_seam_enum_context(struct obs_context_data *context, obs_hotkey_internal_enum_func func, void *data);
size_t obs_hotkey_seam_context_count(const struct obs_context_data *context);
bool obs_hotkey_seam_saved_volume_keys_swapped(void);

struct obs_context_data *obs_hotkey_seam_encoder_context(obs_encoder_t *encoder);
struct obs_context_data *obs_hotkey_seam_output_context(obs_output_t *output);
struct obs_context_data *obs_hotkey_seam_service_context(obs_service_t *service);
struct obs_context_data *obs_hotkey_seam_source_context(obs_source_t *source);

/* obs_hotkey_register_internal() loads the bindings a hotkey has stored. */
void load_bindings(obs_hotkey_t *hotkey, obs_data_array_t *data, bool removed, bool saved);
