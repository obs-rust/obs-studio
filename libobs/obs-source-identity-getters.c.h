#include "libobs/build/libobs.pch.h"

const char *obs_source_get_uuid(const obs_source_t *source)
{
	return obs_source_valid(source, "obs_source_get_uuid") ? source->context.uuid : NULL;
}

const char *obs_source_get_unversioned_id(const obs_source_t *source)
{
	return obs_source_valid(source, "obs_source_get_unversioned_id") ? source->info.unversioned_id : NULL;
}
