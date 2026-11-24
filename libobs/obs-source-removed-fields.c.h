#include "libobs/build/libobs.pch.h"

const volatile bool *obs_source_removed_field(const obs_source_t *source);
const volatile bool *obs_source_removed_field(const obs_source_t *source)
{
	return &source->removed;
}
