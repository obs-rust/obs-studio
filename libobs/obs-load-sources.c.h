#include "libobs/build/libobs.pch.h"

void obs_load_sources(obs_data_array_t *array, obs_load_source_cb cb, void *private_data)
{
	DARRAY(obs_source_t *) sources;
	size_t count;
	size_t i;

	da_init(sources);

	count = obs_data_array_count(array);
	da_reserve(sources, count);

	for (i = 0; i < count; i++) {
		obs_data_t *source_data = obs_data_array_item(array, i);
		obs_source_t *source = obs_load_source(source_data);

		da_push_back(sources, &source);

		obs_data_release(source_data);
	}

	/* tell sources that we want to load */
	for (i = 0; i < sources.num; i++) {
		obs_source_t *source = sources.array[i];
		obs_data_t *source_data = obs_data_array_item(array, i);
		if (source) {
			if (source->info.type == OBS_SOURCE_TYPE_TRANSITION)
				obs_transition_load(source, source_data);
			obs_source_load2(source);
			if (cb)
				cb(private_data, source);
		}
		obs_data_release(source_data);
	}

	for (i = 0; i < sources.num; i++)
		obs_source_release(sources.array[i]);

	da_free(sources);
}
