#include "libobs/build/libobs-ported-service-getters.pch.h"

static inline bool active(const struct obs_output *output)
{
	return os_atomic_load_bool(&output->active);
}

static inline bool reconnecting(const struct obs_output *output)
{
	return os_atomic_load_bool(&output->reconnecting);
}

bool obs_output_active(const obs_output_t *output)
{
	return (output != NULL) ? (active(output) || reconnecting(output)) : false;
}
