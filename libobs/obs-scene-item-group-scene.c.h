#include "libobs/build/libobs.pch.h"

obs_scene_t *obs_sceneitem_group_get_scene(const obs_sceneitem_t *item)
{
	return (item && item->is_group) ? item->source->context.data : NULL;
}
