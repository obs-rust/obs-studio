#include "libobs/build/libobs.pch.h"

/* Existing scene dependencies: the item fields lock of obs-scene.c.h, and
 * the wrappers of obs-scene-item-bounds-setters-fields.c.h around the two
 * scene statics. The check below is shared with obs-scene.c.h's loaders. */
obs_scene_t *obs_sceneitem_fields_lock(const obs_sceneitem_t *item);
void obs_sceneitem_fields_unlock(obs_scene_t *scene);
void obs_sceneitem_bounds_size_from_absolute(struct vec2 *dst, const struct vec2 *v, const obs_sceneitem_t *item);
void obs_sceneitem_bounds_update_transform(obs_sceneitem_t *item);
enum obs_bounds_type obs_scene_bounds_valid_type(int64_t type);

#define item_fields_lock obs_sceneitem_fields_lock
#define item_fields_unlock obs_sceneitem_fields_unlock
#define size_from_absolute obs_sceneitem_bounds_size_from_absolute
#define valid_bounds_type obs_scene_bounds_valid_type
#define do_update_transform(item) obs_sceneitem_bounds_update_transform(item)

/* A bounds type outside the enum (a hand-edited or foreign scene
 * collection, undo data, a plugin) has no bounding box: OBS_BOUNDS_NONE.
 * Takes the int64 a load reads, before any cast truncates it, and an enum
 * value from a setter (an unsigned or a signed int, by compiler). */
enum obs_bounds_type valid_bounds_type(int64_t type)
{
	if (type < OBS_BOUNDS_NONE || type > OBS_BOUNDS_MAX_ONLY)
		return OBS_BOUNDS_NONE;
	return (enum obs_bounds_type)type;
}

void obs_sceneitem_set_bounds_type(obs_sceneitem_t *item, enum obs_bounds_type type)
{
	if (item) {
		obs_scene_t *scene = item_fields_lock(item);
		item->bounds_type = valid_bounds_type(type);
		item_fields_unlock(scene);
		do_update_transform(item);
	}
}

void obs_sceneitem_set_bounds_alignment(obs_sceneitem_t *item, uint32_t alignment)
{
	if (item) {
		obs_scene_t *scene = item_fields_lock(item);
		item->bounds_align = alignment;
		item_fields_unlock(scene);
		do_update_transform(item);
	}
}

void obs_sceneitem_set_bounds_crop(obs_sceneitem_t *item, bool crop)
{
	if (item) {
		obs_scene_t *scene = item_fields_lock(item);
		item->crop_to_bounds = crop;
		item_fields_unlock(scene);
		do_update_transform(item);
	}
}

void obs_sceneitem_set_bounds(obs_sceneitem_t *item, const struct vec2 *bounds)
{
	if (item) {
		obs_scene_t *scene = item_fields_lock(item);
		if (!item->absolute_coordinates)
			size_from_absolute(&item->bounds, bounds, item);
		else
			vec2_copy(&item->bounds, bounds);
		item_fields_unlock(scene);

		do_update_transform(item);
	}
}

#undef do_update_transform
#undef valid_bounds_type
#undef size_from_absolute
#undef item_fields_unlock
#undef item_fields_lock
