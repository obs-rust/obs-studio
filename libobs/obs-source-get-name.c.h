#include "libobs/build/libobs.pch.h"

const char *obs_source_get_name(const obs_source_t *source)
{
	return obs_source_valid(source, "obs_source_get_name") ? source->context.name : NULL;
}
