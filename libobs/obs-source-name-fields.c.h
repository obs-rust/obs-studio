#include "libobs/build/libobs.pch.h"

const char *obs_source_name_field(const obs_source_t *source);
const char *obs_source_name_field(const obs_source_t *source)
{
	return source->context.name;
}
