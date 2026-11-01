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

/* The Wayland key names of obs-nix-wayland.c, on plain values
 * (obs-nix-wayland-key-names.c):
 * - obs_nix_wayland_key_names_route(): obs_nix_wayland_key_to_str()'s choice
 *   of how to name a key, with the default names of the fixed keys.
 *
 * obs-nix-wayland.c keeps the translation reads,
 * obs_get_hotkey_translation(), the keymap reads, xkb_keysym_to_utf8(),
 * obs_key_to_name() and every dstr call. Nothing here calls into xkbcommon or
 * Wayland, allocates, logs or keeps state. Without EXPORT, so hidden in
 * libobs. This file includes no xkbcommon or Wayland header. */

/* How obs_nix_wayland_key_to_str() names a key. The values are those of
 * enum obs_nix_x11_hotkeys_route, with KEYSYM in the place of KEYCODES. */
enum obs_nix_wayland_key_names_route {
	/* Nothing: the name is left as it is. Not returned by this C. */
	OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_NONE,
	/* Mouse button *number: its translation, else "Mouse N". */
	OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_MOUSE,
	/* Numpad digit *number: its translation, else "Numpad N". */
	OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_NUMPAD,
	/* Its translation, with *name as the default. */
	OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_FIXED,
	/* Function key *number: "FN", with no translation. */
	OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_FUNCTION,
	/* The UTF-8 of the key's level-0 keysym in the keymap, else its key
	 * name. */
	OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_KEYSYM,
};

/* How to name key. Sets *number only for the mouse, numpad and function
 * routes, and *name (a static string) only for the fixed one. */
enum obs_nix_wayland_key_names_route obs_nix_wayland_key_names_route(obs_key_t key, int *number, const char **name);
