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

#include "libobs/build/libobs-ported-nix-x11-hotkeys.pch.h"

/* Ported to rust/obs-hotkey (nix_x11_hotkeys): the hybrid libobs drops this
 * file. */

/* The X11 key names and raw button decode of obs-nix-x11.c, moved: the
 * decisions, on plain values. obs-nix-x11.c keeps every X call, every string
 * and the state. Declared in obs-nix-x11-hotkeys.h without EXPORT. */

#define fixed_key(def) (*name = (def), OBS_NIX_X11_HOTKEYS_ROUTE_FIXED)

/* The tests of obs_nix_x11_key_to_str(), in its order: the mouse buttons,
 * the numpad digits, the fixed keys with their default names, the function
 * keys, then the keycodes. */
enum obs_nix_x11_hotkeys_route obs_nix_x11_hotkeys_route(obs_key_t key, int *number, const char **name)
{
	if (key >= OBS_KEY_MOUSE1 && key <= OBS_KEY_MOUSE29) {
		*number = (int)(key - OBS_KEY_MOUSE1 + 1);
		return OBS_NIX_X11_HOTKEYS_ROUTE_MOUSE;
	}

	if (key >= OBS_KEY_NUM0 && key <= OBS_KEY_NUM9) {
		*number = (int)(key - OBS_KEY_NUM0);
		return OBS_NIX_X11_HOTKEYS_ROUTE_NUMPAD;
	}

	switch (key) {
	case OBS_KEY_INSERT:
		return fixed_key("Insert");
	case OBS_KEY_DELETE:
		return fixed_key("Delete");
	case OBS_KEY_HOME:
		return fixed_key("Home");
	case OBS_KEY_END:
		return fixed_key("End");
	case OBS_KEY_PAGEUP:
		return fixed_key("Page Up");
	case OBS_KEY_PAGEDOWN:
		return fixed_key("Page Down");
	case OBS_KEY_NUMLOCK:
		return fixed_key("Num Lock");
	case OBS_KEY_SCROLLLOCK:
		return fixed_key("Scroll Lock");
	case OBS_KEY_CAPSLOCK:
		return fixed_key("Caps Lock");
	case OBS_KEY_BACKSPACE:
		return fixed_key("Backspace");
	case OBS_KEY_TAB:
		return fixed_key("Tab");
	case OBS_KEY_PRINT:
		return fixed_key("Print");
	case OBS_KEY_PAUSE:
		return fixed_key("Pause");
	case OBS_KEY_LEFT:
		return fixed_key("Left");
	case OBS_KEY_RIGHT:
		return fixed_key("Right");
	case OBS_KEY_UP:
		return fixed_key("Up");
	case OBS_KEY_DOWN:
		return fixed_key("Down");
	case OBS_KEY_SHIFT:
		return fixed_key("Shift");
	case OBS_KEY_ALT:
		return fixed_key("Alt");
	case OBS_KEY_CONTROL:
		return fixed_key("Control");
	case OBS_KEY_META:
		return fixed_key("Super");
	case OBS_KEY_MENU:
		return fixed_key("Menu");
	case OBS_KEY_NUMASTERISK:
		return fixed_key("Numpad *");
	case OBS_KEY_NUMPLUS:
		return fixed_key("Numpad +");
	case OBS_KEY_NUMMINUS:
		return fixed_key("Numpad -");
	case OBS_KEY_NUMCOMMA:
		return fixed_key("Numpad ,");
	case OBS_KEY_NUMPERIOD:
		return fixed_key("Numpad .");
	case OBS_KEY_NUMSLASH:
		return fixed_key("Numpad /");
	case OBS_KEY_SPACE:
		return fixed_key("Space");
	case OBS_KEY_ESCAPE:
		return fixed_key("Escape");
	default:;
	}

	if (key >= OBS_KEY_F1 && key <= OBS_KEY_F35) {
		*number = (int)(key - OBS_KEY_F1 + 1);
		return OBS_NIX_X11_HOTKEYS_ROUTE_FUNCTION;
	}

	return OBS_NIX_X11_HOTKEYS_ROUTE_KEYCODES;
}

/* The tests of mouse_button_pressed()'s event loop with XInput, for one
 * event. */
enum obs_nix_x11_hotkeys_button obs_nix_x11_hotkeys_raw_button(uint8_t response_type, const uint16_t *event_type,
							       const uint32_t *detail, uint32_t *slot)
{
	if ((response_type & ~80) == OBS_NIX_X11_HOTKEYS_GE_GENERIC) {
		switch (*event_type) {
		case OBS_NIX_X11_HOTKEYS_RAW_BUTTON_PRESS: {
			if (*detail < XINPUT_MOUSE_LEN) {
				*slot = *detail - 1;
				return OBS_NIX_X11_HOTKEYS_BUTTON_PRESS;
			} else {
				return OBS_NIX_X11_HOTKEYS_BUTTON_UNSUPPORTED;
			}
		}
		case OBS_NIX_X11_HOTKEYS_RAW_BUTTON_RELEASE: {
			if (*detail < XINPUT_MOUSE_LEN) {
				*slot = *detail - 1;
				return OBS_NIX_X11_HOTKEYS_BUTTON_RELEASE;
			} else {
				return OBS_NIX_X11_HOTKEYS_BUTTON_UNSUPPORTED;
			}
		}
		default:
			break;
		}
	}

	return OBS_NIX_X11_HOTKEYS_BUTTON_IGNORE;
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef fixed_key
