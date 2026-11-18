#include "libobs/build/libobs.pch.h"

struct obs_source_missing_files_getter_fields obs_source_missing_files_getter_fields(const obs_source_t *source)
{
	return (struct obs_source_missing_files_getter_fields){source->context.data, source->info.missing_files};
}
