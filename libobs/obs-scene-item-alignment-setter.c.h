#include "libobs/build/libobs.pch.h"

/* Reuse the captured-parent mutex and transform dispatch of the scene core. */
obs_scene_t *obs_sceneitem_fields_lock(const obs_sceneitem_t *item);
void obs_sceneitem_fields_unlock(obs_scene_t *scene);
void obs_sceneitem_bounds_update_transform(obs_sceneitem_t *item);

#define item_fields_lock obs_sceneitem_fields_lock
#define item_fields_unlock obs_sceneitem_fields_unlock
#define do_update_transform(item) obs_sceneitem_bounds_update_transform(item)

void obs_sceneitem_set_alignment(obs_sceneitem_t *item, uint32_t alignment)
{
	if (item) {
		obs_scene_t *scene = item_fields_lock(item);
		item->align = alignment;
		item_fields_unlock(scene);
		do_update_transform(item);
	}
}

#undef do_update_transform
#undef item_fields_unlock
#undef item_fields_lock
