#include "libobs/build/libobs.pch.h"

obs_scene_t *obs_scene_get_ref(obs_scene_t *scene)
{
	if (!scene)
		return NULL;
	if (obs_source_get_ref(scene->source) != NULL)
		return scene;
	return NULL;
}

void obs_scene_release(obs_scene_t *scene)
{
	if (scene)
		obs_source_release(scene->source);
}
