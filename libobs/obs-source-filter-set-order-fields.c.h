#include "libobs/build/libobs.pch.h"

/* What obs_source_filter_set_order() reads, writes and signals of a source
 * (the obs-source shim). The caller rejects NULL, keeps the source alive,
 * and touches the list only between lock and unlock, at indexes below a
 * count read under the lock; each call is one read or write. */
void obs_source_filter_set_order_lock(obs_source_t *source);
void obs_source_filter_set_order_lock(obs_source_t *source)
{
	pthread_mutex_lock(&source->filter_mutex);
}

size_t obs_source_filter_set_order_count(const obs_source_t *source);
size_t obs_source_filter_set_order_count(const obs_source_t *source)
{
	return source->filters.num;
}

obs_source_t *obs_source_filter_set_order_at(const obs_source_t *source, size_t index);
obs_source_t *obs_source_filter_set_order_at(const obs_source_t *source, size_t index)
{
	return source->filters.array[index];
}

uint32_t obs_source_filter_set_order_flags(const obs_source_t *filter);
uint32_t obs_source_filter_set_order_flags(const obs_source_t *filter)
{
	return filter->info.output_flags;
}

void obs_source_filter_set_order_move(obs_source_t *source, size_t from, size_t to);
void obs_source_filter_set_order_move(obs_source_t *source, size_t from, size_t to)
{
	da_move_item(source->filters, from, to);
}

void obs_source_filter_set_order_target(obs_source_t *filter, obs_source_t *target);
void obs_source_filter_set_order_target(obs_source_t *filter, obs_source_t *target)
{
	filter->filter_target = target;
}

void obs_source_filter_set_order_unlock(obs_source_t *source);
void obs_source_filter_set_order_unlock(obs_source_t *source)
{
	pthread_mutex_unlock(&source->filter_mutex);
}

void obs_source_filter_set_order_signal(obs_source_t *source);
void obs_source_filter_set_order_signal(obs_source_t *source)
{
	obs_source_dosignal(source, NULL, "reorder_filters");
}
