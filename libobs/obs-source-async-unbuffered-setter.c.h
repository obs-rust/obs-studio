#include "libobs/build/libobs.pch.h"

void obs_source_set_async_unbuffered(obs_source_t *source, bool unbuffered)
{
	if (!obs_source_valid(source, "obs_source_set_async_unbuffered"))
		return;

	source->async_unbuffered = unbuffered;
}
