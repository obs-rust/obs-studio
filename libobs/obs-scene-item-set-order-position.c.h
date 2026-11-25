#include "libobs/build/libobs.pch.h"

/* Dependencies stay in obs-scene.c.h in this same unity group. */
static inline void full_lock(struct obs_scene *scene);
static inline void full_unlock(struct obs_scene *scene);
static inline void detach_sceneitem(struct obs_scene_item *item);
static inline void attach_sceneitem(struct obs_scene *parent, struct obs_scene_item *item, struct obs_scene_item *prev);
static inline void signal_reorder(struct obs_scene_item *item);

void obs_sceneitem_set_order_position(obs_sceneitem_t *item, int position)
{
	if (!item)
		return;

	struct obs_scene *scene = obs_scene_get_ref(item->parent);

	if (!scene)
		return;

	full_lock(scene);

	detach_sceneitem(item);

	size_t steps = 0;
	if (obs_scene_item_order_position(position, scene->first_item != NULL, &steps)) {
		attach_sceneitem(scene, item, NULL);
	} else {
		struct obs_scene_item *next = scene->first_item;

		for (size_t i = steps; i > 0; --i) {
			if (next->next == NULL)
				break;
			next = next->next;
		}

		attach_sceneitem(scene, item, next);
	}

	full_unlock(scene);

	signal_reorder(item);
	obs_scene_release(scene);
}
