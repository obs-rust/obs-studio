#include "libobs/build/libobs.pch.h"

bool obs_source_filter_remove_refless(obs_source_t *source, obs_source_t *filter)
{
	struct calldata cd;
	uint8_t stack[128];
	size_t idx;
	size_t prev_idx;

	pthread_mutex_lock(&source->filter_mutex);

	idx = da_find(source->filters, &filter, 0);
	if (idx == DARRAY_INVALID) {
		pthread_mutex_unlock(&source->filter_mutex);
		return false;
	}

	if (obs_source_filter_chain_remove_retarget(idx, &prev_idx)) {
		obs_source_t *prev = source->filters.array[prev_idx];
		prev->filter_target = filter->filter_target;
	}

	da_erase(source->filters, idx);

	pthread_mutex_unlock(&source->filter_mutex);

	calldata_init_fixed(&cd, stack, sizeof(stack));
	calldata_set_ptr(&cd, "source", source);
	calldata_set_ptr(&cd, "filter", filter);

	signal_handler_signal(obs->signals, "source_filter_remove", &cd);
	signal_handler_signal(source->context.signals, "filter_remove", &cd);

	blog(LOG_DEBUG, "- filter '%s' (%s) removed from source '%s'", filter->context.name, filter->info.id,
	     source->context.name);

	if (filter->info.filter_remove)
		filter->info.filter_remove(filter->context.data, filter->filter_parent);

	filter->filter_parent = NULL;
	filter->filter_target = NULL;
	return true;
}

void obs_source_filter_remove(obs_source_t *source, obs_source_t *filter)
{
	if (!obs_source_valid(source, "obs_source_filter_remove"))
		return;
	if (!obs_ptr_valid(filter, "obs_source_filter_remove"))
		return;

	if (obs_source_filter_remove_refless(source, filter))
		obs_source_release(filter);
}
