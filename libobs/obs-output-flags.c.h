#include "libobs/build/libobs-ported-service-getters.pch.h"

uint32_t obs_output_get_flags(const obs_output_t *output)
{
	return obs_output_valid(output, "obs_output_get_flags") ? output->info.flags : 0;
}
