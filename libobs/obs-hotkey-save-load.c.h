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

#include "libobs/build/libobs-ported-hotkey-save-load.pch.h"

static inline void create_binding(obs_hotkey_t *hotkey, obs_key_combination_t combo)
{
	if (!obs_hotkey_combination_bindable(combo))
		return;

	obs_hotkey_seam_add_binding(hotkey, combo);
}

/* Until #2635 the X11 and Wayland key tables gave Volume Up the name
 * OBS_KEY_VK_VOLUME_DOWN and Volume Down OBS_KEY_VK_VOLUME_UP. A binding to
 * either that libobs saves for a scene collection (a source's, encoder's,
 * output's or service's hotkey data) has this set since then; one loaded
 * from there without it, on such a host (saved_volume_keys_swapped), names
 * the other key and is swapped once. obs_hotkey_load() and obs_hotkey_save()
 * are left alone: their callers choose what the data is, and the frontend
 * migrates the hotkeys it saves itself. */
#define VOLUME_KEYS_FIXED "volume_keys_fixed"

static inline bool is_volume_up_down(obs_key_t key)
{
	return key == OBS_KEY_VK_VOLUME_UP || key == OBS_KEY_VK_VOLUME_DOWN;
}

static inline void load_binding(obs_hotkey_t *hotkey, obs_data_t *data, bool saved)
{
	if (!hotkey || !data)
		return;

	obs_key_combination_t combo;
	if (!obs_hotkey_combination_from_data(data, &combo))
		return;

	if (saved && is_volume_up_down(combo.key) && !obs_data_get_bool(data, VOLUME_KEYS_FIXED) &&
	    obs_hotkey_seam_saved_volume_keys_swapped())
		combo.key = combo.key == OBS_KEY_VK_VOLUME_UP ? OBS_KEY_VK_VOLUME_DOWN : OBS_KEY_VK_VOLUME_UP;

	create_binding(hotkey, combo);
}

/* removed: the caller's remove_bindings() removed bindings of this hotkey
 * first, so an empty or missing array still changed them (as in
 * obs_hotkey_load_bindings()). saved: the data is a scene collection's
 * (VOLUME_KEYS_FIXED). */
void load_bindings(obs_hotkey_t *hotkey, obs_data_array_t *data, bool removed, bool saved)
{
	const size_t count = obs_data_array_count(data);
	for (size_t i = 0; i < count; i++) {
		obs_data_t *item = obs_data_array_item(data, i);
		load_binding(hotkey, item, saved);
		obs_data_release(item);
	}

	if (count || removed)
		obs_hotkey_seam_signal("hotkey_bindings_changed", hotkey);
}

void obs_hotkey_load_bindings(obs_hotkey_id id, obs_key_combination_t *combinations, size_t num)
{
	if (!obs_hotkey_seam_lock())
		return;

	obs_hotkey_t *hotkey = obs_hotkey_seam_find(id);
	if (hotkey) {
		bool changed = obs_hotkey_seam_remove_bindings(id);
		for (size_t i = 0; i < num; i++)
			create_binding(hotkey, combinations[i]);

		if (num || changed)
			obs_hotkey_seam_signal("hotkey_bindings_changed", hotkey);
	}

	obs_hotkey_seam_unlock();
}

void obs_hotkey_load(obs_hotkey_id id, obs_data_array_t *data)
{
	if (!obs_hotkey_seam_lock())
		return;

	obs_hotkey_t *hotkey = obs_hotkey_seam_find(id);
	if (hotkey) {
		bool removed = obs_hotkey_seam_remove_bindings(id);
		load_bindings(hotkey, data, removed, false);
	}
	obs_hotkey_seam_unlock();
}

static inline bool enum_load_bindings(void *data, obs_hotkey_t *hotkey)
{
	obs_data_array_t *hotkey_data = obs_data_get_array(data, obs_hotkey_get_name(hotkey));
	if (!hotkey_data)
		return true;

	load_bindings(hotkey, hotkey_data, false, true);
	obs_data_array_release(hotkey_data);
	return true;
}

void obs_hotkeys_load_encoder(obs_encoder_t *encoder, obs_data_t *hotkeys)
{
	if (!encoder || !hotkeys)
		return;
	if (!obs_hotkey_seam_lock())
		return;

	obs_hotkey_seam_enum_context(obs_hotkey_seam_encoder_context(encoder), enum_load_bindings, hotkeys);
	obs_hotkey_seam_unlock();
}

void obs_hotkeys_load_output(obs_output_t *output, obs_data_t *hotkeys)
{
	if (!output || !hotkeys)
		return;
	if (!obs_hotkey_seam_lock())
		return;

	obs_hotkey_seam_enum_context(obs_hotkey_seam_output_context(output), enum_load_bindings, hotkeys);
	obs_hotkey_seam_unlock();
}

void obs_hotkeys_load_service(obs_service_t *service, obs_data_t *hotkeys)
{
	if (!service || !hotkeys)
		return;
	if (!obs_hotkey_seam_lock())
		return;

	obs_hotkey_seam_enum_context(obs_hotkey_seam_service_context(service), enum_load_bindings, hotkeys);
	obs_hotkey_seam_unlock();
}

void obs_hotkeys_load_source(obs_source_t *source, obs_data_t *hotkeys)
{
	if (!source || !hotkeys)
		return;
	if (!obs_hotkey_seam_lock())
		return;

	obs_hotkey_seam_enum_context(obs_hotkey_seam_source_context(source), enum_load_bindings, hotkeys);
	obs_hotkey_seam_unlock();
}

void obs_hotkey_pair_load(obs_hotkey_pair_id id, obs_data_array_t *data0, obs_data_array_t *data1)
{
	if ((!data0 && !data1) || !obs_hotkey_seam_lock())
		return;

	obs_hotkey_pair_t *pair = obs_hotkey_seam_find_pair(id);
	if (!pair)
		goto unlock;

	obs_hotkey_t *p1 = obs_hotkey_seam_find(obs_hotkey_seam_pair_id(pair, 0));
	obs_hotkey_t *p2 = obs_hotkey_seam_find(obs_hotkey_seam_pair_id(pair, 1));

	if (p1) {
		bool removed = obs_hotkey_seam_remove_bindings(obs_hotkey_seam_pair_id(pair, 0));
		load_bindings(p1, data0, removed, false);
	}
	if (p2) {
		bool removed = obs_hotkey_seam_remove_bindings(obs_hotkey_seam_pair_id(pair, 1));
		load_bindings(p2, data1, removed, false);
	}

unlock:
	obs_hotkey_seam_unlock();
}

struct save_bindings_helper_t {
	obs_data_array_t *array;
	obs_hotkey_t *hotkey;
	/* For a scene collection (VOLUME_KEYS_FIXED). */
	bool saved;
};

static inline bool save_bindings_helper(void *data, size_t idx, obs_hotkey_binding_t *binding)
{
	UNUSED_PARAMETER(idx);
	struct save_bindings_helper_t *h = data;

	if (obs_hotkey_get_id(h->hotkey) != obs_hotkey_binding_get_hotkey_id(binding))
		return true;

	obs_data_t *hotkey = obs_data_create();

	obs_key_combination_t combo = obs_hotkey_binding_get_key_combination(binding);
	obs_hotkey_combination_to_data(combo, hotkey);
	if (h->saved && is_volume_up_down(combo.key))
		obs_data_set_bool(hotkey, VOLUME_KEYS_FIXED, true);

	obs_data_array_push_back(h->array, hotkey);

	obs_data_release(hotkey);

	return true;
}

static inline obs_data_array_t *save_hotkey(obs_hotkey_t *hotkey, bool saved)
{
	obs_data_array_t *data = obs_data_array_create();

	struct save_bindings_helper_t arg = {data, hotkey, saved};
	obs_hotkey_seam_enum_bindings(save_bindings_helper, &arg);

	return data;
}

obs_data_array_t *obs_hotkey_save(obs_hotkey_id id)
{
	obs_data_array_t *result = NULL;

	if (!obs_hotkey_seam_lock())
		return result;

	obs_hotkey_t *hotkey = obs_hotkey_seam_find(id);
	if (hotkey)
		result = save_hotkey(hotkey, false);

	obs_hotkey_seam_unlock();

	return result;
}

void obs_hotkey_pair_save(obs_hotkey_pair_id id, obs_data_array_t **p_data0, obs_data_array_t **p_data1)
{
	if ((!p_data0 && !p_data1) || !obs_hotkey_seam_lock())
		return;

	obs_hotkey_pair_t *pair = obs_hotkey_seam_find_pair(id);
	if (!pair)
		goto unlock;

	obs_hotkey_t *hotkey;
	if (p_data0) {
		hotkey = obs_hotkey_seam_find(obs_hotkey_seam_pair_id(pair, 0));
		if (hotkey)
			*p_data0 = save_hotkey(hotkey, false);
	}
	if (p_data1) {
		hotkey = obs_hotkey_seam_find(obs_hotkey_seam_pair_id(pair, 1));
		if (hotkey)
			*p_data1 = save_hotkey(hotkey, false);
	}

unlock:
	obs_hotkey_seam_unlock();
}

static inline bool enum_save_hotkey(void *data, obs_hotkey_t *hotkey)
{
	obs_data_array_t *hotkey_data = save_hotkey(hotkey, true);
	obs_data_set_array(data, obs_hotkey_get_name(hotkey), hotkey_data);
	obs_data_array_release(hotkey_data);
	return true;
}

static inline obs_data_t *save_context_hotkeys(struct obs_context_data *context)
{
	if (!obs_hotkey_seam_context_count(context))
		return NULL;

	obs_data_t *result = obs_data_create();
	obs_hotkey_seam_enum_context(context, enum_save_hotkey, result);
	return result;
}

obs_data_t *obs_hotkeys_save_encoder(obs_encoder_t *encoder)
{
	obs_data_t *result = NULL;

	if (!obs_hotkey_seam_lock())
		return result;

	result = save_context_hotkeys(obs_hotkey_seam_encoder_context(encoder));
	obs_hotkey_seam_unlock();

	return result;
}

obs_data_t *obs_hotkeys_save_output(obs_output_t *output)
{
	obs_data_t *result = NULL;

	if (!obs_hotkey_seam_lock())
		return result;

	result = save_context_hotkeys(obs_hotkey_seam_output_context(output));
	obs_hotkey_seam_unlock();

	return result;
}

obs_data_t *obs_hotkeys_save_service(obs_service_t *service)
{
	obs_data_t *result = NULL;

	if (!obs_hotkey_seam_lock())
		return result;

	result = save_context_hotkeys(obs_hotkey_seam_service_context(service));
	obs_hotkey_seam_unlock();

	return result;
}

obs_data_t *obs_hotkeys_save_source(obs_source_t *source)
{
	obs_data_t *result = NULL;

	if (!obs_hotkey_seam_lock())
		return result;

	result = save_context_hotkeys(obs_hotkey_seam_source_context(source));
	obs_hotkey_seam_unlock();

	return result;
}

#undef VOLUME_KEYS_FIXED
