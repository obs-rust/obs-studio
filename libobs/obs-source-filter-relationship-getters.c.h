#include "libobs/build/libobs.pch.h"

obs_source_t *obs_filter_get_parent(const obs_source_t *filter)
{
	return obs_ptr_valid(filter, "obs_filter_get_parent") ? filter->filter_parent : NULL;
}

obs_source_t *obs_filter_get_target(const obs_source_t *filter)
{
	return obs_ptr_valid(filter, "obs_filter_get_target") ? filter->filter_target : NULL;
}
