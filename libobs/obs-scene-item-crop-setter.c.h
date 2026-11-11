#include "libobs/build/libobs.pch.h"

/* Reuse the existing parent capture and mutex boundary. The setter below is
 * verbatim, including its raw-input equality check and post-unlock dirty flag. */
obs_scene_t *obs_sceneitem_fields_lock(const obs_sceneitem_t *item);
void obs_sceneitem_fields_unlock(obs_scene_t *scene);

#define item_fields_lock obs_sceneitem_fields_lock
#define item_fields_unlock obs_sceneitem_fields_unlock

static inline bool crop_equal(const struct obs_sceneitem_crop *crop1, const struct obs_sceneitem_crop *crop2)
{
	return crop1->left == crop2->left && crop1->right == crop2->right && crop1->top == crop2->top &&
	       crop1->bottom == crop2->bottom;
}

void obs_sceneitem_set_crop(obs_sceneitem_t *item, const struct obs_sceneitem_crop *crop)
{
	if (!obs_ptr_valid(item, "obs_sceneitem_set_crop"))
		return;
	if (!obs_ptr_valid(crop, "obs_sceneitem_set_crop"))
		return;

	/* Clamp first: the render path must not see a value that is not clamped. */
	struct obs_sceneitem_crop clamped = *crop;
	clamped.left = obs_scene_clamp_crop(clamped.left);
	clamped.right = obs_scene_clamp_crop(clamped.right);
	clamped.top = obs_scene_clamp_crop(clamped.top);
	clamped.bottom = obs_scene_clamp_crop(clamped.bottom);

	obs_scene_t *scene = item_fields_lock(item);
	if (crop_equal(crop, &item->crop)) {
		item_fields_unlock(scene);
		return;
	}
	item->crop = clamped;
	item_fields_unlock(scene);

	os_atomic_set_bool(&item->update_transform, true);
}

#undef item_fields_lock
#undef item_fields_unlock
