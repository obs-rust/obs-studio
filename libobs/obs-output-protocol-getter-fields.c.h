#include "libobs/build/libobs.pch.h"

struct obs_output_protocol_getter_fields obs_output_protocol_getter_fields(const obs_output_t *output)
{
	return (struct obs_output_protocol_getter_fields){&output->info.flags, &output->info.protocols};
}
