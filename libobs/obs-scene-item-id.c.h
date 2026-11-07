#include "libobs/build/libobs.pch.h"

int64_t obs_sceneitem_get_id(const obs_sceneitem_t *item)
{
	if (!obs_ptr_valid(item, "obs_sceneitem_get_id"))
		return 0;

	return item->id;
}
