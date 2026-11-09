#include "libobs/build/libobs.pch.h"

static void obs_source_hotkey_push_to_mute(void *data, obs_hotkey_id id, obs_hotkey_t *key, bool pressed)
{
	struct audio_action action = {.timestamp = os_gettime_ns(), .type = AUDIO_ACTION_PTM, .set = pressed};

	UNUSED_PARAMETER(id);
	UNUSED_PARAMETER(key);

	struct obs_source *source = data;

	pthread_mutex_lock(&source->audio_actions_mutex);
	da_push_back(source->audio_actions, &action);
	pthread_mutex_unlock(&source->audio_actions_mutex);

	source->user_push_to_mute_pressed = pressed;
}

static void obs_source_hotkey_push_to_talk(void *data, obs_hotkey_id id, obs_hotkey_t *key, bool pressed)
{
	struct audio_action action = {.timestamp = os_gettime_ns(), .type = AUDIO_ACTION_PTT, .set = pressed};

	UNUSED_PARAMETER(id);
	UNUSED_PARAMETER(key);

	struct obs_source *source = data;

	pthread_mutex_lock(&source->audio_actions_mutex);
	da_push_back(source->audio_actions, &action);
	pthread_mutex_unlock(&source->audio_actions_mutex);

	source->user_push_to_talk_pressed = pressed;
}
