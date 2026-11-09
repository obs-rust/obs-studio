#include "libobs/build/libobs.pch.h"

struct obs_output_congestion_fields obs_output_congestion_fields(const obs_output_t *output)
{
	return (struct obs_output_congestion_fields){output->info.get_congestion, output->context.data};
}
