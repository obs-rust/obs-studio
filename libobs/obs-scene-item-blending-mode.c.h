#include "libobs/build/libobs.pch.h"

void obs_sceneitem_set_blending_mode(obs_sceneitem_t *item, enum obs_blending_type type)
{
	if (!obs_ptr_valid(item, "obs_sceneitem_set_blending_mode"))
		return;

	item->blend_type = type;

	os_atomic_set_bool(&item->update_transform, true);
}
