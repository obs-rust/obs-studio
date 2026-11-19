#include "libobs/build/libobs.pch.h"

/* Reuse the existing full scene lock boundary without changing the body. */
void obs_scene_atomic_full_lock(obs_scene_t *scene);
void obs_scene_atomic_full_unlock(obs_scene_t *scene);

#define full_lock obs_scene_atomic_full_lock
#define full_unlock obs_scene_atomic_full_unlock

int obs_sceneitem_get_order_position(obs_sceneitem_t *item)
{
	if (!obs_scene_item_order_item_attached(item != NULL, item && item->parent))
		return 0;

	struct obs_scene *scene = item->parent;
	struct obs_scene_item *next = scene->first_item;

	full_lock(scene);

	int index = 0;
	while (next && next != item) {
		next = next->next;
		++index;
	}

	full_unlock(scene);

	return index;
}

#undef full_unlock
#undef full_lock
