#include "libobs/build/libobs.pch.h"

uint32_t obs_source_get_output_flags(const obs_source_t *source)
{
	return obs_source_valid(source, "obs_source_get_output_flags") ? source->info.output_flags : 0;
}
