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

#include <stdint.h>

/* The X11 key names and raw button decode of obs-nix-x11.c, on plain values
 * (obs-nix-x11-hotkeys.c):
 * - obs_nix_x11_hotkeys_route(): obs_nix_x11_key_to_str()'s choice of how
 *   to name a key, with the default names of the fixed keys;
 * - obs_nix_x11_hotkeys_raw_button(): mouse_button_pressed()'s decode of
 *   one polled event with XInput.
 *
 * obs-nix-x11.c keeps the translation reads, obs_get_hotkey_translation(),
 * the keycode lookups with XLookupString(), obs_key_to_name(), every dstr
 * call, xcb_poll_for_event() with each free, the two memsets, the pressed and
 * update stores and every log. Nothing here calls into X, allocates, logs or
 * keeps state. Without EXPORT, so hidden in libobs.
 *
 * A field of the event that the original read only under a condition is
 * passed by pointer and read here under that same condition.
 *
 * The event codes are their values: this file includes no X header.
 * obs-nix-x11.c checks them against the X headers. */

/* XCB_GE_GENERIC (xcb/xproto.h). */
#define OBS_NIX_X11_HOTKEYS_GE_GENERIC 35
/* XCB_INPUT_RAW_BUTTON_PRESS and XCB_INPUT_RAW_BUTTON_RELEASE
 * (xcb/xinput.h). */
#define OBS_NIX_X11_HOTKEYS_RAW_BUTTON_PRESS 15
#define OBS_NIX_X11_HOTKEYS_RAW_BUTTON_RELEASE 16

/* How obs_nix_x11_key_to_str() names a key. */
enum obs_nix_x11_hotkeys_route {
	/* Nothing: the name is left as it is. Not returned by this C. */
	OBS_NIX_X11_HOTKEYS_ROUTE_NONE,
	/* Mouse button *number: its translation, else "Mouse N". */
	OBS_NIX_X11_HOTKEYS_ROUTE_MOUSE,
	/* Numpad digit *number: its translation, else "Numpad N". */
	OBS_NIX_X11_HOTKEYS_ROUTE_NUMPAD,
	/* Its translation, with *name as the default. */
	OBS_NIX_X11_HOTKEYS_ROUTE_FIXED,
	/* Function key *number: "FN", with no translation. */
	OBS_NIX_X11_HOTKEYS_ROUTE_FUNCTION,
	/* The first keycode of the key that X names, else its key name. */
	OBS_NIX_X11_HOTKEYS_ROUTE_KEYCODES,
};

/* What mouse_button_pressed() does with one polled event. */
enum obs_nix_x11_hotkeys_button {
	/* Nothing. */
	OBS_NIX_X11_HOTKEYS_BUTTON_IGNORE,
	/* Button slot *slot is pressed and updated in this poll. */
	OBS_NIX_X11_HOTKEYS_BUTTON_PRESS,
	/* Button slot *slot is updated in this poll. */
	OBS_NIX_X11_HOTKEYS_BUTTON_RELEASE,
	/* A raw button event for a button past the arrays: logged. */
	OBS_NIX_X11_HOTKEYS_BUTTON_UNSUPPORTED,
};

/* How to name key. Sets *number only for the mouse, numpad and function
 * routes, and *name (a static string) only for the fixed one. */
enum obs_nix_x11_hotkeys_route obs_nix_x11_hotkeys_route(obs_key_t key, int *number, const char **name);

/* The decode of a polled event whose response type is response_type: a raw
 * button press or release when it is a generic event with that XInput event
 * type (*event_type, read only for a generic event) and a button (*detail,
 * read only for those two) below XINPUT_MOUSE_LEN, at slot *detail - 1.
 * Sets *slot only for a press or a release. */
enum obs_nix_x11_hotkeys_button obs_nix_x11_hotkeys_raw_button(uint8_t response_type, const uint16_t *event_type,
							       const uint32_t *detail, uint32_t *slot);
