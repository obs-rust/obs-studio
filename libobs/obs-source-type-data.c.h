#include "libobs/build/libobs.pch.h"

void *obs_source_get_type_data(obs_source_t *source)
{
	return obs_source_valid(source, "obs_source_get_type_data") ? source->info.type_data : NULL;
}
