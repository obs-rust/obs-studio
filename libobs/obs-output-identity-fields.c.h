#include "libobs/build/libobs.pch.h"

struct obs_output_identity_fields obs_output_identity_fields(const obs_output_t *output)
{
	return (struct obs_output_identity_fields){&output->info.id, &output->info.type_data};
}
