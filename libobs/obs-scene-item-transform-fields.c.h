#include "libobs/build/libobs.pch.h"

#define video_lock(scene) pthread_mutex_lock(&scene->video_mutex)
#define video_unlock(scene) pthread_mutex_unlock(&scene->video_mutex)

/* The transform fields of an item: the render path reads them under the
 * parent's video lock, so a setter or getter on another thread takes it too.
 * Hold it only for the fields: no signal, and no call into a plugin (#3569). */
static inline obs_scene_t *item_fields_lock(const obs_sceneitem_t *item)
{
	obs_scene_t *scene = item->parent;
	if (scene)
		video_lock(scene);
	return scene;
}

static inline void item_fields_unlock(obs_scene_t *scene)
{
	if (scene)
		video_unlock(scene);
}

float obs_sceneitem_get_rot(const obs_sceneitem_t *item)
{
	if (!item)
		return 0.0f;

	obs_scene_t *scene = item_fields_lock(item);
	float value = item->rot;
	item_fields_unlock(scene);
	return value;
}

uint32_t obs_sceneitem_get_alignment(const obs_sceneitem_t *item)
{
	if (!item)
		return 0;

	obs_scene_t *scene = item_fields_lock(item);
	uint32_t value = item->align;
	item_fields_unlock(scene);
	return value;
}

#undef video_lock
#undef video_unlock
