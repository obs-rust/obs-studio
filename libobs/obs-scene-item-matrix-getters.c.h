#include "libobs/build/libobs.pch.h"

/* Reuse the captured-parent mutex of the scene core. */
obs_scene_t *obs_sceneitem_fields_lock(const obs_sceneitem_t *item);
void obs_sceneitem_fields_unlock(obs_scene_t *scene);

#define item_fields_lock obs_sceneitem_fields_lock
#define item_fields_unlock obs_sceneitem_fields_unlock

void obs_sceneitem_get_draw_transform(const obs_sceneitem_t *item, struct matrix4 *transform)
{
	if (item) {
		obs_scene_t *scene = item_fields_lock(item);
		matrix4_copy(transform, &item->draw_transform);
		item_fields_unlock(scene);
	}
}

void obs_sceneitem_get_box_transform(const obs_sceneitem_t *item, struct matrix4 *transform)
{
	if (item) {
		obs_scene_t *scene = item_fields_lock(item);
		matrix4_copy(transform, &item->box_transform);
		item_fields_unlock(scene);
	}
}

void obs_sceneitem_get_box_scale(const obs_sceneitem_t *item, struct vec2 *scale)
{
	if (item) {
		obs_scene_t *scene = item_fields_lock(item);
		*scale = item->box_scale;
		item_fields_unlock(scene);
	}
}

#undef item_fields_unlock
#undef item_fields_lock
