#include "libobs/build/libobs.pch.h"

void obs_sceneitem_set_scale_filter(obs_sceneitem_t *item, enum obs_scale_type filter)
{
	if (!obs_ptr_valid(item, "obs_sceneitem_set_scale_filter"))
		return;

	item->scale_filter = filter;

	os_atomic_set_bool(&item->update_transform, true);
}
