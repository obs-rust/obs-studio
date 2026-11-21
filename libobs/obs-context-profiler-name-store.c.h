#include "libobs/build/libobs.pch.h"

profiler_name_store_t *obs_get_profiler_name_store(void)
{
	return obs->name_store;
}
