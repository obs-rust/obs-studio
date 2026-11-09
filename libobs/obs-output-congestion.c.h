#include "libobs/build/libobs-ported-output-mixer-getters.pch.h"

float obs_output_get_congestion(obs_output_t *output)
{
	if (!obs_output_valid(output, "obs_output_get_congestion"))
		return 0;

	if (output->info.get_congestion)
		return obs_output_congestion_clamp(output->info.get_congestion(output->context.data));
	return 0;
}
