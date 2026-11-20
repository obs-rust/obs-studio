#include "libobs/build/libobs.pch.h"

const char *obs_output_get_name_context_name(const obs_output_t *output);

const char *obs_output_get_name_context_name(const obs_output_t *output)
{
	return output->context.name;
}
