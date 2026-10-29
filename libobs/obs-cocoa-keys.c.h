/******************************************************************************
    Copyright (C) 2013 by Ruwen Hahn <palana@stunned.de>

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

#include "libobs/build/libobs-ported-cocoa-keys.pch.h"

/* Ported to rust/obs-hotkey (cocoa_keys): the hybrid libobs drops this
 * file. */

/* The macOS key map and pressed check of obs-cocoa.m, moved: the decisions,
 * on plain values. obs-cocoa.m keeps the tables, every macOS call and the
 * key state. Declared in obs-cocoa-keys.h without EXPORT. */

/* KeyboardEventProc()'s modifier flag map. */
bool obs_cocoa_keys_modifier(uint64_t flags, obs_key_t key)
{
	switch (key) {
	case OBS_KEY_SHIFT:
		return !!(flags & OBS_COCOA_KEYS_FLAG_SHIFT);
	case OBS_KEY_ALT:
		return !!(flags & OBS_COCOA_KEYS_FLAG_ALTERNATE);
	case OBS_KEY_META:
		return !!(flags & OBS_COCOA_KEYS_FLAG_COMMAND);
	case OBS_KEY_CONTROL:
		return !!(flags & OBS_COCOA_KEYS_FLAG_CONTROL);
	default:
		return false;
	}
}

/* obs_key_to_virtual_key(). */
int obs_cocoa_keys_to_virtual_key(const obs_key_code_t *table, obs_key_t key)
{
	if (table[key].is_valid) {
		return table[key].code;
	} else {
		return OBS_COCOA_KEYS_INVALID;
	}
}

/* obs_key_from_virtual_key(). */
obs_key_t obs_cocoa_keys_from_virtual_key(const obs_key_code_t *table, int code)
{
	for (size_t i = 0; i < OBS_KEY_LAST_VALUE; i++) {
		if (table[i].is_valid && table[i].code == code) {
			return (obs_key_t)i;
		}
	}

	return OBS_KEY_NONE;
}

/* obs_hotkeys_platform_is_pressed()'s mouse range. */
bool obs_cocoa_keys_is_mouse(obs_key_t key)
{
	return key >= OBS_KEY_MOUSE1 && key <= OBS_KEY_MOUSE29;
}

/* obs_hotkeys_platform_is_pressed()'s button bit. */
bool obs_cocoa_keys_mouse_pressed(obs_key_t key, uint64_t buttons)
{
	int button = (int)key - 1;

	if ((buttons & (uint64_t)(1 << button)) != 0) {
		return true;
	} else {
		return false;
	}
}

/* obs_hotkeys_platform_is_pressed()'s key range gate. */
bool obs_cocoa_keys_in_range(obs_key_t key)
{
	return key < OBS_KEY_LAST_VALUE;
}
