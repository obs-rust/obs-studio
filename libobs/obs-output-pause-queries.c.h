#include "libobs/build/libobs.pch.h"

bool obs_output_paused(const obs_output_t *output)
{
	return obs_output_valid(output, "obs_output_paused") ? os_atomic_load_bool(&output->paused) : false;
}

uint64_t obs_output_get_pause_offset(obs_output_t *output)
{
	uint64_t offset;

	if (!obs_output_valid(output, "obs_output_get_pause_offset"))
		return 0;

	pthread_mutex_lock(&output->pause.mutex);
	offset = output->pause.ts_offset;
	pthread_mutex_unlock(&output->pause.mutex);

	return offset;
}
