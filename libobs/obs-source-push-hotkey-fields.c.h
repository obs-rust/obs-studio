#include "libobs/build/libobs.pch.h"

void obs_source_push_hotkey_lock(void *data)
{
	struct obs_source *source = data;
	pthread_mutex_lock(&source->audio_actions_mutex);
}

void obs_source_push_hotkey_append(void *data, uint64_t timestamp, bool talk, bool pressed)
{
	struct obs_source *source = data;
	struct audio_action action = {.timestamp = timestamp,
				      .type = talk ? AUDIO_ACTION_PTT : AUDIO_ACTION_PTM,
				      .set = pressed};
	da_push_back(source->audio_actions, &action);
}

void obs_source_push_hotkey_unlock(void *data)
{
	struct obs_source *source = data;
	pthread_mutex_unlock(&source->audio_actions_mutex);
}

void obs_source_push_hotkey_store(void *data, bool talk, bool pressed)
{
	struct obs_source *source = data;
	if (talk)
		source->user_push_to_talk_pressed = pressed;
	else
		source->user_push_to_mute_pressed = pressed;
}
