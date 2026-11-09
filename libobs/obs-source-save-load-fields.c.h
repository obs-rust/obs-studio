#include "libobs/build/libobs.pch.h"

void *obs_source_save_load_data(obs_source_t *source)
{
	return source->context.data;
}

void obs_source_save_load_callback(obs_source_t *source, bool save)
{
	void (*callback)(void *, obs_data_t *) = save ? source->info.save : source->info.load;
	if (callback)
		callback(source->context.data, source->context.settings);
}

void obs_source_save_load_signal(obs_source_t *source, bool save)
{
	obs_source_dosignal(source, save ? "source_save" : "source_load", save ? "save" : "load");
}

size_t obs_source_save_load_filter_count(obs_source_t *source)
{
	return source->filters.num;
}

obs_source_t *obs_source_save_load_filter(obs_source_t *source, size_t index)
{
	return source->filters.array[index];
}
