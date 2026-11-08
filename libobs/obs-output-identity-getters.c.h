#include "libobs/build/libobs-ported-service-getters.pch.h"

void *obs_output_get_type_data(obs_output_t *output)
{
	return obs_output_valid(output, "obs_output_get_type_data") ? output->info.type_data : NULL;
}

const char *obs_output_get_id(const obs_output_t *output)
{
	return obs_output_valid(output, "obs_output_get_id") ? output->info.id : NULL;
}
