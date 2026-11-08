#include "libobs/build/libobs-ported-service-getters.pch.h"

static inline bool flag_service(const struct obs_output *output)
{
	return (output->info.flags & OBS_OUTPUT_SERVICE) != 0;
}

const char *obs_output_get_protocols(const obs_output_t *output)
{
	if (!obs_output_valid(output, "obs_output_get_protocols"))
		return NULL;

	return flag_service(output) ? output->info.protocols : NULL;
}
