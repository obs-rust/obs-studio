#include "libobs/build/libobs.pch.h"

struct obs_output_connect_time_fields obs_output_connect_time_fields(const obs_output_t *output)
{
	return output ? (struct obs_output_connect_time_fields){output->context.data, output->info.get_connect_time_ms}
		      : (struct obs_output_connect_time_fields){0};
}
