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

void obs_source_push_hotkey_dispatch(void *source, bool talk, bool pressed, uint64_t (*clock)(void),
				     void (*lock)(void *), void (*append)(void *, uint64_t, bool, bool),
				     void (*unlock)(void *), void (*store)(void *, bool, bool))
{
	UNUSED_PARAMETER(clock);
	UNUSED_PARAMETER(lock);
	UNUSED_PARAMETER(append);
	UNUSED_PARAMETER(unlock);
	UNUSED_PARAMETER(store);
	if (talk)
		obs_source_hotkey_push_to_talk(source, 0, NULL, pressed);
	else
		obs_source_hotkey_push_to_mute(source, 0, NULL, pressed);
}
