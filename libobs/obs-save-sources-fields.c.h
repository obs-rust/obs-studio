#include "libobs/build/libobs.pch.h"

/* Private adapters for the Rust obs_save_sources_filtered() and
 * obs_save_sources() (obs-save-sources.c.h, #5567): the sources mutex, the
 * uuid table's head and links, the four fields the save checks (each read
 * on its own, so the Rust makes the C's reads in the C's order), and the
 * array, save, push and release calls. The table, its layout and the data
 * calls stay in C; Rust decides which reads and calls are made, and in which
 * order. */

void obs_save_sources_lock(void);
void obs_save_sources_lock(void)
{
	pthread_mutex_lock(&obs->data.sources_mutex);
}

void obs_save_sources_unlock(void);
void obs_save_sources_unlock(void)
{
	pthread_mutex_unlock(&obs->data.sources_mutex);
}

obs_source_t *obs_save_sources_first(void);
obs_source_t *obs_save_sources_first(void)
{
	return obs->data.sources;
}

obs_source_t *obs_save_sources_next(const obs_source_t *source);
obs_source_t *obs_save_sources_next(const obs_source_t *source)
{
	return (obs_source_t *)source->context.hh_uuid.next;
}

bool obs_save_sources_is_filter(const obs_source_t *source);
bool obs_save_sources_is_filter(const obs_source_t *source)
{
	return source->info.type == OBS_SOURCE_TYPE_FILTER;
}

bool obs_save_sources_removed(const obs_source_t *source);
bool obs_save_sources_removed(const obs_source_t *source)
{
	return source->removed;
}

bool obs_save_sources_temp_removed(const obs_source_t *source);
bool obs_save_sources_temp_removed(const obs_source_t *source)
{
	return source->temp_removed;
}

bool obs_save_sources_private(const obs_source_t *source);
bool obs_save_sources_private(const obs_source_t *source)
{
	return source->context.private;
}

obs_data_array_t *obs_save_sources_array_create(void);
obs_data_array_t *obs_save_sources_array_create(void)
{
	return obs_data_array_create();
}

obs_data_t *obs_save_sources_save(obs_source_t *source);
obs_data_t *obs_save_sources_save(obs_source_t *source)
{
	return obs_save_source(source);
}

void obs_save_sources_push(obs_data_array_t *array, obs_data_t *data);
void obs_save_sources_push(obs_data_array_t *array, obs_data_t *data)
{
	obs_data_array_push_back(array, data);
}

void obs_save_sources_release(obs_data_t *data);
void obs_save_sources_release(obs_data_t *data)
{
	obs_data_release(data);
}
