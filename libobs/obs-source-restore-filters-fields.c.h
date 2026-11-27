#include "libobs/build/libobs.pch.h"

/* What obs_source_restore_filters() reads, writes, loads and releases (the
 * obs-source shim). The caller rejects a NULL source and array, keeps both
 * alive, touches the list only between lock and unlock, at indexes below a
 * count read under the lock, and holds a reference to every filter it
 * passes (an old filter until it releases it, a restored one from its
 * reference or load); each call is one read, one write or one libobs
 * call. */
void obs_source_restore_filters_lock(obs_source_t *source);
void obs_source_restore_filters_lock(obs_source_t *source)
{
	pthread_mutex_lock(&source->filter_mutex);
}

size_t obs_source_restore_filters_count(const obs_source_t *source);
size_t obs_source_restore_filters_count(const obs_source_t *source)
{
	return source->filters.num;
}

obs_source_t *obs_source_restore_filters_at(const obs_source_t *source, size_t index);
obs_source_t *obs_source_restore_filters_at(const obs_source_t *source, size_t index)
{
	return source->filters.array[index];
}

void obs_source_restore_filters_set_parent(obs_source_t *filter, obs_source_t *parent);
void obs_source_restore_filters_set_parent(obs_source_t *filter, obs_source_t *parent)
{
	filter->filter_parent = parent;
}

void obs_source_restore_filters_set_target(obs_source_t *filter, obs_source_t *target);
void obs_source_restore_filters_set_target(obs_source_t *filter, obs_source_t *target)
{
	filter->filter_target = target;
}

void obs_source_restore_filters_free_list(obs_source_t *source);
void obs_source_restore_filters_free_list(obs_source_t *source)
{
	da_free(source->filters);
}

void obs_source_restore_filters_unlock(obs_source_t *source);
void obs_source_restore_filters_unlock(obs_source_t *source)
{
	pthread_mutex_unlock(&source->filter_mutex);
}

size_t obs_source_restore_filters_item_count(obs_data_array_t *array);
size_t obs_source_restore_filters_item_count(obs_data_array_t *array)
{
	return obs_data_array_count(array);
}

obs_data_t *obs_source_restore_filters_item(obs_data_array_t *array, size_t index);
obs_data_t *obs_source_restore_filters_item(obs_data_array_t *array, size_t index)
{
	return obs_data_array_item(array, index);
}

const char *obs_source_restore_filters_item_name(obs_data_t *item);
const char *obs_source_restore_filters_item_name(obs_data_t *item)
{
	return obs_data_get_string(item, "name");
}

const char *obs_source_restore_filters_name(const obs_source_t *filter);
const char *obs_source_restore_filters_name(const obs_source_t *filter)
{
	return filter->context.name;
}

obs_source_t *obs_source_restore_filters_get_ref(obs_source_t *filter);
obs_source_t *obs_source_restore_filters_get_ref(obs_source_t *filter)
{
	return obs_source_get_ref(filter);
}

obs_source_t *obs_source_restore_filters_load(obs_data_t *item);
obs_source_t *obs_source_restore_filters_load(obs_data_t *item)
{
	return obs_load_source(item);
}

obs_source_t *obs_source_restore_filters_load_private(obs_data_t *item);
obs_source_t *obs_source_restore_filters_load_private(obs_data_t *item)
{
	return obs_load_private_source(item);
}

/* da_move(source->filters, new_filters) for the shim's list: the old array
 * is freed and the source's list becomes a copy of the count filters. */
void obs_source_restore_filters_store(obs_source_t *source, obs_source_t *const *filters, size_t count);
void obs_source_restore_filters_store(obs_source_t *source, obs_source_t *const *filters, size_t count)
{
	da_free(source->filters);
	if (count)
		da_push_back_array(source->filters, filters, count);
}

void obs_source_restore_filters_release_item(obs_data_t *item);
void obs_source_restore_filters_release_item(obs_data_t *item)
{
	obs_data_release(item);
}

void obs_source_restore_filters_release(obs_source_t *filter);
void obs_source_restore_filters_release(obs_source_t *filter)
{
	obs_source_release(filter);
}
