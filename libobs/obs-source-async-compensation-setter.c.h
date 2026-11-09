#include "libobs/build/libobs.pch.h"

void obs_source_set_async_compensation(obs_source_t *source, bool compensate)
{
	if (!obs_source_valid(source, "obs_source_set_async_compensation"))
		return;

	os_atomic_store_bool(&source->async_compensation, compensate);
}
