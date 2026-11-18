#include "libobs/build/libobs.pch.h"

/* Original obs-output.c.h helper retained only for the isolated C reference. */
static inline bool flag_encoded(const struct obs_output *output)
{
	return (output->info.flags & OBS_OUTPUT_ENCODED) != 0;
}
