#include "libobs/build/libobs.pch.h"

bool obs_source_active(const obs_source_t *source)
{
	return obs_source_valid(source, "obs_source_active") ? os_atomic_load_long(&source->activate_refs) != 0 : false;
}

bool obs_source_showing(const obs_source_t *source)
{
	return obs_source_valid(source, "obs_source_showing") ? os_atomic_load_long(&source->show_refs) != 0 : false;
}
