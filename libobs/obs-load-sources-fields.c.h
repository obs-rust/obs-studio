#include "libobs/build/libobs.pch.h"

/* Private adapters for the Rust obs_load_sources() (obs-load-sources.c.h,
 * #5574): the array count and item take, the data release, the source load,
 * the type check, the transition load, the second-pass load and the source
 * release. The data and source code stays in C; Rust decides which calls
 * are made, and in which order. */

size_t obs_load_sources_count(obs_data_array_t *array);
size_t obs_load_sources_count(obs_data_array_t *array)
{
	return obs_data_array_count(array);
}

obs_data_t *obs_load_sources_item(obs_data_array_t *array, size_t index);
obs_data_t *obs_load_sources_item(obs_data_array_t *array, size_t index)
{
	return obs_data_array_item(array, index);
}

void obs_load_sources_release_data(obs_data_t *data);
void obs_load_sources_release_data(obs_data_t *data)
{
	obs_data_release(data);
}

obs_source_t *obs_load_sources_load(obs_data_t *data);
obs_source_t *obs_load_sources_load(obs_data_t *data)
{
	return obs_load_source(data);
}

bool obs_load_sources_is_transition(const obs_source_t *source);
bool obs_load_sources_is_transition(const obs_source_t *source)
{
	return source->info.type == OBS_SOURCE_TYPE_TRANSITION;
}

void obs_load_sources_transition_load(obs_source_t *source, obs_data_t *data);
void obs_load_sources_transition_load(obs_source_t *source, obs_data_t *data)
{
	obs_transition_load(source, data);
}

void obs_load_sources_load2(obs_source_t *source);
void obs_load_sources_load2(obs_source_t *source)
{
	obs_source_load2(source);
}

void obs_load_sources_release_source(obs_source_t *source);
void obs_load_sources_release_source(obs_source_t *source)
{
	obs_source_release(source);
}
