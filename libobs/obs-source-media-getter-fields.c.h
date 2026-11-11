#include "libobs/build/libobs.pch.h"

void *obs_source_media_getter_data(const obs_source_t *source);
void *obs_source_media_getter_data(const obs_source_t *source)
{
	return source->context.data;
}

uint32_t obs_source_media_getter_flags(const obs_source_t *source);
uint32_t obs_source_media_getter_flags(const obs_source_t *source)
{
	return source->info.output_flags;
}

typedef int64_t (*obs_source_media_time_callback)(void *);
typedef enum obs_media_state (*obs_source_media_state_callback)(void *);

obs_source_media_time_callback obs_source_media_getter_duration_callback(const obs_source_t *source);
obs_source_media_time_callback obs_source_media_getter_duration_callback(const obs_source_t *source)
{
	return source->info.media_get_duration;
}

obs_source_media_time_callback obs_source_media_getter_time_callback(const obs_source_t *source);
obs_source_media_time_callback obs_source_media_getter_time_callback(const obs_source_t *source)
{
	return source->info.media_get_time;
}

obs_source_media_state_callback obs_source_media_getter_state_callback(const obs_source_t *source);
obs_source_media_state_callback obs_source_media_getter_state_callback(const obs_source_t *source)
{
	return source->info.media_get_state;
}
