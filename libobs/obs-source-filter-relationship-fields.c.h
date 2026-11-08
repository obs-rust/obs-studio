#include "libobs/build/libobs.pch.h"

struct obs_source_filter_relationship_fields obs_source_filter_relationship_fields(const obs_source_t *filter)
{
	return (struct obs_source_filter_relationship_fields){filter ? &filter->filter_parent : NULL,
							      filter ? &filter->filter_target : NULL};
}
