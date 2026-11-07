#include "libobs/build/libobs-ported-service-getters.pch.h"

bool obs_source_async_unbuffered(const obs_source_t *source)
{
	return obs_source_valid(source, "obs_source_async_unbuffered") ? source->async_unbuffered : false;
}

bool obs_source_async_decoupled(const obs_source_t *source)
{
	return obs_source_valid(source, "obs_source_async_decoupled") ? source->async_decoupled : false;
}
