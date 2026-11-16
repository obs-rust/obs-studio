#include "libobs/build/libobs.pch.h"

void obs_sceneitem_defer_group_resize_begin(obs_sceneitem_t *item)
{
	if (!obs_ptr_valid(item, "obs_sceneitem_defer_group_resize_begin"))
		return;

	os_atomic_inc_long(&item->defer_group_resize);
}

void obs_sceneitem_defer_group_resize_end(obs_sceneitem_t *item)
{
	if (!obs_ptr_valid(item, "obs_sceneitem_defer_group_resize_end"))
		return;

	if (os_atomic_dec_long(&item->defer_group_resize) == 0)
		os_atomic_set_bool(&item->update_group_resize, true);
}
