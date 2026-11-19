#include "libobs/build/libobs.pch.h"

void obs_source_add_audio_capture_callback(obs_source_t *source, obs_source_audio_capture_t callback, void *param)
{
	struct audio_cb_info info = {callback, param};

	if (!obs_source_valid(source, "obs_source_add_audio_capture_callback"))
		return;

	pthread_mutex_lock(&source->audio_cb_mutex);
	da_push_back(source->audio_cb_list, &info);
	pthread_mutex_unlock(&source->audio_cb_mutex);
}

void obs_source_remove_audio_capture_callback(obs_source_t *source, obs_source_audio_capture_t callback, void *param)
{
	struct audio_cb_info info = {callback, param};

	if (!obs_source_valid(source, "obs_source_remove_audio_capture_callback"))
		return;

	pthread_mutex_lock(&source->audio_cb_mutex);
	da_erase_item(source->audio_cb_list, &info);
	pthread_mutex_unlock(&source->audio_cb_mutex);
}
