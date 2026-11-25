#include "libobs/build/libobs.pch.h"

/* What obs_source_enum_filters() reads of a source (the obs-source shim).
 * The caller rejects NULL, keeps the source alive, and reads the list only
 * between lock and unlock; each call is one read. */
void obs_source_enum_filters_lock(obs_source_t *source);
void obs_source_enum_filters_lock(obs_source_t *source)
{
	pthread_mutex_lock(&source->filter_mutex);
}

size_t obs_source_enum_filters_count(const obs_source_t *source);
size_t obs_source_enum_filters_count(const obs_source_t *source)
{
	return source->filters.num;
}

obs_source_t *obs_source_enum_filters_at(const obs_source_t *source, size_t index);
obs_source_t *obs_source_enum_filters_at(const obs_source_t *source, size_t index)
{
	return source->filters.array[index];
}

void obs_source_enum_filters_unlock(obs_source_t *source);
void obs_source_enum_filters_unlock(obs_source_t *source)
{
	pthread_mutex_unlock(&source->filter_mutex);
}
