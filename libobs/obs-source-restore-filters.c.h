#include "libobs/build/libobs.pch.h"

void obs_source_restore_filters(obs_source_t *source, obs_data_array_t *array)
{
	if (!obs_source_valid(source, "obs_source_restore_filters"))
		return;
	if (!obs_ptr_valid(array, "obs_source_restore_filters"))
		return;

	DARRAY(obs_source_t *) cur_filters;
	DARRAY(obs_source_t *) new_filters;
	obs_source_t *prev = NULL;

	da_init(cur_filters);
	da_init(new_filters);

	pthread_mutex_lock(&source->filter_mutex);

	/* clear filter list */
	da_reserve(cur_filters, source->filters.num);
	da_reserve(new_filters, source->filters.num);
	for (size_t i = 0; i < source->filters.num; i++) {
		obs_source_t *filter = source->filters.array[i];
		da_push_back(cur_filters, &filter);
		filter->filter_parent = NULL;
		filter->filter_target = NULL;
	}

	da_free(source->filters);
	pthread_mutex_unlock(&source->filter_mutex);

	/* add backed up filters */
	size_t count = obs_data_array_count(array);
	for (size_t i = 0; i < count; i++) {
		obs_data_t *data = obs_data_array_item(array, i);
		const char *name = obs_data_get_string(data, "name");
		obs_source_t *filter = NULL;

		/* if backed up filter already exists, don't create */
		for (size_t j = 0; j < cur_filters.num; j++) {
			obs_source_t *cur = cur_filters.array[j];
			const char *cur_name = cur->context.name;
			if (obs_source_filter_chain_restore_matches(cur_name, name)) {
				filter = obs_source_get_ref(cur);
				break;
			}
		}

		int restore_step = obs_source_filter_chain_restore_step(filter != NULL);
		if (restore_step == OBS_SOURCE_FILTER_CHAIN_RESTORE_LOAD)
			filter = obs_load_source(data);
		else if (restore_step == OBS_SOURCE_FILTER_CHAIN_RESTORE_LOAD_PRIVATE)
			filter = obs_load_private_source(data);

		/* add filter; parents are set once every target is (#2172) */
		if (prev)
			prev->filter_target = filter;
		prev = filter;
		da_push_back(new_filters, &filter);

		obs_data_release(data);
	}

	if (prev)
		prev->filter_target = source;
	for (size_t i = 0; i < new_filters.num; i++)
		new_filters.array[i]->filter_parent = source;

	pthread_mutex_lock(&source->filter_mutex);
	da_move(source->filters, new_filters);
	pthread_mutex_unlock(&source->filter_mutex);

	/* release filters */
	for (size_t i = 0; i < cur_filters.num; i++) {
		obs_source_t *filter = cur_filters.array[i];
		obs_source_release(filter);
	}

	da_free(cur_filters);
}
