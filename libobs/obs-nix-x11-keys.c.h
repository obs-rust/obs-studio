/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>
    Copyright (C) 2014 by Zachary Lund <admin@computerquip.com>
    Copyright (C) 2019 by Jason Francis <cycl0ps@tuta.io>

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

#include "libobs/build/libobs-ported-nix-x11-keys.pch.h"

/* Ported to rust/obs-hotkey (nix_x11_keys): the hybrid libobs drops this
 * file. */

/* The X11 key map lookups of obs-nix-x11.c, moved: the lookups, on plain
 * values. obs-nix-x11.c keeps every X call and the state. Declared in
 * obs-nix-x11-keys.h without EXPORT. */

/* key_from_base_keysym(). */
obs_key_t obs_nix_x11_keys_from_base_keysym(const uint32_t *base_keysyms, uint32_t code)
{
	for (size_t i = 0; i < OBS_KEY_LAST_VALUE; i++) {
		if (base_keysyms[i] == (uint32_t)code) {
			return (obs_key_t)i;
		}
	}

	switch (code) {
	case OBS_NIX_X11_KEYS_SHIFT_R:
		return OBS_KEY_SHIFT;
	case OBS_NIX_X11_KEYS_CONTROL_R:
		return OBS_KEY_CONTROL;
	case OBS_NIX_X11_KEYS_ALT_R:
		return OBS_KEY_ALT;
	}

	return OBS_KEY_NONE;
}

/* The loop over one keycode's symbols in fill_keycodes(), without the stores
 * and add_key(). */
enum obs_nix_x11_keys_row obs_nix_x11_keys_row(const uint32_t *row, int syms_per_code, const uint32_t *base_keysyms,
					       obs_key_t *key)
{
	const uint32_t *sym = row;

	for (int i = 0; i < syms_per_code; i++) {
		if (!sym[i])
			break;

		if (sym[i] == OBS_NIX_X11_KEYS_SUPER_L) {
			return OBS_NIX_X11_KEYS_ROW_SUPER_L;
		} else if (sym[i] == OBS_NIX_X11_KEYS_SUPER_R) {
			return OBS_NIX_X11_KEYS_ROW_SUPER_R;
		} else {
			obs_key_t found = obs_nix_x11_keys_from_base_keysym(base_keysyms, sym[i]);

			if (found != OBS_KEY_NONE) {
				*key = found;
				return OBS_NIX_X11_KEYS_ROW_KEY;
			}
		}
	}

	return OBS_NIX_X11_KEYS_ROW_NONE;
}

/* obs_nix_x11_keycode_pressed() (obs-nix-x11-keymap.h) on the reply's
 * bitmap. */
static inline bool keycode_down(const uint8_t *keys, uint8_t code)
{
	if (!keys)
		return false;
	return (keys[code / 8] & (1 << (code % 8))) != 0;
}

/* key_pressed(), after the QueryKeymap request. */
bool obs_nix_x11_keys_pressed(const uint8_t *keys, obs_key_t key, const struct keycode_list *codes,
			      const uint8_t *super_l_code, const uint8_t *super_r_code)
{
	bool pressed = false;

	if (key == OBS_KEY_META) {
		pressed = keycode_down(keys, *super_l_code) || keycode_down(keys, *super_r_code);

	} else {
		for (size_t i = 0; i < codes->list.num; i++) {
			if (keycode_down(keys, codes->list.array[i])) {
				pressed = true;
				break;
			}
		}
	}

	return pressed;
}

/* key_from_keycode(). */
obs_key_t obs_nix_x11_keys_from_keycode(const struct keycode_list *keycodes, uint8_t code)
{
	for (size_t i = 0; i < OBS_KEY_LAST_VALUE; i++) {
		const struct keycode_list *codes = &keycodes[i];

		for (size_t j = 0; j < codes->list.num; j++) {
			if (codes->list.array[j] == code) {
				return (obs_key_t)i;
			}
		}
	}

	return OBS_KEY_NONE;
}

/* obs_nix_x11_key_from_virtual_key(), with the map passed in. */
obs_key_t obs_nix_x11_keys_from_virtual_key(int sym, const uint32_t *keysyms, int syms_per_code, const int *num_keysyms,
					    const uint8_t *min_keycode, const struct keycode_list *keycodes)
{
	if (sym == 0 || syms_per_code <= 0)
		return OBS_KEY_NONE;

	int num_codes = *num_keysyms / syms_per_code;

	/* Search one column (group and shift level) at a time across all
	 * keycodes, like XKeysymToKeycode(), so the key that produces the
	 * symbol unshifted in the first group wins over a key that only has it
	 * at another group or level (e.g. "[" on AltGr+8 in a second layout).
	 * Symbols that only exist on a shifted level still map to their key. */
	for (int col = 0; col < syms_per_code; col++) {
		for (int i = 0; i < num_codes; i++) {
			if (keysyms[i * syms_per_code + col] == (uint32_t)sym) {
				uint8_t code = (uint8_t)(i + *min_keycode);
				return obs_nix_x11_keys_from_keycode(keycodes, code);
			}
		}
	}

	return OBS_KEY_NONE;
}

/* obs_nix_x11_key_to_virtual_key(), with the base keysym table asked for
 * through a getter. */
int obs_nix_x11_keys_to_virtual_key(obs_key_t key, obs_nix_x11_keys_table_fn base_keysyms)
{
	if (key == OBS_KEY_META)
		return OBS_NIX_X11_KEYS_SUPER_L;

	return (int)base_keysyms()[(int)key];
}
