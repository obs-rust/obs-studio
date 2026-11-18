#include "libobs/build/libobs.pch.h"

void obs_source_set_default_flags(obs_source_t *source, uint32_t flags)
{
	if (!obs_source_valid(source, "obs_source_set_default_flags"))
		return;

	source->default_flags = flags;
}
