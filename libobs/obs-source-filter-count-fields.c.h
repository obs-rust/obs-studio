#include "libobs/build/libobs.pch.h"

struct obs_source_filter_count_fields obs_source_filter_count_fields(const obs_source_t *source)
{
	return (struct obs_source_filter_count_fields){&source->filters.num};
}
