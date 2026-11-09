#include "libobs/build/libobs.pch.h"

/* This unit has its own unity group so these unchanged scene lock helpers
 * stay file-local. Lock video before audio; unlock in reverse order. */
#define audio_lock(scene) pthread_mutex_lock(&scene->audio_mutex)
#define video_lock(scene) pthread_mutex_lock(&scene->video_mutex)
#define audio_unlock(scene) pthread_mutex_unlock(&scene->audio_mutex)
#define video_unlock(scene) pthread_mutex_unlock(&scene->video_mutex)

static inline void full_lock(struct obs_scene *scene)
{
	video_lock(scene);
	audio_lock(scene);
}

static inline void full_unlock(struct obs_scene *scene)
{
	audio_unlock(scene);
	video_unlock(scene);
}

obs_sceneitem_t *obs_scene_find_source(obs_scene_t *scene, const char *name)
{
	struct obs_scene_item *item;

	if (!scene)
		return NULL;

	full_lock(scene);

	item = scene->first_item;
	while (item) {
		if (strcmp(item->source->context.name, name) == 0)
			break;

		item = item->next;
	}

	full_unlock(scene);

	return item;
}

obs_sceneitem_t *obs_scene_find_source_recursive(obs_scene_t *scene, const char *name)
{
	struct obs_scene_item *item;

	if (!scene)
		return NULL;

	full_lock(scene);

	item = scene->first_item;
	while (item) {
		if (strcmp(item->source->context.name, name) == 0)
			break;

		if (item->is_group) {
			obs_scene_t *group = item->source->context.data;
			obs_sceneitem_t *child = obs_scene_find_source(group, name);
			if (child) {
				item = child;
				break;
			}
		}

		item = item->next;
	}

	full_unlock(scene);

	return item;
}

obs_sceneitem_t *obs_scene_find_sceneitem_by_id(obs_scene_t *scene, int64_t id)
{
	struct obs_scene_item *item;

	if (!scene)
		return NULL;

	full_lock(scene);

	item = scene->first_item;
	while (item) {
		if (item->id == id)
			break;

		item = item->next;
	}

	full_unlock(scene);

	return item;
}

obs_sceneitem_t *obs_scene_get_group(obs_scene_t *scene, const char *name)
{
	if (!scene || !name || !*name) {
		return NULL;
	}

	obs_sceneitem_t *group = NULL;
	obs_sceneitem_t *item;

	full_lock(scene);

	item = scene->first_item;
	while (item) {
		if (item->is_group && item->source->context.name) {
			if (strcmp(item->source->context.name, name) == 0) {
				group = item;
				break;
			}
		}

		item = item->next;
	}

	full_unlock(scene);

	return group;
}

#undef audio_lock
#undef video_lock
#undef audio_unlock
#undef video_unlock
