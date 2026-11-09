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

/* Keep the extracted bodies unchanged; these dependencies remain in the core. */
#define hotkey_signal obs_hotkey_seam_signal
#define remove_bindings obs_hotkey_seam_remove_bindings
#undef HASH_FUNCTION
#define HASH_FUNCTION(s, len, hashv) (hashv) = (unsigned int)(*s % UINT_MAX)
#define HASH_FIND_HKEY(head, id, out) HASH_FIND(hh, head, &(id), sizeof(size_t), out)

static void release_registerer(obs_hotkey_t *hotkey)
{
	switch (hotkey->registerer_type) {
	case OBS_HOTKEY_REGISTERER_FRONTEND:
		break;

	case OBS_HOTKEY_REGISTERER_ENCODER:
		obs_weak_encoder_release(hotkey->registerer);
		break;

	case OBS_HOTKEY_REGISTERER_OUTPUT:
		obs_weak_output_release(hotkey->registerer);
		break;

	case OBS_HOTKEY_REGISTERER_SERVICE:
		obs_weak_service_release(hotkey->registerer);
		break;

	case OBS_HOTKEY_REGISTERER_SOURCE:
		obs_weak_source_release(hotkey->registerer);
		break;
	}

	hotkey->registerer = NULL;
}

static inline void unregister_hotkey(obs_hotkey_id id)
{
	if (id >= obs->hotkeys.next_id)
		return;

	obs_hotkey_t *hotkey;
	HASH_FIND_HKEY(obs->hotkeys.hotkeys, id, hotkey);
	if (!hotkey)
		return;

	HASH_DEL(obs->hotkeys.hotkeys, hotkey);

	hotkey_signal("hotkey_unregister", hotkey);

	release_registerer(hotkey);

	if (hotkey->registerer_type == OBS_HOTKEY_REGISTERER_SOURCE)
		obs_weak_source_release(hotkey->registerer);

	/* Before the frees: remove_bindings() releases every pressed binding it
	 * removes, and the release reads this hotkey's pressed count. */
	remove_bindings(id);

	bfree(hotkey->name);
	bfree(hotkey->description);
	bfree(hotkey);
}

/* Private entry points also serve isolated C fixtures with their own core. */
void obs_hotkey_unregister_release_registerer(obs_hotkey_t *hotkey);
void obs_hotkey_unregister_one(obs_hotkey_id id);

void obs_hotkey_unregister_release_registerer(obs_hotkey_t *hotkey)
{
	release_registerer(hotkey);
}

void obs_hotkey_unregister_one(obs_hotkey_id id)
{
	unregister_hotkey(id);
}

#undef hotkey_signal
#undef remove_bindings
#undef HASH_FIND_HKEY
#undef HASH_FUNCTION
#define HASH_FUNCTION HASH_SFH
