#include "libobs/build/libobs.pch.h"

void obs_source_add_caption_callback(obs_source_t *source, obs_source_caption_t callback, void *param)
{
	struct caption_cb_info info = {callback, param};

	if (!obs_source_valid(source, "obs_source_add_caption_callback"))
		return;

	pthread_mutex_lock(&source->caption_cb_mutex);
	da_push_back(source->caption_cb_list, &info);
	pthread_mutex_unlock(&source->caption_cb_mutex);
}

void obs_source_remove_caption_callback(obs_source_t *source, obs_source_caption_t callback, void *param)
{
	struct caption_cb_info info = {callback, param};

	if (!obs_source_valid(source, "obs_source_remove_caption_callback"))
		return;

	pthread_mutex_lock(&source->caption_cb_mutex);
	da_erase_item(source->caption_cb_list, &info);
	pthread_mutex_unlock(&source->caption_cb_mutex);
}
