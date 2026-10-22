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

/* The hotkey binding press count: what a binding press or release does to
 * its hotkey. A press sets the binding's pressed flag and counts it in the
 * hotkey's pressed count; a release clears the flag and uncounts it. Only
 * the first press (count 0 -> 1) and the last release (1 -> 0) call the
 * hotkey: its func, or, with callback rerouting on, the router (and nothing
 * when rerouting is on and no router is set). The flag and the count are
 * written before the call, and nothing is touched after it. The hotkey press
 * state machine (obs-hotkey-press.c.h) calls obs_hotkey_binding_press() and
 * obs_hotkey_binding_release(), and remove_bindings() in obs-hotkey.c.h calls
 * the release for a pressed binding it removes, all with the hotkey mutex
 * held. The bindings array, the hotkeys and the callbacks stay in
 * obs-hotkey.c.h. Declared in obs-internal.h. */

static inline void press_released_binding(obs_hotkey_binding_t *binding)
{
	binding->pressed = true;

	obs_hotkey_t *hotkey = binding->hotkey;
	if (hotkey->pressed++)
		return;

	if (!obs->hotkeys.reroute_hotkeys)
		hotkey->func(hotkey->data, hotkey->id, hotkey, true);
	else if (obs->hotkeys.router_func)
		obs->hotkeys.router_func(obs->hotkeys.router_func_data, hotkey->id, true);
}

static inline void release_pressed_binding(obs_hotkey_binding_t *binding)
{
	binding->pressed = false;

	obs_hotkey_t *hotkey = binding->hotkey;
	if (--hotkey->pressed)
		return;

	if (!obs->hotkeys.reroute_hotkeys)
		hotkey->func(hotkey->data, hotkey->id, hotkey, false);
	else if (obs->hotkeys.router_func)
		obs->hotkeys.router_func(obs->hotkeys.router_func_data, hotkey->id, false);
}

void obs_hotkey_binding_press(obs_hotkey_binding_t *binding)
{
	press_released_binding(binding);
}

void obs_hotkey_binding_release(obs_hotkey_binding_t *binding)
{
	release_pressed_binding(binding);
}
