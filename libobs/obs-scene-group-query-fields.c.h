#include "libobs/build/libobs.pch.h"

const bool *obs_scene_group_query_fields(const struct obs_scene *scene)
{
	return &scene->is_group;
}

const bool *obs_sceneitem_group_query_fields(const struct obs_scene_item *item)
{
	return &item->is_group;
}
