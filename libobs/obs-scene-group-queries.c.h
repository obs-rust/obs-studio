#include "libobs/build/libobs.pch.h"

bool obs_sceneitem_is_group(obs_sceneitem_t *item)
{
	return item && item->is_group;
}

bool obs_scene_is_group(const obs_scene_t *scene)
{
	return scene ? scene->is_group : false;
}
