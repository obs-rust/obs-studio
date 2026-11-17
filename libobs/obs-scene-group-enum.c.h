#include "libobs/build/libobs.pch.h"

void obs_sceneitem_group_enum_items(obs_sceneitem_t *group, bool (*callback)(obs_scene_t *, obs_sceneitem_t *, void *),
				    void *param)
{
	if (!group || !group->is_group)
		return;

	obs_scene_t *scene = group->source->context.data;
	if (scene)
		obs_scene_enum_items(scene, callback, param);
}
