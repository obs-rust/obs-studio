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

/* The X11 keysym table and mouse button answers of obs-nix-x11.c, on plain
 * values (obs-nix-x11-input.c):
 * - obs_nix_x11_input_keysym(): get_keysym();
 * - obs_nix_x11_input_mouse_pressed(): mouse_button_pressed()'s answer with
 *   XInput;
 * - obs_nix_x11_input_commit(): its commit of this poll's raw button events;
 * - obs_nix_x11_input_mask_pressed(): its answer without XInput.
 *
 * obs-nix-x11.c keeps the base keysym fill loop, the raw event drain (every
 * event read and store, each free), the two memsets, xcb_query_pointer() with
 * its reply, error and frees, and every log. Nothing here calls into X,
 * allocates, logs or keeps state. Without EXPORT, so hidden in libobs.
 *
 * The three per-button arrays (pressed in this poll, updated in this poll,
 * held from earlier polls) each have XINPUT_MOUSE_LEN entries; they cross by
 * pointer and are read and written here as obs-nix-x11.c did.
 *
 * The keysyms are their values: this file includes no X header.
 * obs-nix-x11.c checks the pointer button masks below against the X headers. */

/* XCB_BUTTON_MASK_1, XCB_BUTTON_MASK_2 and XCB_BUTTON_MASK_3 (xcb/xproto.h). */
#define OBS_NIX_X11_INPUT_BUTTON_MASK_1 0x100
#define OBS_NIX_X11_INPUT_BUTTON_MASK_2 0x200
#define OBS_NIX_X11_INPUT_BUTTON_MASK_3 0x400

/* The X keysym of key, 0 for a key with none; mouse buttons 1 to 5 are their
 * number shifted left by 16. */
int obs_nix_x11_input_keysym(obs_key_t key);

/* Whether the mouse key is down with XInput: its button pressed in this poll
 * or held from an earlier one; false for any other key. held is read only
 * when the button wasn't pressed in this poll. */
bool obs_nix_x11_input_mouse_pressed(obs_key_t key, const bool *pressed, const bool *held);

/* Each button updated in this poll is held when it was pressed in this poll,
 * and released otherwise. */
void obs_nix_x11_input_commit(const bool *update, const bool *pressed, bool *held);

/* Whether the mouse key is down in a QueryPointer reply's button mask, for
 * mouse buttons 1 to 3; false for any other key. */
bool obs_nix_x11_input_mask_pressed(obs_key_t key, uint16_t buttons);
