#include "libobs/build/libobs.pch.h"

void obs_source_enum_filters(obs_source_t *source, obs_source_enum_proc_t callback, void *param)
{
	if (!obs_source_valid(source, "obs_source_enum_filters"))
		return;
	union {
		void *obj;
		obs_source_enum_proc_t fn;
	} callback_obj = {.fn = callback};

	if (!obs_ptr_valid(callback_obj.obj, "obs_source_enum_filters"))
		return;

	pthread_mutex_lock(&source->filter_mutex);

	for (size_t i = source->filters.num; i > 0; i--) {
		struct obs_source *filter = source->filters.array[i - 1];
		callback(source, filter, param);
	}

	pthread_mutex_unlock(&source->filter_mutex);
}
