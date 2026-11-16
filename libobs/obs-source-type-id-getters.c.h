#include "libobs/build/libobs.pch.h"

enum obs_source_type obs_source_get_type(const obs_source_t *source)
{
	return obs_source_valid(source, "obs_source_get_type") ? source->info.type : OBS_SOURCE_TYPE_INPUT;
}

const char *obs_source_get_id(const obs_source_t *source)
{
	return obs_source_valid(source, "obs_source_get_id") ? source->info.id : NULL;
}
