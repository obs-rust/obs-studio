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

#include "libobs/build/libobs-ported-hotkey-pair.pch.h"

/* The hotkey pair press latch: the callbacks libobs registers for the two
 * halves of a hotkey pair. Each half keeps a latch (pressed0, pressed1). While one half is latched, the other
 * half's events are dropped. A press whose func returns true latches its
 * half, and its release then clears the latch without calling the func; any
 * other event calls the func, and the latch takes the event's pressed value
 * if the func returns true. They run with the hotkey mutex held: on the
 * hotkey thread, in obs_hotkey_inject_event(), or in
 * obs_hotkey_trigger_routed_callback(). This file reaches a pair only through
 * obs_hotkey_pair_fields() (the two latches) and calls a half's func through
 * obs_hotkey_pair_call_func(). The pair itself (create_hotkey_pair(),
 * register_hotkey_pair_internal(), the pair hash) stays in obs-hotkey.c.h.
 * Declared in obs-internal.h. Ported to rust/obs-hotkey (pair): the hybrid
 * libobs drops this file. */

void obs_hotkey_pair_first_func(void *data, obs_hotkey_id id, obs_hotkey_t *hotkey, bool pressed)
{
	UNUSED_PARAMETER(id);

	obs_hotkey_pair_t *pair = data;
	struct obs_hotkey_pair_fields fields = obs_hotkey_pair_fields(pair);
	if (*fields.pressed1)
		return;

	if (*fields.pressed0 && !pressed)
		*fields.pressed0 = false;
	else if (obs_hotkey_pair_call_func(pair, 0, hotkey, pressed))
		*fields.pressed0 = pressed;
}

void obs_hotkey_pair_second_func(void *data, obs_hotkey_id id, obs_hotkey_t *hotkey, bool pressed)
{
	UNUSED_PARAMETER(id);

	obs_hotkey_pair_t *pair = data;
	struct obs_hotkey_pair_fields fields = obs_hotkey_pair_fields(pair);
	if (*fields.pressed0)
		return;

	if (*fields.pressed1 && !pressed)
		*fields.pressed1 = false;
	else if (obs_hotkey_pair_call_func(pair, 1, hotkey, pressed))
		*fields.pressed1 = pressed;
}
