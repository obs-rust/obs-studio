#include "libobs/build/libobs.pch.h"

/* Defined in obs-source.c.h in this unity group. */
static inline bool data_valid(const struct obs_source *source, const char *f);
static inline enum obs_source_media_admit media_admit(obs_source_t *source, bool valid,
						      enum obs_source_media_call call);

int64_t obs_source_media_get_duration(obs_source_t *source)
{
	enum obs_source_media_admit admit = media_admit(source, data_valid(source, "obs_source_media_get_duration"),
							OBS_SOURCE_MEDIA_CALL_GET_DURATION);
	if (admit == OBS_SOURCE_MEDIA_ADMIT_READ_CALLBACK)
		admit = obs_source_media_admit_callback(source->info.media_get_duration != NULL);
	if (admit != OBS_SOURCE_MEDIA_ADMIT_ACCEPT)
		return obs_source_media_time_fallback();

	return source->info.media_get_duration(source->context.data);
}

int64_t obs_source_media_get_time(obs_source_t *source)
{
	enum obs_source_media_admit admit =
		media_admit(source, data_valid(source, "obs_source_media_get_time"), OBS_SOURCE_MEDIA_CALL_GET_TIME);
	if (admit == OBS_SOURCE_MEDIA_ADMIT_READ_CALLBACK)
		admit = obs_source_media_admit_callback(source->info.media_get_time != NULL);
	if (admit != OBS_SOURCE_MEDIA_ADMIT_ACCEPT)
		return obs_source_media_time_fallback();

	return source->info.media_get_time(source->context.data);
}

enum obs_media_state obs_source_media_get_state(obs_source_t *source)
{
	enum obs_source_media_admit admit =
		media_admit(source, data_valid(source, "obs_source_media_get_state"), OBS_SOURCE_MEDIA_CALL_GET_STATE);
	if (admit == OBS_SOURCE_MEDIA_ADMIT_READ_CALLBACK)
		admit = obs_source_media_admit_callback(source->info.media_get_state != NULL);
	if (admit != OBS_SOURCE_MEDIA_ADMIT_ACCEPT)
		return (enum obs_media_state)obs_source_media_state_fallback();

	return source->info.media_get_state(source->context.data);
}
