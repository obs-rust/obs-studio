#include "libobs/build/libobs.pch.h"

struct obs_source_boolean_query_fields obs_source_boolean_query_fields(const obs_source_t *source)
{
	return (struct obs_source_boolean_query_fields){&source->texcoords_centered, &source->temp_removed};
}
