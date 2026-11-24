#include "libobs/build/libobs.pch.h"

bool obs_source_async_compensation(const obs_source_t *source)
{
	return obs_source_valid(source, "obs_source_async_compensation")
		       ? os_atomic_load_bool(&source->async_compensation)
		       : false;
}
