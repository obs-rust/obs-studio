#include "libobs/build/libobs.pch.h"

const char *obs_output_get_name(const obs_output_t *output)
{
	return obs_output_valid(output, "obs_output_get_name") ? output->context.name : NULL;
}
