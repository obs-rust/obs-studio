#include "libobs/build/libobs.pch.h"

/* Dependencies stay in obs-scene.c.h in this same unity group. */
static inline void full_lock(struct obs_scene *scene);
static inline void full_unlock(struct obs_scene *scene);
static inline void detach_sceneitem(struct obs_scene_item *item);
static inline void attach_sceneitem(struct obs_scene *parent, struct obs_scene_item *item, struct obs_scene_item *prev);
static inline void signal_reorder(struct obs_scene_item *item);

void obs_sceneitem_set_order(obs_sceneitem_t *item, enum obs_order_movement movement)
{
	if (!item)
		return;

	struct obs_scene_item *next, *prev;
	struct obs_scene *scene = obs_scene_get_ref(item->parent);

	if (!scene)
		return;

	full_lock(scene);

	next = item->next;
	prev = item->prev;

	int anchor = obs_scene_item_order_anchor((int)movement, prev != NULL, next != NULL);

	if (anchor == OBS_SCENE_ITEM_ORDER_ANCHOR_STAY) {
		full_unlock(scene);
		obs_scene_release(scene);
		return;
	}

	detach_sceneitem(item);

	if (anchor == OBS_SCENE_ITEM_ORDER_ANCHOR_PREV_PREV) {
		attach_sceneitem(scene, item, prev->prev);

	} else if (anchor == OBS_SCENE_ITEM_ORDER_ANCHOR_NEXT) {
		attach_sceneitem(scene, item, next);

	} else if (anchor == OBS_SCENE_ITEM_ORDER_ANCHOR_PREV) {
		attach_sceneitem(scene, item, prev);

	} else if (anchor == OBS_SCENE_ITEM_ORDER_ANCHOR_LAST) {
		struct obs_scene_item *last = next;
		if (!last) {
			last = prev;
		} else {
			while (last->next)
				last = last->next;
		}

		attach_sceneitem(scene, item, last);

	} else if (anchor == OBS_SCENE_ITEM_ORDER_ANCHOR_BOTTOM) {
		attach_sceneitem(scene, item, NULL);
	}

	full_unlock(scene);

	signal_reorder(item);
	obs_scene_release(scene);
}
