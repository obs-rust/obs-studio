#include "libobs/build/libobs.pch.h"

/* Same atomic helpers used by the original unit in the O group. */
static inline bool active(const struct obs_output *output)
{
	return os_atomic_load_bool(&output->active);
}

static inline bool reconnecting(const struct obs_output *output)
{
	return os_atomic_load_bool(&output->reconnecting);
}

static inline bool delay_active(const struct obs_output *output)
{
	return os_atomic_load_bool(&output->delay_active);
}
