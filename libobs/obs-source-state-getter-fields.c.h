#include "libobs/build/libobs.pch.h"

struct obs_source_state_getter_fields obs_source_state_getter_fields(const obs_source_t *source)
{
	return (struct obs_source_state_getter_fields){&source->flags, &source->enabled, &source->last_obs_ver};
}
