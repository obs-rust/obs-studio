#include "libobs/build/libobs.pch.h"

struct obs_source_type_data_fields obs_source_type_data_fields(const obs_source_t *source)
{
	return (struct obs_source_type_data_fields){&source->info.type_data};
}
