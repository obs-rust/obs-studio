#include "libobs/build/libobs.pch.h"

struct obs_output_service_getter_fields obs_output_service_getter_fields(const obs_output_t *output)
{
	return (struct obs_output_service_getter_fields){&output->service};
}
