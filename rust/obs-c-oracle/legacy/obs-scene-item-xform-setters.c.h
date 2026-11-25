#include "libobs/build/libobs.pch.h"

obs_scene_t *obs_sceneitem_fields_lock(const obs_sceneitem_t *item);
void obs_sceneitem_fields_unlock(obs_scene_t *scene);
void obs_sceneitem_xform_pos_from_absolute(struct vec2 *dst, const struct vec2 *v, const obs_sceneitem_t *item);
void obs_sceneitem_xform_relative_scale(struct vec2 *dst, const struct vec2 *v, const obs_sceneitem_t *item);
void obs_sceneitem_bounds_update_transform(obs_sceneitem_t *item);

#define item_fields_lock obs_sceneitem_fields_lock
#define item_fields_unlock obs_sceneitem_fields_unlock
#define pos_from_absolute obs_sceneitem_xform_pos_from_absolute
#define item_relative_scale obs_sceneitem_xform_relative_scale
#define do_update_transform(item) obs_sceneitem_bounds_update_transform(item)

/* Preserve the extracted C oracle bodies verbatim. */
/* clang-format off */
void obs_sceneitem_set_pos(obs_sceneitem_t *item, const struct vec2 *pos)
{
	if (item) {
		obs_scene_t *scene = item_fields_lock(item);
		if (!item->absolute_coordinates)
			pos_from_absolute(&item->pos, pos, item);
		else
			vec2_copy(&item->pos, pos);
		item_fields_unlock(scene);

		do_update_transform(item);
	}
}

void obs_sceneitem_set_rot(obs_sceneitem_t *item, float rot)
{
	if (item) {
		obs_scene_t *scene = item_fields_lock(item);
		item->rot = rot;
		item_fields_unlock(scene);
		do_update_transform(item);
	}
}

void obs_sceneitem_set_scale(obs_sceneitem_t *item, const struct vec2 *scale)
{
	if (item) {
		obs_scene_t *scene = item_fields_lock(item);
		if (!item->absolute_coordinates)
			item_relative_scale(&item->scale, scale, item);
		else
			vec2_copy(&item->scale, scale);
		item_fields_unlock(scene);

		do_update_transform(item);
	}
}

/* clang-format on */

#undef do_update_transform
#undef item_relative_scale
#undef pos_from_absolute
#undef item_fields_unlock
#undef item_fields_lock
