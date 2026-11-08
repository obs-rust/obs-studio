#include "libobs/build/libobs-ported-service-getters.pch.h"

uint32_t obs_source_get_flags(const obs_source_t *source)
{
	return obs_source_valid(source, "obs_source_get_flags") ? source->flags : 0;
}

bool obs_source_enabled(const obs_source_t *source)
{
	return obs_source_valid(source, "obs_source_enabled") ? source->enabled : false;
}

uint32_t obs_source_get_last_obs_version(const obs_source_t *source)
{
	return obs_source_valid(source, "obs_source_get_last_obs_version") ? source->last_obs_ver : 0;
}
