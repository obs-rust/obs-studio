#include "libobs/build/libobs.pch.h"

void obs_source_filter_add(obs_source_t *source, obs_source_t *filter)
{
	struct calldata cd;
	uint8_t stack[128];

	if (!obs_source_valid(source, "obs_source_filter_add"))
		return;
	if (!obs_ptr_valid(filter, "obs_source_filter_add"))
		return;

	pthread_mutex_lock(&source->filter_mutex);

	if (!obs_source_filter_chain_add_allowed(da_find(source->filters, &filter, 0) != DARRAY_INVALID,
						 filter->filter_parent != NULL)) {
		blog(LOG_WARNING, "Tried to add a filter that was already "
				  "present on the source");
		pthread_mutex_unlock(&source->filter_mutex);
		return;
	}

	if (obs_source_filter_chain_checks_compatible(source->owns_info_id) && !filter_compatible(source, filter)) {
		pthread_mutex_unlock(&source->filter_mutex);
		return;
	}

	filter = obs_source_get_ref(filter);
	int ref_step = obs_source_filter_chain_ref_step(obs_ptr_valid(filter, "obs_source_filter_add"));
	if (ref_step != OBS_SOURCE_FILTER_CHAIN_REF_ADD) {
		if (ref_step == OBS_SOURCE_FILTER_CHAIN_REF_UNLOCK_RETURN)
			pthread_mutex_unlock(&source->filter_mutex);
		return;
	}

	filter->filter_parent = source;
	filter->filter_target = obs_source_filter_chain_add_target(source->filters.num) ==
						OBS_SOURCE_FILTER_CHAIN_TARGET_SOURCE
					? source
					: source->filters.array[0];

	da_insert(source->filters, 0, &filter);

	pthread_mutex_unlock(&source->filter_mutex);

	calldata_init_fixed(&cd, stack, sizeof(stack));
	calldata_set_ptr(&cd, "source", source);
	calldata_set_ptr(&cd, "filter", filter);

	signal_handler_signal(obs->signals, "source_filter_add", &cd);
	signal_handler_signal(source->context.signals, "filter_add", &cd);

	blog(LOG_DEBUG, "- filter '%s' (%s) added to source '%s'", filter->context.name, filter->info.id,
	     source->context.name);

	if (filter->info.filter_add)
		filter->info.filter_add(filter->context.data, filter->filter_parent);
}
