#include "libobs/build/libobs.pch.h"

const enum obs_source_type *obs_source_type_id_type_field(const obs_source_t *source)
{
	return &source->info.type;
}

const char *const *obs_source_type_id_id_field(const obs_source_t *source)
{
	return &source->info.id;
}
