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

/* The hotkey binding press count: when one binding is pressed or released.
 * A hotkey can have several bindings, and each is pressed and released on its
 * own: by the hotkey thread on each 25 ms poll, by obs_hotkey_inject_event()
 * and by remove_bindings() (obs-hotkey.c) when a pressed binding is rebound
 * or its hotkey unregistered. The hotkey's pressed count turns those binding
 * events into hotkey events: its callback runs on the first press and the
 * last release only, so a hotkey held through two bindings fires once.
 *
 * The binding events come from the OS key state, the frontend and
 * obs-websocket, and this file runs with the hotkey mutex held. Declared in
 * obs-internal.h. Next to be ported to rust/obs-hotkey (binding_press). */

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
