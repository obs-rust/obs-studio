#include "libobs/build/libobs.pch.h"

/* What obs_source_filter_remove() and obs_source_filter_remove_refless()
 * read, write, signal and call (the obs-source shim). The caller rejects a
 * NULL source, keeps the source and the filter alive, touches the list only
 * between lock and unlock, at indexes below a count read under the lock,
 * and follows the filter only once it was found on the list; each call is
 * one read, one write or one libobs call. */
void obs_source_filter_remove_lock(obs_source_t *source);
void obs_source_filter_remove_lock(obs_source_t *source)
{
	pthread_mutex_lock(&source->filter_mutex);
}

size_t obs_source_filter_remove_count(const obs_source_t *source);
size_t obs_source_filter_remove_count(const obs_source_t *source)
{
	return source->filters.num;
}

obs_source_t *obs_source_filter_remove_at(const obs_source_t *source, size_t index);
obs_source_t *obs_source_filter_remove_at(const obs_source_t *source, size_t index)
{
	return source->filters.array[index];
}

obs_source_t *obs_source_filter_remove_target(const obs_source_t *filter);
obs_source_t *obs_source_filter_remove_target(const obs_source_t *filter)
{
	return filter->filter_target;
}

void obs_source_filter_remove_set_target(obs_source_t *filter, obs_source_t *target);
void obs_source_filter_remove_set_target(obs_source_t *filter, obs_source_t *target)
{
	filter->filter_target = target;
}

void obs_source_filter_remove_erase(obs_source_t *source, size_t index);
void obs_source_filter_remove_erase(obs_source_t *source, size_t index)
{
	da_erase(source->filters, index);
}

void obs_source_filter_remove_unlock(obs_source_t *source);
void obs_source_filter_remove_unlock(obs_source_t *source)
{
	pthread_mutex_unlock(&source->filter_mutex);
}

void obs_source_filter_remove_signal(obs_source_t *source, obs_source_t *filter);
void obs_source_filter_remove_signal(obs_source_t *source, obs_source_t *filter)
{
	struct calldata cd;
	uint8_t stack[128];

	calldata_init_fixed(&cd, stack, sizeof(stack));
	calldata_set_ptr(&cd, "source", source);
	calldata_set_ptr(&cd, "filter", filter);

	signal_handler_signal(obs->signals, "source_filter_remove", &cd);
	signal_handler_signal(source->context.signals, "filter_remove", &cd);
}

void obs_source_filter_remove_log(obs_source_t *source, obs_source_t *filter);
void obs_source_filter_remove_log(obs_source_t *source, obs_source_t *filter)
{
	blog(LOG_DEBUG, "- filter '%s' (%s) removed from source '%s'", filter->context.name, filter->info.id,
	     source->context.name);
}

bool obs_source_filter_remove_has_callback(const obs_source_t *filter);
bool obs_source_filter_remove_has_callback(const obs_source_t *filter)
{
	return filter->info.filter_remove != NULL;
}

void obs_source_filter_remove_callback(obs_source_t *filter);
void obs_source_filter_remove_callback(obs_source_t *filter)
{
	filter->info.filter_remove(filter->context.data, filter->filter_parent);
}

void obs_source_filter_remove_clear_links(obs_source_t *filter);
void obs_source_filter_remove_clear_links(obs_source_t *filter)
{
	filter->filter_parent = NULL;
	filter->filter_target = NULL;
}

void obs_source_filter_remove_release(obs_source_t *filter);
void obs_source_filter_remove_release(obs_source_t *filter)
{
	obs_source_release(filter);
}
