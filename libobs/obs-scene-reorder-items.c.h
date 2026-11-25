#include "libobs/build/libobs.pch.h"

/* Private helpers remain in the same core-source translation unit. */
static inline void full_lock(struct obs_scene *scene);
static inline void full_unlock(struct obs_scene *scene);
static inline void signal_reorder(struct obs_scene_item *item);

static bool sceneitems_match(obs_scene_t *scene, obs_sceneitem_t *const *items, size_t size, bool *order_matches)
{
	obs_sceneitem_t *item = scene->first_item;

	size_t count = 0;
	while (item) {
		if (!obs_scene_item_order_match_item(items, size, item, count, order_matches))
			return false;

		item = item->next;
		count += 1;
	}

	return count == size;
}

bool obs_scene_reorder_items(obs_scene_t *scene, obs_sceneitem_t *const *item_order, size_t item_order_size)
{
	if (!scene || !item_order_size)
		return false;

	scene = obs_scene_get_ref(scene);
	if (!scene)
		return false;

	full_lock(scene);

	bool order_matches = true;
	if (!sceneitems_match(scene, item_order, item_order_size, &order_matches) || order_matches) {
		full_unlock(scene);
		obs_scene_release(scene);
		return false;
	}

	scene->first_item = item_order[0];

	obs_sceneitem_t *prev = NULL;
	for (size_t i = 0; i < item_order_size; i++) {
		item_order[i]->prev = prev;
		item_order[i]->next = NULL;

		if (prev)
			prev->next = item_order[i];

		prev = item_order[i];
	}

	full_unlock(scene);

	signal_reorder(scene->first_item);
	obs_scene_release(scene);
	return true;
}
