#include "libobs/build/libobs.pch.h"

struct obs_scene *const *obs_sceneitem_parent_fields(const struct obs_scene_item *item)
{
	return &item->parent;
}
