#include "libobs/build/libobs.pch.h"

struct obs_output_pause_queries_fields obs_output_pause_queries_fields(obs_output_t *output)
{
	return (struct obs_output_pause_queries_fields){&output->paused, &output->pause.ts_offset,
							&output->pause.mutex};
}

void obs_output_pause_queries_lock(void *mutex)
{
	pthread_mutex_lock(mutex);
}

void obs_output_pause_queries_unlock(void *mutex)
{
	pthread_mutex_unlock(mutex);
}
