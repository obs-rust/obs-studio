#include "libobs/build/libobs.pch.h"

struct obs_output_active_fields obs_output_active_fields(const obs_output_t *output)
{
	return (struct obs_output_active_fields){output ? &output->active : NULL,
						output ? &output->reconnecting : NULL};
}
