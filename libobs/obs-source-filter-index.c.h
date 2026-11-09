#include "libobs/build/libobs.pch.h"

int obs_source_filter_get_index(obs_source_t *source, obs_source_t *filter)
{
	if (!obs_source_valid(source, "obs_source_filter_get_index"))
		return -1;
	if (!obs_ptr_valid(filter, "obs_source_filter_get_index"))
		return -1;

	size_t idx;

	pthread_mutex_lock(&source->filter_mutex);
	idx = da_find(source->filters, &filter, 0);
	pthread_mutex_unlock(&source->filter_mutex);

	return idx != DARRAY_INVALID ? (int)idx : -1;
}
