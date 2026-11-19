#include "libobs/build/libobs.pch.h"

size_t obs_service_find_count(void);
size_t obs_service_find_count(void)
{
	return obs->service_types.num;
}

const struct obs_service_info *obs_service_find_entry(size_t index);
const struct obs_service_info *obs_service_find_entry(size_t index)
{
	return obs->service_types.array + index;
}

const char *obs_service_find_id(const struct obs_service_info *entry);
const char *obs_service_find_id(const struct obs_service_info *entry)
{
	return entry->id;
}
