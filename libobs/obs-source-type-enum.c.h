#include "libobs/build/libobs.pch.h"

bool obs_enum_source_types(size_t idx, const char **id)
{
	if (idx >= obs->source_types.num)
		return false;
	*id = obs->source_types.array[idx].id;
	return true;
}

bool obs_enum_input_types2(size_t idx, const char **id, const char **unversioned_id)
{
	if (idx >= obs->input_types.num)
		return false;
	if (id)
		*id = obs->input_types.array[idx].id;
	if (unversioned_id)
		*unversioned_id = obs->input_types.array[idx].unversioned_id;
	return true;
}
