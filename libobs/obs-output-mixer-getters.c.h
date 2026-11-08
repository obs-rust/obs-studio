#include "libobs/build/libobs-ported-output-mixer-getters.pch.h"

static inline size_t get_first_mixer(const obs_output_t *output)
{
	return obs_output_first_mixer(output->mixer_mask);
}

size_t obs_output_get_mixer(const obs_output_t *output)
{
	if (!obs_output_valid(output, "obs_output_get_mixer"))
		return 0;

	return get_first_mixer(output);
}

size_t obs_output_get_mixers(const obs_output_t *output)
{
	return obs_output_valid(output, "obs_output_get_mixers") ? output->mixer_mask : 0;
}
