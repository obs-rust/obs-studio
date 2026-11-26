#include "libobs/build/libobs-ported-service-getters.pch.h"

uint64_t obs_source_get_last_async_ts(const obs_source_t *source)
{
	return source->async_last_rendered_ts;
}
