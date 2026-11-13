#include "libobs/build/libobs.pch.h"

obs_data_array_t *obs_source_backup_filters(obs_source_t *source)
{
	if (!obs_source_valid(source, "obs_source_backup_filters"))
		return NULL;

	obs_data_array_t *array = obs_data_array_create();

	pthread_mutex_lock(&source->filter_mutex);
	for (size_t i = 0; i < source->filters.num; i++) {
		struct obs_source *filter = source->filters.array[i];
		obs_data_t *data = obs_save_source(filter);
		obs_data_array_push_back(array, data);
		obs_data_release(data);
	}
	pthread_mutex_unlock(&source->filter_mutex);

	return array;
}
