#include "libobs/build/libobs.pch.h"

obs_data_t *obs_sceneitem_get_private_settings(obs_sceneitem_t *item)
{
	if (!obs_ptr_valid(item, "obs_sceneitem_get_private_settings"))
		return NULL;

	obs_data_addref(item->private_settings);
	return item->private_settings;
}
