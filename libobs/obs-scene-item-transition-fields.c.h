#include "libobs/build/libobs.pch.h"

struct obs_scene_item_transition_fields obs_scene_item_transition_fields(const obs_sceneitem_t *item)
{
	return (struct obs_scene_item_transition_fields){&item->show_transition, &item->hide_transition,
							 &item->show_transition_duration,
							 &item->hide_transition_duration};
}
