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

#include "libobs/build/libobs-ported-hotkey-binding-press.pch.h"

obs_key_combination_t obs_hotkey_binding_get_key_combination(obs_hotkey_binding_t *binding)
{
	return binding->key;
}

obs_hotkey_id obs_hotkey_binding_get_hotkey_id(obs_hotkey_binding_t *binding)
{
	return binding->hotkey_id;
}

obs_hotkey_t *obs_hotkey_binding_get_hotkey(obs_hotkey_binding_t *binding)
{
	return binding->hotkey;
}
