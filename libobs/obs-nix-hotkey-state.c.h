/*
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
*/

#include "libobs/build/libobs-ported-nix-hotkey-state.pch.h"

/* Changes take obs->hotkeys.mutex so they land between two poll cycles of
 * the hotkey thread, which holds it for a whole cycle. */
static inline void hotkey_state_lock(void)
{
	obs_hotkeys_mutex_lock();
}

static inline void hotkey_state_unlock(void)
{
	obs_hotkeys_mutex_unlock();
}

void obs_nix_hotkey_state_free(struct obs_nix_hotkey_state *state)
{
	da_free(state->shortcuts);
}

void obs_nix_hotkey_state_set_key(struct obs_nix_hotkey_state *state, obs_key_t key, bool pressed)
{
	if (key <= OBS_KEY_NONE || key >= OBS_KEY_LAST_VALUE)
		return;

	hotkey_state_lock();
	state->focused_keys[key] = pressed;
	hotkey_state_unlock();
}

void obs_nix_hotkey_state_set_modifiers(struct obs_nix_hotkey_state *state, uint32_t modifiers)
{
	hotkey_state_lock();
	state->focused_modifiers = modifiers;
	hotkey_state_unlock();
}

void obs_nix_hotkey_state_clear_keys(struct obs_nix_hotkey_state *state)
{
	hotkey_state_lock();
	memset(state->focused_keys, 0, sizeof(state->focused_keys));
	state->focused_modifiers = 0;
	hotkey_state_unlock();
}

static size_t find_shortcut(struct obs_nix_hotkey_state *state, obs_key_combination_t combo)
{
	for (size_t i = 0; i < state->shortcuts.num; i++) {
		obs_key_combination_t *held = &state->shortcuts.array[i].combo;
		if (held->key == combo.key && held->modifiers == combo.modifiers)
			return i;
	}
	return DARRAY_INVALID;
}

void obs_nix_hotkey_state_set_shortcut(struct obs_nix_hotkey_state *state, obs_key_combination_t combo, bool active)
{
	hotkey_state_lock();
	size_t idx = find_shortcut(state, combo);
	if (active) {
		if (idx == DARRAY_INVALID) {
			struct obs_nix_hotkey_shortcut shortcut = {combo, false, false};
			da_push_back(state->shortcuts, &shortcut);
		} else {
			state->shortcuts.array[idx].release_pending = false;
		}
	} else if (idx != DARRAY_INVALID) {
		if (state->shortcuts.array[idx].observed)
			da_erase(state->shortcuts, idx);
		else
			state->shortcuts.array[idx].release_pending = true;
	}
	hotkey_state_unlock();
}

bool obs_nix_hotkey_state_flush(struct obs_nix_hotkey_state *state)
{
	bool pending = false;

	hotkey_state_lock();
	for (size_t i = state->shortcuts.num; i > 0; i--) {
		struct obs_nix_hotkey_shortcut *shortcut = &state->shortcuts.array[i - 1];
		if (!shortcut->release_pending)
			continue;
		if (shortcut->observed)
			da_erase(state->shortcuts, i - 1);
		else
			pending = true;
	}
	hotkey_state_unlock();
	return pending;
}

static inline uint32_t modifier_for_key(obs_key_t key)
{
	switch (key) {
	case OBS_KEY_SHIFT:
		return INTERACT_SHIFT_KEY;
	case OBS_KEY_CONTROL:
		return INTERACT_CONTROL_KEY;
	case OBS_KEY_ALT:
		return INTERACT_ALT_KEY;
	case OBS_KEY_META:
		return INTERACT_COMMAND_KEY;
	default:
		return 0;
	}
}

bool obs_nix_hotkey_state_is_pressed(struct obs_nix_hotkey_state *state, obs_key_t key)
{
	if (key <= OBS_KEY_NONE || key >= OBS_KEY_LAST_VALUE)
		return false;

	uint32_t modifier = modifier_for_key(key);
	bool pressed;

	/* The hotkey thread already holds the (recursive) hotkey_state_lock for the whole
	 * cycle, so every shortcut present now is seen by all of it. */
	hotkey_state_lock();
	pressed = modifier ? (state->focused_modifiers & modifier) != 0 : state->focused_keys[key];

	for (size_t i = 0; i < state->shortcuts.num; i++) {
		struct obs_nix_hotkey_shortcut *shortcut = &state->shortcuts.array[i];
		shortcut->observed = true;
		if (modifier)
			pressed = pressed || (shortcut->combo.modifiers & modifier) != 0;
		else
			pressed = pressed || shortcut->combo.key == key;
	}
	hotkey_state_unlock();
	return pressed;
}
