/* The crop integration executable links Rust's hidden rules and runs real OBS.
 * Its static scene archive needs the same private mutex boundary as libobs;
 * unlike pure-rule consumers, this executable may actually call the getters. */
#include "util/threading.h"
#include "obs-scene.h"

obs_scene_t *obs_sceneitem_fields_lock(const obs_sceneitem_t *item);
void obs_sceneitem_fields_unlock(obs_scene_t *scene);

obs_scene_t *obs_sceneitem_fields_lock(const obs_sceneitem_t *item)
{
	obs_scene_t *scene = item->parent;
	if (scene)
		pthread_mutex_lock(&scene->video_mutex);
	return scene;
}

void obs_sceneitem_fields_unlock(obs_scene_t *scene)
{
	if (scene)
		pthread_mutex_unlock(&scene->video_mutex);
}

struct vec2;
void obs_sceneitem_scale_canvas_read(struct vec2 *dst, const struct obs_scene_item *item);
void obs_sceneitem_scale_canvas_read(struct vec2 *dst, const struct obs_scene_item *item)
{
	struct obs_transform_info info = {0};
	obs_sceneitem_get_info2(item, &info);
	*dst = info.scale;
}
