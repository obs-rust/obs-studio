#include "libobs/build/libobs.pch.h"

bool obs_sceneitem_visible(const obs_sceneitem_t *item)
{
	return item ? os_atomic_load_bool(&item->user_visible) : false;
}
