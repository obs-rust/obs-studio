#include "libobs/build/libobs.pch.h"

/* Original delay helpers (obs-output.c.h) for the isolated C reference and oracle. */

static inline bool delay_active(const struct obs_output *output)
{
	return os_atomic_load_bool(&output->delay_active);
}

static inline bool delay_capturing(const struct obs_output *output)
{
	return os_atomic_load_bool(&output->delay_capturing);
}
