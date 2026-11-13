#include "libobs/build/libobs.pch.h"

/* Private source-layout and mutex operations for the Rust backup core. */
void obs_source_filter_backup_lock(obs_source_t *source);
void obs_source_filter_backup_unlock(obs_source_t *source);
size_t obs_source_filter_backup_count(const obs_source_t *source);
obs_source_t *obs_source_filter_backup_at(obs_source_t *source, size_t index);

void obs_source_filter_backup_lock(obs_source_t *source)
{
	pthread_mutex_lock(&source->filter_mutex);
}

void obs_source_filter_backup_unlock(obs_source_t *source)
{
	pthread_mutex_unlock(&source->filter_mutex);
}

size_t obs_source_filter_backup_count(const obs_source_t *source)
{
	return source->filters.num;
}

obs_source_t *obs_source_filter_backup_at(obs_source_t *source, size_t index)
{
	return source->filters.array[index];
}
