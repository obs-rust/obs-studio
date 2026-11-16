#include "libobs/build/libobs.pch.h"

obs_data_t *obs_source_settings_getter_fields(const obs_source_t *source);
obs_data_t *obs_source_settings_getter_fields(const obs_source_t *source)
{
	return source->context.settings;
}
