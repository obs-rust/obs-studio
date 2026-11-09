#include "libobs/build/libobs-ported-service-getters.pch.h"

static inline bool reconnecting(const struct obs_output *output)
{
	return os_atomic_load_bool(&output->reconnecting);
}

bool obs_output_reconnecting(const obs_output_t *output)
{
	if (!obs_output_valid(output, "obs_output_reconnecting"))
		return false;

	return reconnecting(output);
}
