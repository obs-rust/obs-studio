#include "libobs/build/libobs.pch.h"

/* Dependencies stay in obs-scene.c.h in this same unity group. */
static inline obs_scene_t *item_fields_lock(const obs_sceneitem_t *item);
static inline void item_fields_unlock(obs_scene_t *scene);
static inline void pos_from_absolute(struct vec2 *dst, const struct vec2 *v, const obs_sceneitem_t *item);
static inline void item_relative_scale(struct vec2 *dst, const struct vec2 *v, const obs_sceneitem_t *item);
static void update_item_transform(struct obs_scene_item *item, bool update_tex);

#define do_update_transform(item)                                          \
	do {                                                               \
		if (!item->parent || item->parent->is_group)               \
			os_atomic_set_bool(&item->update_transform, true); \
		else                                                       \
			update_item_transform(item, false);                \
	} while (false)

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

#undef do_update_transform
