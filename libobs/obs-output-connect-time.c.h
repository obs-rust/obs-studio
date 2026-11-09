#include "libobs/build/libobs-ported-service-getters.pch.h"

int obs_output_get_connect_time_ms(obs_output_t *output)
{
	if (!obs_output_valid(output, "obs_output_get_connect_time_ms"))
		return -1;

	if (output->info.get_connect_time_ms)
		return output->info.get_connect_time_ms(output->context.data);
	return -1;
}
