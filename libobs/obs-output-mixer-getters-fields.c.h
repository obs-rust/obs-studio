#include "libobs/build/libobs.pch.h"

struct obs_output_mixer_getters_fields obs_output_mixer_getters_fields(const obs_output_t *output)
{
	return (struct obs_output_mixer_getters_fields){&output->mixer_mask};
}
