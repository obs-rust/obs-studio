#include "libobs/build/libobs.pch.h"

struct obs_output_total_bytes_fields obs_output_total_bytes_fields(const obs_output_t *output)
{
	return (struct obs_output_total_bytes_fields){&output->info.get_total_bytes, &output->delay_active,
						      &output->delay_capturing, &output->context.data};
}
