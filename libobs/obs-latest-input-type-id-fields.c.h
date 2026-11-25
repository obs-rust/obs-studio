#include "libobs/build/libobs.pch.h"

uint32_t obs_latest_input_type_id_version_fields(const void *array, size_t index);
uint32_t obs_latest_input_type_id_version_fields(const void *array, size_t index)
{
	return ((const struct obs_source_info *)array)[index].version;
}

void obs_latest_input_type_id_assert_fields(bool found);
void obs_latest_input_type_id_assert_fields(bool found)
{
	assert(found);
	(void)found;
}
