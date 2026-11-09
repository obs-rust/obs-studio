#include "libobs/build/libobs.pch.h"

size_t obs_source_filter_count(const obs_source_t *source)
{
	return obs_source_valid(source, "obs_source_filter_count") ? source->filters.num : 0;
}
