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

#include "libobs/build/libobs.pch.h"

/* The hotkey binding codec (moved verbatim from obs-hotkey.c.h): one item of
 * a hotkey's saved bindings array, as obs_hotkey_load(), obs_hotkey_save()
 * and the obs_hotkeys_load_* and obs_hotkeys_save_* functions read and write
 * it in every profile and scene collection. load_binding(), create_binding()
 * and save_bindings_helper() in obs-hotkey.c.h, later in the same unity
 * group, call it and keep the bindings array. Declared in obs-internal.h. */

static inline void load_modifier(uint32_t *modifiers, obs_data_t *data, const char *name, uint32_t flag)
{
	if (obs_data_get_bool(data, name))
		*modifiers |= flag;
}

/* The parse of load_binding(): false where it returned without a binding. */
bool obs_hotkey_combination_from_data(obs_data_t *data, obs_key_combination_t *combo_out)
{
	obs_key_combination_t combo = {0};
	uint32_t *modifiers = &combo.modifiers;
	load_modifier(modifiers, data, "shift", INTERACT_SHIFT_KEY);
	load_modifier(modifiers, data, "control", INTERACT_CONTROL_KEY);
	load_modifier(modifiers, data, "alt", INTERACT_ALT_KEY);
	load_modifier(modifiers, data, "command", INTERACT_COMMAND_KEY);

	combo.key = obs_key_from_name(obs_data_get_string(data, "key"));
	if (!modifiers && (combo.key == OBS_KEY_NONE || combo.key >= OBS_KEY_LAST_VALUE))
		return false;

	*combo_out = combo;
	return true;
}

/* The rule of create_binding(), for JSON and API bindings alike. */
bool obs_hotkey_combination_bindable(obs_key_combination_t combo)
{
	/* Match the frontend: primary mouse buttons are reserved for UI interaction. */
	if (combo.key == OBS_KEY_MOUSE1 || combo.key == OBS_KEY_MOUSE2)
		return false;

	return true;
}

static inline void save_modifier(uint32_t modifiers, obs_data_t *data, const char *name, uint32_t flag)
{
	if ((modifiers & flag) == flag)
		obs_data_set_bool(data, name, true);
}

/* The per-binding body of save_bindings_helper(). */
void obs_hotkey_combination_to_data(obs_key_combination_t combo, obs_data_t *hotkey)
{
	uint32_t modifiers = combo.modifiers;
	save_modifier(modifiers, hotkey, "shift", INTERACT_SHIFT_KEY);
	save_modifier(modifiers, hotkey, "control", INTERACT_CONTROL_KEY);
	save_modifier(modifiers, hotkey, "alt", INTERACT_ALT_KEY);
	save_modifier(modifiers, hotkey, "command", INTERACT_COMMAND_KEY);

	obs_data_set_string(hotkey, "key", obs_key_to_name(combo.key));
}
