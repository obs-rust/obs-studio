#include "libobs/build/libobs.pch.h"

obs_source_t *obs_get_transition_by_uuid(const char *uuid)
{
	obs_source_t *source = obs_get_source_by_uuid(uuid);

	if (source && source->info.type == OBS_SOURCE_TYPE_TRANSITION)
		return source;
	else if (source)
		obs_source_release(source);

	return NULL;
}
