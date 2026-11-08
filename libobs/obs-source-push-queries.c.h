#include "libobs/build/libobs.pch.h"

bool obs_source_push_to_mute_enabled(obs_source_t *source)
{
	bool enabled;
	if (!obs_source_valid(source, "obs_source_push_to_mute_enabled"))
		return false;

	pthread_mutex_lock(&source->audio_mutex);
	enabled = source->push_to_mute_enabled;
	pthread_mutex_unlock(&source->audio_mutex);

	return enabled;
}

uint64_t obs_source_get_push_to_mute_delay(obs_source_t *source)
{
	uint64_t delay;
	if (!obs_source_valid(source, "obs_source_get_push_to_mute_delay"))
		return 0;

	pthread_mutex_lock(&source->audio_mutex);
	delay = source->push_to_mute_delay;
	pthread_mutex_unlock(&source->audio_mutex);

	return delay;
}

bool obs_source_push_to_talk_enabled(obs_source_t *source)
{
	bool enabled;
	if (!obs_source_valid(source, "obs_source_push_to_talk_enabled"))
		return false;

	pthread_mutex_lock(&source->audio_mutex);
	enabled = source->push_to_talk_enabled;
	pthread_mutex_unlock(&source->audio_mutex);

	return enabled;
}

uint64_t obs_source_get_push_to_talk_delay(obs_source_t *source)
{
	uint64_t delay;
	if (!obs_source_valid(source, "obs_source_get_push_to_talk_delay"))
		return 0;

	pthread_mutex_lock(&source->audio_mutex);
	delay = source->push_to_talk_delay;
	pthread_mutex_unlock(&source->audio_mutex);

	return delay;
}
