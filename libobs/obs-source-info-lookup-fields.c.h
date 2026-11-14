#include "libobs/build/libobs.pch.h"

/* Private view of the core's source type registry for the Rust
 * get_source_info: the entry count, an entry's address and its ID. Rust never
 * reads struct obs_source_info's layout. */
size_t obs_source_info_lookup_count(void)
{
	return obs_core_texture_activity_core()->source_types.num;
}

struct obs_source_info *obs_source_info_lookup_entry(size_t index)
{
	return &obs_core_texture_activity_core()->source_types.array[index];
}

const char *obs_source_info_lookup_id(const struct obs_source_info *info)
{
	return info->id;
}
