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

void obs_sceneitem_transition_load(struct obs_scene_item *item, obs_data_t *data, bool show)
{
	if (!item || !data)
		return;
	const char *id = obs_data_get_string(data, "id");
	if (id && strlen(id)) {
		const char *tn = obs_data_get_string(data, "name");
		obs_data_t *s = obs_data_get_obj(data, "transition");
		obs_source_t *t = obs_source_create_private(id, tn, s);
		obs_sceneitem_set_transition(item, show, t);
		obs_source_release(t);
		obs_data_release(s);
	} else {
		obs_sceneitem_set_transition(item, show, NULL);
	}
	obs_sceneitem_set_transition_duration(item, show, (uint32_t)obs_data_get_int(data, "duration"));
}

obs_data_t *obs_sceneitem_transition_save(struct obs_scene_item *item, bool show)
{
	obs_data_t *data = obs_data_create();
	struct obs_source *transition = show ? item->show_transition : item->hide_transition;
	if (transition) {
		obs_data_set_string(data, "id", obs_source_get_unversioned_id(transition));
		obs_data_set_string(data, "versioned_id", obs_source_get_id(transition));
		obs_data_set_string(data, "name", obs_source_get_name(transition));
		obs_data_t *s = obs_source_get_settings(transition);
		obs_data_set_obj(data, "transition", s);
		obs_data_release(s);
	}
	obs_data_set_int(data, "duration", show ? item->show_transition_duration : item->hide_transition_duration);
	return data;
}
