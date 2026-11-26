#include "libobs/build/libobs.pch.h"

/* The fields and the lock obs_source_get_width() and obs_source_get_height()
 * reach (the obs-source shims). The caller rejects NULL and keeps the source
 * alive; the filter list is read only between lock and unlock. */
bool obs_source_get_size_has_data(const obs_source_t *source);
bool obs_source_get_size_has_data(const obs_source_t *source)
{
	return source->context.data != NULL;
}

int obs_source_get_size_type(const obs_source_t *source);
int obs_source_get_size_type(const obs_source_t *source)
{
	return (int)source->info.type;
}

void obs_source_get_size_lock(obs_source_t *source);
void obs_source_get_size_lock(obs_source_t *source)
{
	pthread_mutex_lock(&source->filter_mutex);
}

void obs_source_get_size_unlock(obs_source_t *source);
void obs_source_get_size_unlock(obs_source_t *source)
{
	pthread_mutex_unlock(&source->filter_mutex);
}

bool obs_source_get_size_has_filters(const obs_source_t *source);
bool obs_source_get_size_has_filters(const obs_source_t *source)
{
	return source->filters.num != 0;
}

obs_source_t *obs_source_get_size_first_filter(const obs_source_t *source);
obs_source_t *obs_source_get_size_first_filter(const obs_source_t *source)
{
	return source->filters.array[0];
}
