#include "libobs/build/libobs.pch.h"

obs_source_t *const *obs_source_filter_index_lock(obs_source_t *source, size_t *count)
{
	pthread_mutex_lock(&source->filter_mutex);
	*count = source->filters.num;
	return source->filters.array;
}

void obs_source_filter_index_unlock(obs_source_t *source)
{
	pthread_mutex_unlock(&source->filter_mutex);
}
