/******************************************************************************
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

#pragma once

#include "obs-internal.h"

#include <stdbool.h>
#include <stdint.h>

/* The keysym tables and the keymap and focused-key rules of the Wayland
 * hotkey platform, on plain values (obs-nix-wayland-keys.c):
 * - obs_nix_wayland_keys_from_keysym(): obs_nix_wayland_key_from_virtual_key();
 * - obs_nix_wayland_keys_to_keysym(): obs_nix_wayland_key_to_virtual_key();
 * - obs_nix_wayland_keys_row(): load_keymap_data()'s rule for one keycode;
 * - obs_nix_wayland_keys_focused(): set_focused_key()'s rule.
 *
 * obs-nix-wayland.c keeps every xkbcommon and Wayland call (the keysyms of
 * each level come through a callback, called where load_keymap_data() called
 * xkb_keymap_key_get_syms_by_level()), the keymap mmap and compile, the
 * rebuild's memsets, the modifier mask, the seats and registry, every
 * obs_nix_hotkey_state_*() call, the key names and the vtable. Nothing here
 * calls into xkbcommon or Wayland, allocates, logs or keeps state. Without
 * EXPORT, so hidden in libobs.
 *
 * A keysym, a keycode and a level index are uint32_t (xkb_keysym_t,
 * xkb_keycode_t and xkb_level_index_t); obs-nix-wayland.c checks them, and
 * the two bounds below, against xkbcommon and its own. The tables use the
 * XKB_KEY_* names of <xkbcommon/xkbcommon-keysyms.h>, which has no code. */

/* X11 only supports 256 scancodes, and X11 keymaps only have 4 shift levels
 * (obs-nix-wayland.c's MAX_KEYCODES and MAX_SHIFT_LEVELS). */
#define OBS_NIX_WAYLAND_KEYS_MAX_KEYCODES 256
#define OBS_NIX_WAYLAND_KEYS_MAX_SHIFT_LEVELS 4

/* The obs key of an XKB keysym, OBS_KEY_NONE for one the table lacks. */
obs_key_t obs_nix_wayland_keys_from_keysym(int sym);

/* The XKB keysym of an obs key, 0 for one the table lacks. */
int obs_nix_wayland_keys_to_keysym(obs_key_t key);

/* The keysyms of one level of the keycode being loaded: their count, with
 * *syms set to them (xkb_keymap_key_get_syms_by_level()). */
typedef int (*obs_nix_wayland_keys_syms_fn)(void *data, uint32_t level, const uint32_t **syms);

/* Loads one keycode of a keymap: nothing for a keycode at or above
 * OBS_NIX_WAYLAND_KEYS_MAX_KEYCODES; else, for each of the
 * OBS_NIX_WAYLAND_KEYS_MAX_SHIFT_LEVELS levels whose level_syms() count is at
 * least 1, its first keysym goes to key_to_sym[level][keycode], and keycode
 * to obs_to_key[] at that keysym's obs key unless one is stored there (the
 * first keycode wins). level_syms() is called once per level, in order, only
 * for a keycode in range. obs_to_key has OBS_KEY_LAST_VALUE entries. */
void obs_nix_wayland_keys_row(uint32_t keycode, obs_nix_wayland_keys_syms_fn level_syms, void *data,
			      uint32_t *obs_to_key, uint32_t (*key_to_sym)[OBS_NIX_WAYLAND_KEYS_MAX_KEYCODES]);

/* Whether the evdev scancode of a focused key event (a press when pressed)
 * reaches the hotkey state, and as which key: its XKB keycode is scancode + 8
 * (a uint32_t), none at or above OBS_NIX_WAYLAND_KEYS_MAX_KEYCODES; then the
 * keycode's level-0 keysym in key_to_sym0 (OBS_NIX_WAYLAND_KEYS_MAX_KEYCODES
 * entries, read only for a keycode in range), none for 0, and *key its obs
 * key, for a press and a release alike. *key is written only when true is
 * returned.
 *
 * pressed_as (OBS_NIX_WAYLAND_KEYS_MAX_KEYCODES entries, OBS_KEY_NONE when
 * cleared) is the platform's record of the key each keycode was pressed as,
 * which obs-nix-wayland.c clears with the key state. This rule does not use
 * it, nor pressed. */
bool obs_nix_wayland_keys_focused(uint32_t scancode, bool pressed, const uint32_t *key_to_sym0, obs_key_t *pressed_as,
				  obs_key_t *key);
