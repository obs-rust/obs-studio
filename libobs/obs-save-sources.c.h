#include "libobs/build/libobs.pch.h"

obs_data_array_t *obs_save_sources_filtered(obs_save_source_filter_cb cb, void *data_)
{
	struct obs_core_data *data = &obs->data;
	obs_data_array_t *array;
	obs_source_t *source;

	array = obs_data_array_create();

	pthread_mutex_lock(&data->sources_mutex);

	source = data->sources;

	while (source) {
		if ((source->info.type != OBS_SOURCE_TYPE_FILTER) != 0 && !source->removed && !source->temp_removed &&
		    !source->context.private && cb(data_, source)) {
			obs_data_t *source_data = obs_save_source(source);

			obs_data_array_push_back(array, source_data);
			obs_data_release(source_data);
		}

		source = (obs_source_t *)source->context.hh_uuid.next;
	}

	pthread_mutex_unlock(&data->sources_mutex);

	return array;
}

static bool save_source_filter(void *data, obs_source_t *source)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(source);
	return true;
}

obs_data_array_t *obs_save_sources(void)
{
	return obs_save_sources_filtered(save_source_filter, NULL);
}
