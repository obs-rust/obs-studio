#include "libobs/build/libobs.pch.h"

/* The helper stays in the same core-source translation unit. */
static void update_item_transform(struct obs_scene_item *item, bool update_tex);

void obs_sceneitem_force_update_transform(obs_sceneitem_t *item)
{
	if (!item)
		return;

	if (os_atomic_set_bool(&item->update_transform, false))
		update_item_transform(item, false);
}
