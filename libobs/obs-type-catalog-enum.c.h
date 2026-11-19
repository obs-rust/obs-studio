#include "libobs/build/libobs.pch.h"

bool obs_enum_input_types(size_t idx, const char **id)
{
	if (idx >= obs->input_types.num)
		return false;
	*id = obs->input_types.array[idx].id;
	return true;
}

bool obs_enum_filter_types(size_t idx, const char **id)
{
	if (idx >= obs->filter_types.num)
		return false;
	*id = obs->filter_types.array[idx].id;
	return true;
}

bool obs_enum_transition_types(size_t idx, const char **id)
{
	if (idx >= obs->transition_types.num)
		return false;
	*id = obs->transition_types.array[idx].id;
	return true;
}

bool obs_enum_output_types(size_t idx, const char **id)
{
	if (idx >= obs->output_types.num)
		return false;
	*id = obs->output_types.array[idx].id;
	return true;
}

bool obs_enum_encoder_types(size_t idx, const char **id)
{
	if (idx >= obs->encoder_types.num)
		return false;
	*id = obs->encoder_types.array[idx].id;
	return true;
}

bool obs_enum_service_types(size_t idx, const char **id)
{
	if (idx >= obs->service_types.num)
		return false;
	*id = obs->service_types.array[idx].id;
	return true;
}
