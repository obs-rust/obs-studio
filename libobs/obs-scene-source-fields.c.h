#include "libobs/build/libobs.pch.h"

struct obs_source *const *obs_scene_source_fields(const struct obs_scene *scene)
{
	return &scene->source;
}

struct obs_source *const *obs_sceneitem_source_fields(const struct obs_scene_item *item)
{
	return &item->source;
}
