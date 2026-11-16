#include "libobs/build/libobs.pch.h"

obs_data_t *obs_source_private_settings_fields(const obs_source_t *source);
obs_data_t *obs_source_private_settings_fields(const obs_source_t *source)
{
	return source->private_settings;
}
