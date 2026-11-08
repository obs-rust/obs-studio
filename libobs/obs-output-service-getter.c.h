#include "libobs/build/libobs.pch.h"

obs_service_t *obs_output_get_service(const obs_output_t *output)
{
	return obs_output_valid(output, "obs_output_get_service") ? output->service : NULL;
}
