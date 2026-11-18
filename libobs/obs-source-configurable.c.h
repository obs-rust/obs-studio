#include "libobs/build/libobs.pch.h"

/* obs-source.c's private data_valid(), which this group includes later. */
#define data_valid(source, f) (obs_source_valid(source, f) && (source)->context.data)

bool obs_source_configurable(const obs_source_t *source)
{
	return data_valid(source, "obs_source_configurable") &&
	       (source->info.get_properties || source->info.get_properties2);
}

#undef data_valid
