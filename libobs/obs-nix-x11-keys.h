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

#pragma once

#include "obs-internal.h"

#include <stdbool.h>
#include <stdint.h>

/* The X11 key map lookups of obs-nix-x11.c, on plain values
 * (obs-nix-x11-keys.c):
 * - obs_nix_x11_keys_from_base_keysym(): key_from_base_keysym();
 * - obs_nix_x11_keys_row(): fill_keycodes()'s rule for one keycode's row;
 * - obs_nix_x11_keys_pressed(): key_pressed()'s decision;
 * - obs_nix_x11_keys_from_keycode(): key_from_keycode();
 * - obs_nix_x11_keys_from_virtual_key(): obs_nix_x11_key_from_virtual_key();
 * - obs_nix_x11_keys_to_virtual_key(): obs_nix_x11_key_to_virtual_key().
 *
 * obs-nix-x11.c keeps the keysym table (get_keysym()), the base keysym fill,
 * add_key() (its list push and log), the super keycode stores, every xcb and
 * Xlib call, the keyboard map copy and its size, the mouse path and the key
 * names. Nothing here calls into X, allocates, logs or keeps state. Without
 * EXPORT, so hidden in libobs.
 *
 * A platform field the original read only under a condition is passed by
 * pointer (or, for the table behind obs->hotkeys.platform_context, through
 * a getter) and read here under that same condition, so every read happens
 * where and when it did in obs-nix-x11.c.
 *
 * A keysym is an xcb_keysym_t (uint32_t) and a keycode an xcb_keycode_t
 * (uint8_t); obs-nix-x11.c checks both, and the keysyms below, against the
 * X headers, which this file does not include. */

/* XK_Shift_R, XK_Control_R, XK_Alt_R, XK_Super_L and XK_Super_R
 * (X11/keysymdef.h). */
#define OBS_NIX_X11_KEYS_SHIFT_R 0xffe2
#define OBS_NIX_X11_KEYS_CONTROL_R 0xffe4
#define OBS_NIX_X11_KEYS_ALT_R 0xffea
#define OBS_NIX_X11_KEYS_SUPER_L 0xffeb
#define OBS_NIX_X11_KEYS_SUPER_R 0xffec

/* The size of a QueryKeymap reply's key bitmap: one bit per keycode. */
#define OBS_NIX_X11_KEYS_KEYMAP_LEN 32

/* The keycodes that produce one key, in the order they were found. */
struct keycode_list {
	DARRAY(uint8_t) list;
};

/* What fill_keycodes() does with one keycode's row. */
enum obs_nix_x11_keys_row {
	/* No symbol of the row is wanted. */
	OBS_NIX_X11_KEYS_ROW_NONE,
	/* The keycode is the left (right) super key's. */
	OBS_NIX_X11_KEYS_ROW_SUPER_L,
	OBS_NIX_X11_KEYS_ROW_SUPER_R,
	/* The keycode produces *key. */
	OBS_NIX_X11_KEYS_ROW_KEY,
};

/* The first key whose base keysym is code; else the right Shift, Control and
 * Alt give their key; else OBS_KEY_NONE. base_keysyms has OBS_KEY_LAST_VALUE
 * entries. */
obs_key_t obs_nix_x11_keys_from_base_keysym(const uint32_t *base_keysyms, uint32_t code);

/* The first non-zero symbol of row (syms_per_code symbols) that is Super_L,
 * Super_R or has a key; a zero symbol ends the row. Sets *key only for
 * OBS_NIX_X11_KEYS_ROW_KEY. */
enum obs_nix_x11_keys_row obs_nix_x11_keys_row(const uint32_t *row, int syms_per_code, const uint32_t *base_keysyms,
					       obs_key_t *key);

/* Whether key is down in keys, a QueryKeymap reply's bitmap
 * (OBS_NIX_X11_KEYS_KEYMAP_LEN bytes; NULL for no reply: nothing is down).
 * OBS_KEY_META checks *super_l_code, then *super_r_code only when the left
 * one is not down; any other key each keycode of codes. The super keycodes
 * are read only for Meta, and codes only for any other key. */
bool obs_nix_x11_keys_pressed(const uint8_t *keys, obs_key_t key, const struct keycode_list *codes,
			      const uint8_t *super_l_code, const uint8_t *super_r_code);

/* The first key whose keycode list holds code, or OBS_KEY_NONE. keycodes has
 * OBS_KEY_LAST_VALUE entries. */
obs_key_t obs_nix_x11_keys_from_keycode(const struct keycode_list *keycodes, uint8_t code);

/* The key of keysym sym in the keyboard map keysyms (*num_keysyms symbols,
 * syms_per_code per keycode from *min_keycode on), searched one column at a
 * time across all keycodes; OBS_KEY_NONE for sym 0 or an empty map.
 * *num_keysyms is read only past that gate, *min_keycode and keycodes only
 * once the symbol is found. */
obs_key_t obs_nix_x11_keys_from_virtual_key(int sym, const uint32_t *keysyms, int syms_per_code, const int *num_keysyms,
					    const uint8_t *min_keycode, const struct keycode_list *keycodes);

/* The base keysym table, as obs_nix_x11_keys_to_virtual_key() asks for it. */
typedef const uint32_t *(*obs_nix_x11_keys_table_fn)(void);

/* XK_Super_L for OBS_KEY_META; any other key's base keysym in the table
 * base_keysyms() returns, which is asked for only then. */
int obs_nix_x11_keys_to_virtual_key(obs_key_t key, obs_nix_x11_keys_table_fn base_keysyms);
