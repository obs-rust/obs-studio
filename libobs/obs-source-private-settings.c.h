#include "libobs/build/libobs.pch.h"

obs_data_t *obs_source_get_private_settings(obs_source_t *source)
{
	if (!obs_ptr_valid(source, "obs_source_get_private_settings"))
		return NULL;

	obs_data_addref(source->private_settings);
	return source->private_settings;
}
