/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>
                          Philippe Groarke <philippe.groarke@gmail.com>

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

static obs_sceneitem_t *sceneitem_get_ref(obs_sceneitem_t *si)
{
	long owners = os_atomic_load_long(&si->ref);
	while (owners > 0) {
		if (os_atomic_compare_exchange_long(&si->ref, &owners, owners + 1)) {
			return si;
		}
	}
	return NULL;
}

static bool hotkey_show_sceneitem(void *data, obs_hotkey_pair_id id, obs_hotkey_t *hotkey, bool pressed)
{
	UNUSED_PARAMETER(id);
	UNUSED_PARAMETER(hotkey);

	obs_sceneitem_t *si = sceneitem_get_ref(data);
	if (pressed && si && !os_atomic_load_bool(&si->user_visible)) {
		obs_sceneitem_set_visible(si, true);
		obs_sceneitem_release(si);
		return true;
	}

	obs_sceneitem_release(si);
	return false;
}

static bool hotkey_hide_sceneitem(void *data, obs_hotkey_pair_id id, obs_hotkey_t *hotkey, bool pressed)
{
	UNUSED_PARAMETER(id);
	UNUSED_PARAMETER(hotkey);

	obs_sceneitem_t *si = sceneitem_get_ref(data);
	if (pressed && si && os_atomic_load_bool(&si->user_visible)) {
		obs_sceneitem_set_visible(si, false);
		obs_sceneitem_release(si);
		return true;
	}

	obs_sceneitem_release(si);
	return false;
}

static void sceneitem_rename_hotkey(const obs_sceneitem_t *scene_item, const char *new_name)
{
	struct dstr show_desc = {0};
	struct dstr hide_desc = {0};

	dstr_copy(&show_desc, obs->hotkeys.sceneitem_show);
	dstr_replace(&show_desc, "%1", new_name);
	dstr_copy(&hide_desc, obs->hotkeys.sceneitem_hide);
	dstr_replace(&hide_desc, "%1", new_name);

	obs_hotkey_pair_set_descriptions(scene_item->toggle_visibility, show_desc.array, hide_desc.array);

	dstr_free(&show_desc);
	dstr_free(&hide_desc);
}

static void sceneitem_renamed(void *param, calldata_t *data)
{
	obs_sceneitem_t *scene_item = param;
	const char *name = calldata_string(data, "new_name");

	sceneitem_rename_hotkey(scene_item, name);
}
