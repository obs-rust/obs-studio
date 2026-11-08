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

void obs_sceneitem_scale_canvas_read(struct vec2 *dst, const obs_sceneitem_t *item);
#define item_canvas_scale obs_sceneitem_scale_canvas_read

void obs_sceneitem_get_scale(const obs_sceneitem_t *item, struct vec2 *scale)
{
	if (!item)
		return;

	obs_scene_t *scene = item_fields_lock(item);
	if (!item->absolute_coordinates)
		item_canvas_scale(scale, item);
	else
		vec2_copy(scale, &item->scale);
	item_fields_unlock(scene);
}

#undef item_canvas_scale
#undef video_lock
#undef video_unlock
