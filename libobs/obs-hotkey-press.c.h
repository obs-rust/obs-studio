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

/* The hotkey press state machine (moved verbatim from obs-hotkey.c.h): when
 * one binding is pressed or released. The hotkey thread calls
 * obs_hotkey_binding_query() for every binding on each 25 ms poll, with the
 * modifiers from obs_hotkeys_query_modifiers(), and obs_hotkey_inject_event()
 * calls obs_hotkey_binding_inject() for every binding. Both run with the
 * hotkey mutex held. The bindings array and its walk (enum_bindings()), the
 * pressed count and callbacks (obs_hotkey_binding_press() and
 * obs_hotkey_binding_release()) and the platform key state
 * (obs_hotkey_key_is_pressed()) stay in obs-hotkey.c.h. Declared in
 * obs-internal.h. */

static inline bool modifiers_match(obs_hotkey_binding_t *binding, uint32_t modifiers_, bool strict_modifiers)
{
	uint32_t modifiers = binding->key.modifiers;
	if (!strict_modifiers)
		return (modifiers & modifiers_) == modifiers;
	else
		return modifiers == modifiers_;
}

static inline void handle_binding(obs_hotkey_binding_t *binding, uint32_t modifiers, bool no_press,
				  bool strict_modifiers, bool *pressed)
{
	bool modifiers_match_ = modifiers_match(binding, modifiers, strict_modifiers);
	bool modifiers_only = binding->key.key == OBS_KEY_NONE;

	if (!strict_modifiers && !binding->key.modifiers)
		binding->modifiers_match = true;

	if (modifiers_only)
		pressed = &modifiers_only;

	if (!binding->key.modifiers && modifiers_only)
		goto reset;

	if ((!binding->modifiers_match && !modifiers_only) || !modifiers_match_)
		goto reset;

	if ((pressed && !*pressed) || (!pressed && !obs_hotkey_key_is_pressed(binding->key.key)))
		goto reset;

	if (binding->pressed || no_press)
		return;

	obs_hotkey_binding_press(binding);
	return;

reset:
	binding->modifiers_match = modifiers_match_;
	if (!binding->pressed)
		return;

	obs_hotkey_binding_release(binding);
}

/* The decision of inject_hotkey(). */
void obs_hotkey_binding_inject(obs_hotkey_binding_t *binding, obs_key_combination_t hotkey, bool pressed_,
			       bool strict_modifiers)
{
	if (modifiers_match(binding, hotkey.modifiers, strict_modifiers)) {
		bool pressed = binding->key.key == hotkey.key && pressed_;
		if (binding->key.key == OBS_KEY_NONE)
			pressed = true;

		if (pressed) {
			binding->modifiers_match = true;
			if (!binding->pressed)
				obs_hotkey_binding_press(binding);
		}
	}
}

/* The body of query_hotkey(). */
void obs_hotkey_binding_query(obs_hotkey_binding_t *binding, uint32_t modifiers, bool no_press, bool strict_modifiers)
{
	handle_binding(binding, modifiers, no_press, strict_modifiers, NULL);
}

/* The modifier gathering of query_hotkeys(). */
uint32_t obs_hotkeys_query_modifiers(void)
{
	uint32_t modifiers = 0;
	if (obs_hotkey_key_is_pressed(OBS_KEY_SHIFT))
		modifiers |= INTERACT_SHIFT_KEY;
	if (obs_hotkey_key_is_pressed(OBS_KEY_CONTROL))
		modifiers |= INTERACT_CONTROL_KEY;
	if (obs_hotkey_key_is_pressed(OBS_KEY_ALT))
		modifiers |= INTERACT_ALT_KEY;
	if (obs_hotkey_key_is_pressed(OBS_KEY_META))
		modifiers |= INTERACT_COMMAND_KEY;
	return modifiers;
}
